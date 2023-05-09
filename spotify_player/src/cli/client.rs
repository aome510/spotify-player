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

/// Context's spotify ID
enum ContextSid {
    Playlist(PlaylistId<'static>),
    Artist(ArtistId<'static>),
    Album(AlbumId<'static>),
}

pub async fn start_socket(client: Client, state: SharedState) -> Result<()> {
    let port = state.app_config.client_port;
    tracing::info!("Starting a client socket at 127.0.0.1:{port}");

    let socket = UdpSocket::bind(("127.0.0.1", port)).await?;

    // initialize the receive buffer to be 4096 bytes
    let mut buf = [0; 4096];
    loop {
        match socket.recv_from(&mut buf).await {
            Err(err) => tracing::warn!("Failed to receive from the socket: {err}"),
            Ok((n_bytes, dest_addr)) => {
                let req_buf = &buf[0..n_bytes];
                let request: Request = match serde_json::from_slice(req_buf) {
                    Ok(v) => v,
                    Err(err) => {
                        tracing::error!("Cannot deserialize the socket request: {err}");
                        continue;
                    }
                };
                tracing::info!("Handling socket request: {request:?}...");
                if let Err(err) =
                    handle_socket_request(&client, &state, request, &socket, dest_addr).await
                {
                    tracing::error!("Failed to handle socket request: {err}");
                }
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

async fn send_err_message(
    err: anyhow::Error,
    socket: &UdpSocket,
    dest_addr: SocketAddr,
) -> Result<()> {
    let msg = format!("Bad request: {err}");
    send_data(msg.into_bytes(), socket, dest_addr).await
}

async fn handle_socket_request(
    client: &Client,
    state: &SharedState,
    request: super::Request,
    socket: &UdpSocket,
    dest_addr: SocketAddr,
) -> Result<()> {
    match request {
        Request::Get(GetRequest::Key(key)) => match handle_get_key_request(client, key).await {
            Ok(result) => send_data(result, socket, dest_addr).await?,
            Err(err) => send_err_message(err, socket, dest_addr).await?,
        },
        Request::Get(GetRequest::Context(context_type, context_id)) => {
            match handle_get_context_request(client, context_type, context_id).await {
                Ok(result) => send_data(result, socket, dest_addr).await?,
                Err(err) => send_err_message(err, socket, dest_addr).await?,
            }
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

/// Get a context's Spotify ID from its `cli::ContextId` representation
async fn get_spotify_id_from_context_id(
    client: &Client,
    context_type: ContextType,
    context_id: IdOrName,
) -> Result<ContextSid> {
    // For `cli::ContextId::Name`, we search for the first item matching the name and return its spotify id

    let sid = match context_type {
        ContextType::Playlist => match context_id {
            IdOrName::Id(id) => ContextSid::Playlist(PlaylistId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Playlist)
                    .await?;

                match results {
                    SearchResult::Playlists(page) => {
                        if !page.items.is_empty() {
                            ContextSid::Playlist(page.items[0].id.to_owned())
                        } else {
                            anyhow::bail!("Cannot find playlist with name='{name}'");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        },
        ContextType::Album => match context_id {
            IdOrName::Id(id) => ContextSid::Album(AlbumId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Album)
                    .await?;

                match results {
                    SearchResult::Albums(page) => {
                        if !page.items.is_empty() && page.items[0].id.is_some() {
                            ContextSid::Album(page.items[0].id.to_owned().unwrap())
                        } else {
                            anyhow::bail!("Cannot find album with name='{name}'");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        },
        ContextType::Artist => match context_id {
            IdOrName::Id(id) => ContextSid::Artist(ArtistId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Artist)
                    .await?;

                match results {
                    SearchResult::Artists(page) => {
                        if !page.items.is_empty() {
                            ContextSid::Artist(page.items[0].id.to_owned())
                        } else {
                            anyhow::bail!("Cannot find artist with name='{name}'");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        },
    };

    Ok(sid)
}

async fn handle_get_context_request(
    client: &Client,
    context_type: ContextType,
    context_id: IdOrName,
) -> Result<Vec<u8>> {
    let sid = get_spotify_id_from_context_id(client, context_type, context_id).await?;
    let context = match sid {
        ContextSid::Playlist(id) => client.playlist_context(id).await?,
        ContextSid::Album(id) => client.album_context(id).await?,
        ContextSid::Artist(id) => client.artist_context(id).await?,
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
            anyhow::bail!("No playback found!");
        }
    };
    let device_id = playback.device.id.as_deref();

    match command {
        Command::Start(context_type, context_id) => {
            let sid = get_spotify_id_from_context_id(client, context_type, context_id).await?;
            let context_id = match sid {
                ContextSid::Playlist(id) => PlayContextId::Playlist(id),
                ContextSid::Album(id) => PlayContextId::Album(id),
                ContextSid::Artist(id) => PlayContextId::Artist(id),
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
        Command::Volume(percent, offset) => {
            let percent = if offset {
                std::cmp::max(
                    0,
                    (playback.device.volume_percent.unwrap_or_default() as i8) + percent,
                )
            } else {
                percent
            };

            client
                .spotify
                .volume(percent.try_into()?, device_id)
                .await?;
        }
        Command::Seek(position_offset_ms) => {
            let progress = match playback.progress {
                Some(progress) => progress,
                None => {
                    anyhow::bail!("Playback has no progress!");
                }
            };
            client
                .spotify
                .seek_track(
                    progress + chrono::Duration::milliseconds(position_offset_ms),
                    device_id,
                )
                .await?;
        }
    }

    Ok(())
}
