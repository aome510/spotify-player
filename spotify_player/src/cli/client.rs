use std::{
    collections::HashSet,
    fs::{create_dir_all, remove_dir_all},
    io::Write,
    net::SocketAddr,
};

use anyhow::{Context as _, Result};
use rand::seq::SliceRandom;
use tokio::net::UdpSocket;
use tracing::Instrument;

use crate::{
    cli::Request,
    client::{Client, PlayerRequest},
    config::get_cache_folder_path,
    state::{Context, ContextId, Playback, PlaybackMetadata, SharedState},
};
use rspotify::{
    model::*,
    prelude::{BaseClient, OAuthClient},
};

use super::*;

pub async fn start_socket(client: Client, socket: UdpSocket, state: Option<SharedState>) {
    let mut buf = [0; MAX_REQUEST_SIZE];

    loop {
        match socket.recv_from(&mut buf).await {
            Err(err) => tracing::warn!("Failed to receive from the socket: {err:#}"),
            Ok((n_bytes, dest_addr)) => {
                if n_bytes == 0 {
                    // received a connection request from the destination address
                    socket.send_to(&[], dest_addr).await.unwrap_or_default();
                    continue;
                }

                let req_buf = &buf[0..n_bytes];
                let request: Request = match serde_json::from_slice(req_buf) {
                    Ok(v) => v,
                    Err(err) => {
                        tracing::error!("Cannot deserialize the socket request: {err:#}");
                        continue;
                    }
                };

                let span = tracing::info_span!("socket_request", request = ?request, dest_addr = ?dest_addr);

                async {
                    let response = match handle_socket_request(&client, &state, request).await {
                        Err(err) => {
                            tracing::error!("Failed to handle socket request: {err:#}");
                            let msg = format!("Bad request: {err:#}");
                            Response::Err(msg.into_bytes())
                        }
                        Ok(data) => Response::Ok(data),
                    };
                    send_response(response, &socket, dest_addr)
                        .await
                        .unwrap_or_default();

                    tracing::info!("Successfully handled the socket request.",);
                }
                .instrument(span)
                .await;
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

async fn current_playback(
    client: &Client,
    state: &Option<SharedState>,
) -> Result<Option<CurrentPlaybackContext>> {
    // get current playback from the application's state, if exists, or by making an API request
    match state {
        Some(ref state) => Ok(state.player.read().current_playback()),
        None => client
            .current_playback(None, None::<Vec<_>>)
            .await
            .context("get current playback"),
    }
}

async fn handle_socket_request(
    client: &Client,
    state: &Option<SharedState>,
    request: super::Request,
) -> Result<Vec<u8>> {
    if let Some(state) = state {
        client.check_valid_session(state).await?;
    }

    match request {
        Request::Get(GetRequest::Key(key)) => handle_get_key_request(client, state, key).await,
        Request::Get(GetRequest::Item(item_type, id_or_name)) => {
            handle_get_item_request(client, item_type, id_or_name).await
        }
        Request::Playback(command) => {
            handle_playback_request(client, state, command).await?;
            Ok(Vec::new())
        }
        Request::Connect(data) => {
            let id = match data {
                IdOrName::Id(id) => id,
                IdOrName::Name(name) => {
                    let devices = client.device().await?;
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

            client.transfer_playback(&id, None).await?;
            Ok(Vec::new())
        }
        Request::Like { unlike } => {
            let playback = current_playback(client, state).await?;

            // get currently playing track from the playback
            let track = match playback {
                None => None,
                Some(ref playback) => match playback.item {
                    Some(rspotify::model::PlayableItem::Track(ref track)) => Some(track),
                    _ => None,
                },
            };

            if let Some(id) = track.and_then(|t| t.id.to_owned()) {
                if unlike {
                    client.current_user_saved_tracks_delete([id]).await?;
                } else {
                    client.current_user_saved_tracks_add([id]).await?;
                }
            }

            Ok(Vec::new())
        }
        Request::Playlist(command) => {
            let resp = handle_playlist_request(client, command).await?;
            Ok(resp.into_bytes())
        }
        Request::Search { query } => {
            let resp = handle_search_request(client, query).await?;
            Ok(resp)
        }
    }
}

async fn handle_get_key_request(
    client: &Client,
    state: &Option<SharedState>,
    key: Key,
) -> Result<Vec<u8>> {
    Ok(match key {
        Key::Playback => {
            let playback = current_playback(client, state).await?;
            serde_json::to_vec(&playback)?
        }
        Key::Devices => {
            let devices = client.device().await?;
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
            let queue = client.current_user_queue().await?;
            serde_json::to_vec(&queue)?
        }
    })
}

/// Get a Spotify item's ID from its `IdOrName` representation
async fn get_spotify_id(client: &Client, typ: ItemType, id_or_name: IdOrName) -> Result<ItemId> {
    // For `IdOrName::Name`, we search for the first item matching the name and return its Spotify id.
    // The item's id is then used to retrieve the item's data.

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

async fn handle_get_item_request(
    client: &Client,
    item_type: ItemType,
    id_or_name: IdOrName,
) -> Result<Vec<u8>> {
    let sid = get_spotify_id(client, item_type, id_or_name).await?;
    Ok(match sid {
        ItemId::Playlist(id) => serde_json::to_vec(&client.playlist_context(id).await?)?,
        ItemId::Album(id) => serde_json::to_vec(&client.album_context(id).await?)?,
        ItemId::Artist(id) => serde_json::to_vec(&client.artist_context(id).await?)?,
        ItemId::Track(id) => serde_json::to_vec(&client.track(id).await?)?,
    })
}

async fn handle_search_request(client: &Client, query: String) -> Result<Vec<u8>> {
    let search_result = client.search(&query).await?;

    Ok(serde_json::to_vec(&search_result)?)
}

async fn handle_playback_request(
    client: &Client,
    state: &Option<SharedState>,
    command: Command,
) -> Result<()> {
    let playback = match state {
        Some(state) => state.player.read().buffered_playback.clone(),
        None => {
            let playback = client.current_playback(None, None::<Vec<_>>).await?;
            playback.as_ref().map(PlaybackMetadata::from_playback)
        }
    };

    let player_request = match command {
        Command::StartRadio(item_type, id_or_name) => {
            let sid = get_spotify_id(client, item_type, id_or_name).await?;
            let tracks = client.radio_tracks(sid.uri()).await?;

            PlayerRequest::StartPlayback(
                Playback::URIs(tracks.into_iter().map(|t| t.id).collect(), None),
                None,
            )
        }
        Command::StartLikedTracks { limit, random } => {
            // get a list of liked tracks' ids
            let mut ids: Vec<_> = if let Some(ref state) = state {
                state
                    .data
                    .read()
                    .user_data
                    .saved_tracks
                    .values()
                    .map(|t| t.id.to_owned())
                    .collect()
            } else {
                client
                    .current_user_saved_tracks()
                    .await?
                    .into_iter()
                    .map(|t| t.id)
                    .collect()
            };

            if random {
                let mut rng = rand::thread_rng();
                ids.shuffle(&mut rng)
            }

            ids.truncate(limit);

            PlayerRequest::StartPlayback(Playback::URIs(ids, None), None)
        }
        Command::StartContext {
            context_type,
            id_or_name,
            shuffle,
        } => {
            let sid = get_spotify_id(client, context_type.into(), id_or_name).await?;
            let context_id = match sid {
                ItemId::Playlist(id) => ContextId::Playlist(id),
                ItemId::Album(id) => ContextId::Album(id),
                ItemId::Artist(id) => ContextId::Artist(id),
                _ => unreachable!(),
            };

            PlayerRequest::StartPlayback(Playback::Context(context_id, None), Some(shuffle))
        }
        Command::PlayPause => PlayerRequest::ResumePause,
        Command::Play => PlayerRequest::Resume,
        Command::Pause => PlayerRequest::Pause,
        Command::Next => PlayerRequest::NextTrack,
        Command::Previous => PlayerRequest::PreviousTrack,
        Command::Shuffle => PlayerRequest::Shuffle,
        Command::Repeat => PlayerRequest::Repeat,
        Command::Volume { percent, is_offset } => {
            let volume = playback
                .as_ref()
                .context("no active playback found!")?
                .volume
                .context("playback has no volume!")?;
            let percent = if is_offset {
                std::cmp::max(0, (volume as i8) + percent)
            } else {
                percent
            };
            PlayerRequest::Volume(percent.try_into()?)
        }
        Command::Seek(position_offset_ms) => {
            // Playback's progress cannot be computed trivially without knowing the `playback` variable in
            // the function scope is from the application's state (cached) or the `current_playback` API.
            // Therefore, we need to make an additional API request to get the playback's progress.
            let progress = client
                .current_playback(None, None::<Vec<_>>)
                .await?
                .context("no active playback found!")?
                .progress
                .context("playback has no progress!")?;
            PlayerRequest::SeekTrack(
                progress + chrono::Duration::try_milliseconds(position_offset_ms).unwrap(),
            )
        }
    };

    if let Some(ref state) = state {
        // A non-null application's state indicates there is a running application instance.
        // To reduce the latency of the CLI command, the player request is handled asynchronously
        // knowing that the application will outlive the asynchronous task.
        tokio::task::spawn({
            let client = client.clone();
            let state = state.clone();
            async move {
                match client.handle_player_request(player_request, playback).await {
                    Ok(playback) => {
                        // update application's states
                        state.player.write().buffered_playback = playback;
                        client.update_playback(&state);
                    }
                    Err(err) => {
                        tracing::warn!(
                            "Failed to handle a player request for playback CLI command: {err:#}"
                        );
                    }
                }
            }
        });
    } else {
        // Handles the player request synchronously
        client
            .handle_player_request(player_request, playback)
            .await?;
    }
    Ok(())
}

async fn handle_playlist_request(client: &Client, command: PlaylistCommand) -> Result<String> {
    let uid = client.current_user().await?.id;

    match command {
        PlaylistCommand::New {
            name,
            public,
            collab,
            description,
        } => {
            let resp = client
                .user_playlist_create(
                    uid,
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
            let following = client
                .playlist_check_follow(id.to_owned(), &[uid])
                .await
                .context(format!("Could not find playlist '{}'", id.id()))?
                .pop()
                .unwrap();

            // Won't delete if not following
            if following {
                client.playlist_unfollow(id.to_owned()).await?;
                Ok(format!("Playlist '{id}' was deleted/unfollowed"))
            } else {
                Ok(format!(
                    "Playlist '{id}' was not followed by the user, nothing to be done.",
                ))
            }
        }
        PlaylistCommand::List => {
            let resp = client.current_user_playlists().await?;

            let mut out = String::new();
            for pl in resp {
                out += &format!("{}: {}\n", pl.id.id(), pl.name);
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
            let from = client
                .playlist(id.to_owned(), None, None)
                .await
                .context(format!("Cannot import from {}.", id.id()))?;
            let from_desc = from.description.unwrap_or_default();

            let to = client
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
                to.id.id(),
                to.name
            );

            result += &playlist_import(client, id, to.id, false).await?;

            Ok(result)
        }
        PlaylistCommand::Sync { id, delete } => {
            // Get import dir/file
            let imports_dir = get_cache_folder_path()?.join("imports");

            let mut result = String::new();

            // Iterate through the playlist `imports` folder in the cache folder to
            // get all playlists' import data represented as subdirectories with `import_to` name.
            // Inside each `import_to` subdirectory, an import `import_from -> import_to`
            // data is represented as a file with `import_from` name.
            for dir in imports_dir.read_dir()? {
                let to_dir = dir?.path();
                let to_id = PlaylistId::from_id(to_dir.file_name().unwrap().to_str().unwrap())?;

                // If a playlist id is specified, only consider sync imports of that playlist
                if let Some(id) = &id {
                    if to_id != *id {
                        continue;
                    }
                }

                let pl_follow = client
                    .playlist_check_follow(to_id.as_ref(), &[uid.as_ref()])
                    .await?
                    .pop()
                    .unwrap();

                if pl_follow {
                    for i in to_dir.read_dir()? {
                        let from_id =
                            PlaylistId::from_id(i?.file_name().to_str().unwrap().to_owned())?;
                        result +=
                            &playlist_import(client, from_id, to_id.clone_static(), delete).await?;
                        result += "\n";
                    }
                } else {
                    remove_dir_all(&to_dir)?;
                    result += &format!(
                        "Not following playlist '{}'. Deleted its import data in the cache folder...\n",
                        to_id.id()
                    );
                }
            }

            Ok(result)
        }
    }
}

const TRACK_BUFFER_CAP: usize = 100;

/// Imports a playlist into another playlist.
///
/// All tracks from the `import_from` playlist are added to the `import_to` playlist if they are not in there already.
///
/// The state of `import_from` playlist is stored into a cache file to add/delete the differed tracks between
/// subsequent imports of the same two playlists.
async fn playlist_import(
    client: &Client,
    import_from: PlaylistId<'static>,
    import_to: PlaylistId<'static>,
    delete: bool,
) -> Result<String> {
    #[derive(Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
    struct TrackData {
        id: TrackId<'static>,
        name: String,
    }
    // Get playlists' info
    let (from_tracks, from_name) = match client.playlist_context(import_from.to_owned()).await? {
        Context::Playlist { tracks, playlist } => (
            tracks.into_iter().map(|t| TrackData {
                id: t.id,
                name: t.name,
            }),
            playlist.name,
        ),
        _ => unreachable!(),
    };
    let (to_tracks, to_name) = match client.playlist_context(import_to.to_owned()).await? {
        Context::Playlist { tracks, playlist } => (
            tracks.into_iter().map(|t| TrackData {
                id: t.id,
                name: t.name,
            }),
            playlist.name,
        ),
        _ => unreachable!(),
    };

    // Get import dir/file
    let cache_dir = get_cache_folder_path()?;
    let imports_dir = cache_dir.join("imports");
    let to_dir = imports_dir.join(import_to.id());
    let from_file = to_dir.join(import_from.id());

    if !to_dir.exists() {
        create_dir_all(to_dir)?;
    }
    // Construct hash sets of the playlists' track IDs
    let to_hash_set: HashSet<TrackData> = HashSet::from_iter(to_tracks);
    let from_hash_set: HashSet<TrackData> = HashSet::from_iter(from_tracks);

    let mut new_tracks_hash_set = &from_hash_set - &to_hash_set;

    let mut result = String::new();
    result += &format!(
        "Importing from {}:{} to {}:{}...\n",
        import_from.id(),
        from_name,
        import_to.id(),
        to_name
    );

    let mut track_buff = Vec::new();
    if from_file.exists() {
        let hash_set_bytes = std::fs::read(&from_file).context(format!(
            "Read cached playlist import data from {}",
            from_file.display()
        ))?;
        std::fs::remove_file(&from_file)?;
        let old_from_hash_set: HashSet<TrackData> =
            serde_json::from_slice(&hash_set_bytes).context("Deserialize playlist import data")?;

        // Only consider new tracks that were not included in the previous import.
        new_tracks_hash_set = &new_tracks_hash_set - &old_from_hash_set;

        // If `delete` option is specified, delete previously imported tracks that are not in the current `from` playlist
        let deleted_hash_set = &old_from_hash_set - &from_hash_set;
        if delete {
            for t in &deleted_hash_set {
                track_buff.push(PlayableId::Track(t.id.as_ref()));

                if track_buff.len() == TRACK_BUFFER_CAP {
                    client
                        .playlist_remove_all_occurrences_of_items(
                            import_to.as_ref(),
                            track_buff,
                            None,
                        )
                        .await?;
                    track_buff = Vec::new();
                }
            }

            if !track_buff.is_empty() {
                client
                    .playlist_remove_all_occurrences_of_items(import_to.as_ref(), track_buff, None)
                    .await?;
            }
            result += &format!("Tracks deleted from {from_name}: \n");
        } else {
            result += &format!("Tracks that are no longer in {from_name} since last import: \n");
        }

        for t in &deleted_hash_set {
            result += &format!("    {}: {}\n", t.id.id(), t.name);
        }
    }

    result += &format!("New tracks imported to {to_name}: \n");

    track_buff = Vec::new();
    for t in &new_tracks_hash_set {
        track_buff.push(PlayableId::Track(t.id.as_ref()));

        if track_buff.len() == TRACK_BUFFER_CAP {
            client
                .playlist_add_items(import_to.as_ref(), track_buff, None)
                .await?;
            track_buff = Vec::new();
        }

        result += &format!("    {}: {}\n", t.id.id(), t.name);
    }

    if !track_buff.is_empty() {
        client
            .playlist_add_items(import_to.as_ref(), track_buff, None)
            .await?;
    }

    // Create a new cache file storing the latest import data
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&from_file)?;
    let hash_set_bytes =
        serde_json::to_vec(&from_hash_set).context("Serialize new playlist import data")?;
    f.write_all(&hash_set_bytes).context(format!(
        "Save new playlist import data into a cache file {}",
        from_file.display()
    ))?;

    Ok(result)
}
