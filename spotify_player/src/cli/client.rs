use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    fs::{create_dir_all, remove_dir_all, remove_file, OpenOptions},
    hash::Hasher,
    io::BufReader,
    net::SocketAddr,
};

use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use tokio::net::UdpSocket;

use crate::{
    cli::{ContextType, Request},
    client::Client,
    config::get_config_folder_path,
    event::PlayerRequest,
    state::{Context, ContextId, Playback, SharedState},
};
use rspotify::{
    model::*,
    prelude::{BaseClient, OAuthClient},
};
use std::io::BufRead;
use std::io::Write;

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
            Ok(resp.into_bytes())
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
    };

    client.handle_player_request(state, player_request).await?;
    client.update_playback(state);
    Ok(())
}

async fn handle_playlist_request(
    client: &Client,
    state: &SharedState,
    command: PlaylistCommand,
) -> Result<String> {
    match command {
        PlaylistCommand::New {
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
                resp.name, resp.id
            ))
        }
        PlaylistCommand::Delete { id } => {
            let user = state.data.read().user_data.user.to_owned().unwrap();
            let uid = user.id;

            let playlist_check = client
                .spotify
                .playlist_check_follow(id.to_owned(), &[uid])
                .await;
            if playlist_check.is_err() {
                anyhow::bail!("Could not find playlist {}", id)
            }

            // Won't delete if not following
            let following = playlist_check.unwrap().pop().unwrap();
            if following {
                client.spotify.playlist_unfollow(id.to_owned()).await?;
                Ok(format!("Playlist '{}' was deleted/unfollowed", id))
            } else {
                Ok(format!(
                    "Playlist '{}' was not followed by the user, nothing to be done.",
                    id
                ))
            }
        }
        PlaylistCommand::List => {
            let resp = client.current_user_playlists().await?;

            let mut out = String::new();
            for pl in resp {
                // Might want to add color
                out.push_str(format!("{}: {}\n", pl.id.id(), pl.name).as_str());
            }
            out = out.trim().to_string();

            Ok(out)
        }
        PlaylistCommand::Import {
            from: import_from,
            to: import_to,
            delete,
        } => playlist_import(client, import_from, import_to, delete).await,
        PlaylistCommand::Fork { id } => {
            let user = state.data.read().user_data.user.to_owned().unwrap();
            let uid = user.id;

            let from = client
                .spotify
                .playlist(id.to_owned(), None, None)
                .await
                .unwrap_or_else(|_| panic!("Cannot import from {}. Playlist not found.", id.id()));
            let from_desc = from.description.unwrap_or("".to_owned());

            let to = client
                .spotify
                .user_playlist_create(
                    uid,
                    &from.name,
                    from.public,
                    Some(from.collaborative),
                    Some(from_desc.as_str()),
                )
                .await?;
            let mut result = format!(
                "Forked {}.\nNew playlist: {}:{}\n",
                id.id(),
                to.name,
                to.id.id()
            );

            result.push_str(playlist_import(client, id, to.id, false).await?.as_str());

            Ok(result)
        }
        PlaylistCommand::Update { id, delete } => {
            let user = state.data.read().user_data.user.to_owned().unwrap();
            let uid = user.id;

            // Get import dir/file
            let conf_dir = get_config_folder_path()?;
            let imports_dir = conf_dir.join("imports");

            // If use specific id option
            if id.is_some() {
                let to_id = id.unwrap();
                let to_dir = imports_dir.join(to_id.id());

                let pl_follow = client
                    .spotify
                    .playlist_check_follow(to_id.to_owned(), &[uid])
                    .await?
                    .pop()
                    .unwrap();

                // Import is useless if not following your own playlist
                if pl_follow {
                    // Must have imported to update
                    if to_dir.exists() {
                        let mut result = String::new();

                        for dir in to_dir.read_dir()? {
                            let from_id =
                                PlaylistId::from_id(dir?.file_name().into_string().unwrap())?;

                            // Add each import's output
                            result.push_str(
                                playlist_import(client, to_id.to_owned(), from_id, delete)
                                    .await?
                                    .as_str(),
                            );
                            result.push('\n');
                        }
                        Ok(result)
                    } else {
                        Err(anyhow!("No imports found for '{}'", to_id.id()))
                    }
                } else {
                    Ok(format!("Not following '{}'", to_id.id()))
                }
            } else {
                // Updating all imports

                let mut result = String::new();

                let dirs = imports_dir.read_dir()?;
                for dir in dirs {
                    let dir_path = dir?.path();
                    let dir_name = dir_path.file_name().unwrap().to_str().unwrap();

                    let to_id = PlaylistId::from_id(dir_name.to_owned())?;

                    let pl_follow = client
                        .spotify
                        .playlist_check_follow(to_id.to_owned(), &[uid.to_owned()])
                        .await?
                        .pop()
                        .unwrap();

                    // No import for non following playlist
                    if pl_follow {
                        let to_dir = imports_dir.join(dir_name);
                        for file in to_dir.read_dir()? {
                            let file_name = file?.file_name().into_string().unwrap();

                            let from_id = PlaylistId::from_id(file_name)?;

                            result.push_str(
                                playlist_import(client, from_id, to_id.to_owned(), delete)
                                    .await?
                                    .as_str(),
                            );
                            result.push('\n');
                        }
                    } else {
                        // Remove non following imports as they are now useless
                        remove_dir_all(dir_path)?;
                        result.push_str(
                            format!(
                                "Not following playlist '{}'. Deleting import...\n",
                                to_id.id()
                            )
                            .as_str(),
                        )
                    }
                }

                Ok(result)
            }
        }
    }
}

