use std::net::SocketAddr;

use anyhow::Result;
use tokio::net::UdpSocket;

use rspotify::{model::*, prelude::OAuthClient};

use crate::{
    cli::{ContextType, Request},
    client::Client,
    state::SharedState,
};

use super::*;

pub async fn start_socket(client: Client, state: SharedState) -> Result<()> {
    let port = state.app_config.client_port;
    tracing::info!("Starting a client socket at 127.0.0.1:{port}");

    let socket = UdpSocket::bind(("127.0.0.1", port)).await?;

    // initialize the receive buffer to be 4096 bytes
    let mut buf = [0; 4096];
    loop {
        match socket.recv_from(&mut buf).await {
            Err(err) => tracing::warn!("failed to receive from the socket: {err}"),
            Ok((n_bytes, dest_addr)) => {
                let request: Request = serde_json::from_slice(&buf[0..n_bytes])?;
                tracing::info!("Handle socket request: {request:?}");
                handle_socket_request(&client, &state, request, &socket, dest_addr).await?;
            }
        }
    }
}

async fn send_data(data: Vec<u8>, socket: &UdpSocket, dest_addr: SocketAddr) -> Result<()> {
    // as the result data can be large and may not be sent in a single UDP datagram,
    // split it into smaller chunks
    for chunk in data.chunks(4096) {
        socket.send_to(chunk, dest_addr).await?;
    }
    // send an empty data at the end to indicate end of chunks
    socket.send_to(&[], dest_addr).await?;
    Ok(())
}

async fn handle_socket_request(
    client: &Client,
    state: &SharedState,
    request: super::Request,
    socket: &UdpSocket,
    dest_addr: SocketAddr,
) -> Result<()> {
    match request {
        Request::Get(GetRequest::Key(key)) => {
            let result = handle_get_key_request(client, key).await?;
            send_data(result, socket, dest_addr).await?;
        }
        Request::Get(GetRequest::Context(context_id, context_type)) => {
            let result = handle_get_context_request(client, context_id, context_type).await?;
            send_data(result, socket, dest_addr).await?;
        }
        Request::Playback(command) => {
            handle_playback_request(client, command).await?;
            client.update_playback(state);
        }
    }
    Ok(())
}

async fn handle_get_key_request(client: &Client, key: Key) -> Result<Vec<u8>> {
    Ok(match key {
        Key::Playback => {
            let playback = client
                .spotify
                .current_playback(None, None::<Vec<_>>)
                .await?;
            serde_json::to_vec(&playback)?
        }
        Key::Devices => {
            let devices = client.spotify.device().await?;
            serde_json::to_vec(&devices)?
        }
        Key::UserPlaylists => {
            let playlists = client.current_user_playlists().await?;
            serde_json::to_vec(&playlists)?
        }
        Key::UserLikedTracks => {
            let tracks = client.current_user_saved_tracks().await?;
            serde_json::to_vec(&tracks)?
        }
        Key::UserTopTracks => {
            let tracks = client.current_user_top_tracks().await?;
            serde_json::to_vec(&tracks)?
        }
        Key::UserSavedAlbums => {
            let albums = client.current_user_saved_albums().await?;
            serde_json::to_vec(&albums)?
        }
        Key::UserFollowedArtists => {
            let artists = client.current_user_followed_artists().await?;
            serde_json::to_vec(&artists)?
        }
        Key::Queue => {
            let queue = client.spotify.current_user_queue().await?;
            serde_json::to_vec(&queue)?
        }
    })
}

async fn handle_get_context_request(
    client: &Client,
    context_id: String,
    context_type: ContextType,
) -> Result<Vec<u8>> {
    let context = match context_type {
        ContextType::Playlist => {
            let id = PlaylistId::from_id(context_id)?;
            client.playlist_context(id).await?
        }
        ContextType::Album => {
            let id = AlbumId::from_id(context_id)?;
            client.album_context(id).await?
        }
        ContextType::Artist => {
            let id = ArtistId::from_id(context_id)?;
            client.artist_context(id).await?
        }
    };

    Ok(serde_json::to_vec(&context)?)
}

async fn handle_playback_request(client: &Client, command: Command) -> Result<()> {
    let playback = match client
        .spotify
        .current_playback(None, None::<Vec<_>>)
        .await?
    {
        Some(playback) => playback,
        None => {
            eprintln!("No playback found!");
            std::process::exit(1);
        }
    };
    let device_id = playback.device.id.as_deref();

    match command {
        Command::Start(context_id, context_type) => {
            let context_id = match context_type {
                ContextType::Playlist => PlayContextId::Playlist(PlaylistId::from_id(context_id)?),
                ContextType::Album => PlayContextId::Album(AlbumId::from_id(context_id)?),
                ContextType::Artist => PlayContextId::Artist(ArtistId::from_id(context_id)?),
            };

            client
                .spotify
                .start_context_playback(context_id, device_id, None, None)
                .await?;

            // for some reasons, when starting a new playback, the integrated `spotify-player`
            // client doesn't respect the initial shuffle state, so we need to manually update the state
            client
                .spotify
                .shuffle(playback.shuffle_state, device_id)
                .await?
        }
        Command::PlayPause => {
            if playback.is_playing {
                client.spotify.pause_playback(device_id).await?;
            } else {
                client.spotify.resume_playback(device_id, None).await?;
            }
        }
        Command::Next => {
            client.spotify.next_track(device_id).await?;
        }
        Command::Previous => {
            client.spotify.previous_track(device_id).await?;
        }
        Command::Shuffle => {
            client
                .spotify
                .shuffle(!playback.shuffle_state, device_id)
                .await?;
        }
        Command::Repeat => {
            let next_repeat_state = match playback.repeat_state {
                RepeatState::Off => RepeatState::Track,
                RepeatState::Track => RepeatState::Context,
                RepeatState::Context => RepeatState::Off,
            };

            client.spotify.repeat(next_repeat_state, device_id).await?;
        }
        Command::Volume(percent) => {
            client.spotify.volume(percent, device_id).await?;
        }
        Command::Seek(position_offset_ms) => {
            let progress_ms = match playback.progress {
                Some(progress) => progress.as_millis(),
                None => {
                    eprintln!("Playback has no progress!");
                    std::process::exit(1);
                }
            };
            client
                .spotify
                .seek_track(
                    (progress_ms as u32).saturating_add_signed(position_offset_ms),
                    device_id,
                )
                .await?;
        }
    }

    Ok(())
}
