use std::net::SocketAddr;

use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use tokio::net::UdpSocket;

use rspotify::{
    model::*,
    prelude::{BaseClient, OAuthClient},
};

use crate::{
    cli::{ContextType, Request},
    client::Client,
    event::PlayerRequest,
    state::{ContextId, Playback, SharedState},
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
            Err(err) => tracing::warn!("Failed to receive from the socket: {err:#}"),
            Ok((n_bytes, dest_addr)) => {
                let req_buf = &buf[0..n_bytes];
                let request: Request = match serde_json::from_slice(req_buf) {
                    Ok(v) => v,
                    Err(err) => {
                        tracing::error!("Cannot deserialize the socket request: {err:#}");
                        continue;
                    }
                };

                tracing::info!("Handling socket request: {request:?}...");
                let response = match handle_socket_request(&client, &state, request).await {
                    Err(err) => {
                        tracing::error!("Failed to handle socket request: {err:#}");
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
        Request::Like { unlike } => {
            let id = state
                .player
                .read()
                .current_playing_track()
                .and_then(|t| t.id.to_owned());

            if let Some(id) = id {
                if unlike {
                    client
                        .spotify
                        .current_user_saved_tracks_delete([id])
                        .await?;
                } else {
                    client.spotify.current_user_saved_tracks_add([id]).await?;
                }
            }

            Ok(Vec::new())
        }
        Request::Playlist(command) => {
            let resp = handle_playlist_request(client, state, command).await?;

            // Allowing command output of success
            let mut resp_vec = serde_json::to_vec_pretty(&resp)?;
            // Removing quotation marks
            resp_vec.pop();
            resp_vec.remove(0);
            Ok(resp_vec)
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

async fn handle_playback_request(
    client: &Client,
    state: &SharedState,
    command: Command,
) -> Result<()> {
    let player_request = match command {
        Command::StartRadio(item_type, id_or_name) => {
            let sid = get_spotify_id(client, item_type, id_or_name).await?;
            let tracks = client.radio_tracks(sid.uri()).await?;

            PlayerRequest::StartPlayback(Playback::URIs(
                tracks.into_iter().map(|t| t.id).collect(),
                None,
            ))
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
            .map(|t| t.id.to_owned())
            .collect();

            PlayerRequest::StartPlayback(Playback::URIs(ids, None))
        }
        Command::StartContext(context_type, context_id) => {
            let sid = get_spotify_id(client, context_type.into(), context_id).await?;
            let context_id = match sid {
                ItemId::Playlist(id) => ContextId::Playlist(id),
                ItemId::Album(id) => ContextId::Album(id),
                ItemId::Artist(id) => ContextId::Artist(id),
                _ => unreachable!(),
            };

            PlayerRequest::StartPlayback(Playback::Context(context_id, None))
        }
        Command::PlayPause => PlayerRequest::ResumePause,
        Command::Next => PlayerRequest::NextTrack,
        Command::Previous => PlayerRequest::PreviousTrack,
        Command::Shuffle => PlayerRequest::Shuffle,
        Command::Repeat => PlayerRequest::Repeat,
        Command::Volume { percent, is_offset } => match state.player.read().buffered_playback {
            Some(ref playback) => {
                let percent = if is_offset {
                    std::cmp::max(0, (playback.volume.unwrap_or_default() as i8) + percent)
                } else {
                    percent
                };
                PlayerRequest::Volume(percent.try_into()?)
            }
            None => anyhow::bail!("No playback found!"),
        },
        Command::Seek(position_offset_ms) => {
            let progress = match state.player.read().playback_progress() {
                Some(progress) => progress,
                None => {
                    anyhow::bail!("Playback has no progress!");
                }
            };
            PlayerRequest::SeekTrack(progress + chrono::Duration::milliseconds(position_offset_ms))
        }
        _ => unreachable!(),
    };

    client.handle_player_request(state, player_request).await?;
    client.update_playback(state);
    Ok(())
}

async fn handle_playlist_request(
    client: &Client,
    state: &SharedState,
    command: Command,
) -> Result<String> {
    match command {
        Command::PlaylistNew {
            name,
            public,
            collab,
            description,
        } => {
            let user = state.data.read().user_data.user.to_owned().unwrap();
            let id = user.id;

            let resp = client
                .spotify
                .user_playlist_create(
                    id,
                    name.as_str(),
                    Some(public),
                    Some(collab),
                    Some(description.as_str()),
                )
                .await?;
            Ok(format!(
                "Playlist '{}' with id '{}' was created.",
                resp.name,
                resp.id.to_string()
            ))
        }
        Command::PlaylistDelete { id } => {
            let user = state.data.read().user_data.user.to_owned().unwrap();
            let uid = user.id;

            let playlist_check = client
                .spotify
                .playlist_check_follow(id.to_owned(), &[uid])
                .await;
            if playlist_check.is_err() {
                anyhow::bail!("Could not find playlist {}", id.to_string())
            }

            let following = playlist_check.unwrap().pop();
            if following.unwrap() {
                client.spotify.playlist_unfollow(id.to_owned()).await?;
                Ok(format!("'{}' was deleted", id.to_string()))
            } else {
                Ok(format!("'{}' was not followed.", id.to_string()))
            }
        }
        _ => unreachable!(),
    }
}
