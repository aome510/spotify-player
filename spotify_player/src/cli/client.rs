use std::net::SocketAddr;

use anyhow::Result;
use rand::seq::SliceRandom;
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
                let response = match handle_socket_request(&client, &state, request).await {
                    Err(err) => {
                        tracing::error!("Failed to handle socket request: {err}");
                        let msg = format!("Bad request: {err}");
                        Response::Err(msg.into_bytes())
                    }
                    Ok(data) => Response::Ok(data),
                };
                send_response(response, &socket, dest_addr)
                    .await
                    .unwrap_or_default();
            }
        }
    }
}

async fn send_response(
    response: Response,
    socket: &UdpSocket,
    dest_addr: SocketAddr,
) -> Result<()> {
    let data = serde_json::to_vec(&response)?;

    // as the result data can be large and may not be sent in a single UDP datagram,
    // split it into smaller chunks
    for chunk in data.chunks(4096) {
        socket.send_to(chunk, dest_addr).await?;
    }
    // send an empty buffer to indicate end of chunk
    socket.send_to(&[], dest_addr).await?;
    Ok(())
}

async fn handle_socket_request(
    client: &Client,
    state: &SharedState,
    request: super::Request,
) -> Result<Vec<u8>> {
    if client.spotify.session().await.is_invalid() {
        tracing::info!("Spotify client's session is invalid, re-creating a new session...");
        client.new_session(state).await?;
    }

    match request {
        Request::Get(GetRequest::Key(key)) => handle_get_key_request(client, key).await,
        Request::Get(GetRequest::Context(context_type, context_id)) => {
            handle_get_context_request(client, context_type, context_id).await
        }
        Request::Playback(command) => {
            handle_playback_request(client, state, command).await?;
            Ok(Vec::new())
        }
        Request::Connect(data) => {
            let id = match data {
                IdOrName::Id(id) => id,
                IdOrName::Name(name) => {
                    let devices = client.spotify.device().await?;
                    match devices
                        .into_iter()
                        .find(|d| d.name == name)
                        .and_then(|d| d.id)
                    {
                        Some(id) => id,
                        None => {
                            anyhow::bail!("No device with name={name} found");
                        }
                    }
                }
            };

            client.spotify.transfer_playback(&id, None).await?;
            Ok(Vec::new())
        }
    }
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

/// Get a Spotify item's ID from its `IdOrName` representation
async fn get_spotify_id(client: &Client, typ: ItemType, id_or_name: IdOrName) -> Result<ItemId> {
    // For `cli::ContextId::Name`, we search for the first item matching the name and return its spotify id

    let sid = match typ {
        ItemType::Playlist => match id_or_name {
            IdOrName::Id(id) => ItemId::Playlist(PlaylistId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Playlist)
                    .await?;

                match results {
                    SearchResult::Playlists(page) => {
                        if !page.items.is_empty() {
                            ItemId::Playlist(page.items[0].id.to_owned())
                        } else {
                            anyhow::bail!("Cannot find playlist with name='{name}'");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        },
        ItemType::Album => match id_or_name {
            IdOrName::Id(id) => ItemId::Album(AlbumId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Album)
                    .await?;

                match results {
                    SearchResult::Albums(page) => {
                        if !page.items.is_empty() && page.items[0].id.is_some() {
                            ItemId::Album(page.items[0].id.to_owned().unwrap())
                        } else {
                            anyhow::bail!("Cannot find album with name='{name}'");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        },
        ItemType::Artist => match id_or_name {
            IdOrName::Id(id) => ItemId::Artist(ArtistId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Artist)
                    .await?;

                match results {
                    SearchResult::Artists(page) => {
                        if !page.items.is_empty() {
                            ItemId::Artist(page.items[0].id.to_owned())
                        } else {
                            anyhow::bail!("Cannot find artist with name='{name}'");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        },
        ItemType::Track => match id_or_name {
            IdOrName::Id(id) => ItemId::Track(TrackId::from_id(id)?),
            IdOrName::Name(name) => {
                let results = client
                    .search_specific_type(&name, SearchType::Track)
                    .await?;

                match results {
                    SearchResult::Tracks(page) => {
                        if !page.items.is_empty() && page.items[0].id.is_some() {
                            ItemId::Track(page.items[0].id.to_owned().unwrap())
                        } else {
                            anyhow::bail!("Cannot find track with name='{name}'");
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
    let sid = get_spotify_id(client, context_type.into(), context_id).await?;
    let context = match sid {
        ItemId::Playlist(id) => client.playlist_context(id).await?,
        ItemId::Album(id) => client.album_context(id).await?,
        ItemId::Artist(id) => client.artist_context(id).await?,
        _ => unreachable!(),
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
        Command::StartRadio(item_type, id_or_name) => {
            let sid = get_spotify_id(client, item_type, id_or_name).await?;
            let tracks = client.radio_tracks(sid.uri()).await?;

            client
                .spotify
                .start_uris_playback(
                    tracks.into_iter().map(|t| PlayableId::from(t.id)),
                    device_id,
                    None,
                    None,
                )
                .await?;
        }
        Command::StartLikedTracks { limit, random } => {
            let mut tracks = client.current_user_saved_tracks().await?;

            if random {
                let mut rng = rand::thread_rng();
                tracks.shuffle(&mut rng)
            }

            let ids = if tracks.len() > limit {
                tracks[0..limit].iter()
            } else {
                tracks.iter()
            }
            .map(|t| PlayableId::from(t.id.to_owned()));

            client
                .spotify
                .start_uris_playback(ids, device_id, None, None)
                .await?;
        }
        Command::StartContext(context_type, context_id) => {
            let sid = get_spotify_id(client, context_type.into(), context_id).await?;
            let context_id = match sid {
                ItemId::Playlist(id) => PlayContextId::Playlist(id),
                ItemId::Album(id) => PlayContextId::Album(id),
                ItemId::Artist(id) => PlayContextId::Artist(id),
                _ => unreachable!(),
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
        Command::Volume { percent, is_offset } => {
            let percent = if is_offset {
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
        Command::Like { unlike } => {
            if let Some(PlayableItem::Track(t)) = playback.item {
                if let Some(id) = t.id {
                    if unlike {
                        client
                            .spotify
                            .current_user_saved_tracks_delete([id])
                            .await?;
                    } else {
                        client.spotify.current_user_saved_tracks_add([id]).await?;
                    }
                }
            }
        }
    }

    Ok(())
}