/// Imports a playlist into another playlist.
/// All tracks from the imported playlist are added to the import-to playlist if they are not in there already.
/// The state of the imported playlist and a hash of the track ids are written to a file
///
/// This file is used in subsequent imports of the same two playlists
/// It is first used to see if the state of the imported playlist has changed
/// then if so, is used to see what has differed in the change.
async fn playlist_import(
    client: &Client,
    import_from: PlaylistId<'static>,
    import_to: PlaylistId<'static>,
    delete: bool,
) -> Result<String> {
    #[derive(PartialEq, Eq, Hash)]
    struct PlaylistData {
        id: TrackId<'static>,
        name: String,
    }

    // Get playlists' info, checking if they exist
    let (from, from_tracks) = match client.playlist_context(import_from.to_owned()).await? {
        Context::Playlist { playlist, tracks } => (playlist, tracks),
        _ => panic!(
            "Cannot import from {}. Playlist not found.",
            import_from.id()
        ),
    };

    let (to, to_tracks) = match client.playlist_context(import_to.to_owned()).await? {
        Context::Playlist { playlist, tracks } => (playlist, tracks),
        _ => panic!("Cannot import to {}. Playlist not found.", import_to.id()),
    };

    // Create hashset of import_to playlist
    // Will use to check before adding track
    let mut to_hash = HashSet::new();
    for track in to_tracks {
        to_hash.insert(track.id);
    }

    // Create hash and vec of 'from_ids'
    let mut hasher = DefaultHasher::new();
    let mut from_ids = Vec::new();
    for track in from_tracks {
        let song = PlaylistData {
            id: track.id.to_owned(),
            name: track.name,
        };

        from_ids.push(song);
        hasher.write(track.id.id().as_bytes());
    }
    let hash = hasher.finish().to_string();

    // Get import dir/file
    let conf_dir = get_config_folder_path()?;
    let imports_dir = conf_dir.join("imports");
    let to_dir = imports_dir.join(import_to.id());
    let from_file = to_dir.join(import_from.id());

    if !to_dir.exists() {
        create_dir_all(to_dir)?;
    }

    // If this import has not happened before
    if !from_file.exists() {
        let num_songs = from_ids.len();

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&from_file)?;

        // Write hash and new line to top of file
        writeln!(file, "{}", hash)?;
        writeln!(file)?;

        let mut track_buff = Vec::new();
        for track in from_ids {
            writeln!(file, "{}:{}", track.id.id(), track.name)?;

            if !to_hash.contains(&track.id) {
                track_buff.push(PlayableId::Track(track.id));

                if track_buff.len() > 90 {
                    client
                        .spotify
                        .playlist_add_items(import_to.to_owned(), track_buff, None)
                        .await?;
                    track_buff = Vec::new();
                }
            }
        }

        if !track_buff.is_empty() {
            client
                .spotify
                .playlist_add_items(import_to.to_owned(), track_buff, None)
                .await?;
        }

        Ok(format!(
            "Successfully imported '{}' into '{}'.\n{} songs were added.",
            from.name, to.name, num_songs
        ))
    } else {
        // If the import has happened before, read from hash to check from changes
        // If the hashes don't match reimport

        let file = OpenOptions::new().read(true).open(&from_file)?;
        let reader = BufReader::new(file);
        let mut iter = reader.lines();

        let old_hash = iter.next().unwrap()?;

        // Consume empty line
        iter.next();

        // If old hash doesn't match,
        // need to do import
        if !old_hash.eq(&hash) {
            let mut old_ids = HashSet::new();
            let mut new_ids = HashSet::new();

            // Write playlists new info to file
            remove_file(&from_file)?; // Writing wasn't overwriting
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(&from_file)?;

            writeln!(file, "{}", hash)?;
            writeln!(file)?;

            for track in from_ids {
                let id = track.id.to_owned();

                writeln!(file, "{}:{}", id.id(), track.name)?;

                // Add tracks to hashset
                new_ids.insert(track);

                if !to_hash.contains(&id) {
                    client
                        .spotify
                        .playlist_add_items(import_to.to_owned(), [PlayableId::Track(id)], None)
                        .await?;
                }
            }

            // Read from old file, getting id and name
            while let Some(Ok(line)) = iter.next() {
                let mut split = line.trim().split(':');
                let id_s = split.next().unwrap().to_string();
                let name = split.next().unwrap().to_string();

                let id = TrackId::from_id(id_s.to_owned())?;
                let track = PlaylistData { id, name };

                old_ids.insert(track);
            }

            let mut result = String::new();

            // Add all new tracks
            let mut track_buff = Vec::new();
            let new_tracks = new_ids.difference(&old_ids);
            let mut new_tracks_count = 0;
            for track in new_tracks {
                if !to_hash.contains(&track.id) {
                    track_buff.push(PlayableId::Track(track.id.to_owned()));
                    new_tracks_count += 1;

                    if track_buff.len() > 90 {
                        client
                            .spotify
                            .playlist_add_items(import_to.to_owned(), track_buff, None)
                            .await?;
                        track_buff = Vec::new();
                    }
                }
            }

            if !track_buff.is_empty() {
                client
                    .spotify
                    .playlist_add_items(import_to.to_owned(), track_buff, None)
                    .await?;
            }

            result.push_str(
                format!(
                    "Updated the import '{}' for '{}'.\nAdded '{}' new songs.\n",
                    from.name, to.name, new_tracks_count
                )
                .as_str(),
            );

            // This is meant to prompt for deletion,
            // May not be able to do input/output because of
            // client and cli structure
            let mut delete_buff = Vec::new();
            let mut have_removed = false;
            let deleted_tracks = old_ids.difference(&new_ids);
            for track in deleted_tracks {
                if !have_removed {
                    result.push_str("The import has deleted these tracks:\n");
                    have_removed = true;
                }

                result.push_str(format!("{}:{}\n", track.id.id(), track.name).as_str());
                if delete {
                    delete_buff.push(PlayableId::Track(track.id.to_owned()));
                    if delete_buff.len() > 90 {
                        client.spotify.playlist_remove_all_occurrences_of_items(to.id.to_owned(), delete_buff, None).await?;
                        delete_buff = Vec::new();

                    }
                }
            }

            if delete {
                result.push_str("These tracks have been deleted from the playlist.");
                client.spotify.playlist_remove_all_occurrences_of_items(to.id, delete_buff, None).await?;

            }

            if !have_removed {
                result.push('\n');
            }

            Ok(result)
        } else {
            Ok(format!(
                "No updates to the import '{}' for '{}'\n",
                from.name, to.name
            ))
        }
    }
}
