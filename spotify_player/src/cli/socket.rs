use anyhow::{Context, Result};
use std::net::{SocketAddr, UdpSocket};

use rspotify::{model::*, prelude::OAuthClient};

use crate::{
    cli::{ContextType, Request},
    client::Client,
};

use super::*;

pub async fn start_socket(client: Client, port: u16) -> Result<()> {
    tracing::info!("Starting a client socket at 127.0.0.1:{port}");

    let socket = UdpSocket::bind(("127.0.0.1", port))
        .context(format!("failed to bind a new socket to port {port}"))?;

    // initialize the receive buffer to be 4096 bytes
    let mut buf = [0; 4096];
    loop {
        match socket.recv_from(&mut buf) {
            Err(err) => tracing::warn!("failed to receive from the socket: {err}"),
            Ok((n_bytes, dest_addr)) => {
                let request: Request = serde_json::from_slice(&buf[0..n_bytes])?;
                handle_socket_request(&client, request, &socket, dest_addr).await?;
            }
        }
    }
}

async fn handle_socket_request(
    client: &Client,
    request: super::Request,
    socket: &UdpSocket,
    dest_addr: SocketAddr,
) -> Result<()> {
    match request {
        Request::Get(GetRequest::Key(key)) => {
            let result = handle_get_key_request(client, key).await?;
            socket.send_to(&result, dest_addr)?;
        }
        Request::Get(GetRequest::Context(context_id, context_type)) => {
            let result = handle_get_context_request(client, context_id, context_type).await?;
            socket.send_to(&result, dest_addr)?;
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
