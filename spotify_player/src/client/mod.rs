use std::ops::Deref;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

use crate::config;
use crate::{auth::AuthConfig, state::*};

use anyhow::Context as _;
use anyhow::Result;
use librespot_core::session::Session;
use rspotify::{
    http::Query,
    model::{FullPlaylist, Market, Page, SimplifiedPlaylist},
    prelude::*,
};

mod handlers;
mod request;
mod spotify;

pub use handlers::*;
pub use request::*;
use serde::Deserialize;

const SPOTIFY_API_ENDPOINT: &str = "https://api.spotify.com/v1";

/// The application's Spotify client
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    spotify: Arc<spotify::Spotify>,
    auth_config: AuthConfig,
    #[cfg(feature = "streaming")]
    stream_conn: Arc<Mutex<Option<librespot_connect::spirc::Spirc>>>,
}

impl Deref for Client {
    type Target = spotify::Spotify;
    fn deref(&self) -> &Self::Target {
        self.spotify.as_ref()
    }
}

fn market_query() -> Query<'static> {
    Query::from([("market", "from_token")])
}

impl Client {
    /// Construct a new client
    pub fn new(session: Session, auth_config: AuthConfig, client_id: String) -> Self {
        Self {
            spotify: Arc::new(spotify::Spotify::new(session, client_id)),
            http: reqwest::Client::new(),
            auth_config,

            #[cfg(feature = "streaming")]
            stream_conn: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new client session
    // unused variables:
    // - `state` when the `streaming` feature is not enabled
    #[allow(unused_variables)]
    async fn new_session(&self, state: &SharedState) -> Result<()> {
        let session = crate::auth::new_session(&self.auth_config, false).await?;
        *self.session.lock().await = Some(session);

        tracing::info!("Used a new session for Spotify client.");

        // upon creating a new session, also create a new streaming connection
        #[cfg(feature = "streaming")]
        if state.is_streaming_enabled() {
            self.new_streaming_connection(state).await;
            // handle `connect_device` task separately as we don't want to block here
            tokio::task::spawn({
                let client = self.clone();
                let state = state.clone();
                async move {
                    client.connect_device(&state).await;
                }
            });
        }

        Ok(())
    }

    /// Check if the current session is valid and if invalid, create a new session
    pub async fn check_valid_session(&self, state: &SharedState) -> Result<()> {
        if self.session().await.is_invalid() {
            tracing::info!("Client's current session is invalid, creating a new session...");
            self.new_session(state)
                .await
                .context("create new client session")?;
        }
        Ok(())
    }

    /// Create a new streaming connection
    #[cfg(feature = "streaming")]
    pub async fn new_streaming_connection(&self, state: &SharedState) {
        let new_conn = crate::streaming::new_connection(self.clone(), state.clone()).await;

        let mut stream_conn = self.stream_conn.lock();
        // shutdown old streaming connection and replace it with a new connection
        if let Some(conn) = stream_conn.as_ref() {
            conn.shutdown();
        }
        *stream_conn = Some(new_conn);
    }

    /// Handle a player request, return a new playback metadata on success
    pub async fn handle_player_request(
        &self,
        request: PlayerRequest,
        mut playback: Option<PlaybackMetadata>,
    ) -> Result<Option<PlaybackMetadata>> {
        // handle requests that don't require an active playback
        match request {
            PlayerRequest::TransferPlayback(device_id, force_play) => {
                // `TransferPlayback` needs to be handled separately from other player requests
                // because `TransferPlayback` doesn't require an active playback
                self.transfer_playback(&device_id, Some(force_play)).await?;
                tracing::info!("Transferred playback to device with id={}", device_id);
                return Ok(playback);
            }
            PlayerRequest::StartPlayback(p, shuffle) => {
                // Set the playback's shuffle state if specified in the request
                if let (Some(shuffle), Some(playback)) = (shuffle, playback.as_mut()) {
                    playback.shuffle_state = shuffle;
                }
                let device_id = playback.as_ref().and_then(|p| p.device_id.as_deref());
                self.start_playback(p, device_id).await?;
                // For some reasons, when starting a new playback, the integrated `spotify_player`
                // client doesn't respect the initial shuffle state, so we need to manually update the state
                if let Some(ref playback) = playback {
                    self.shuffle(playback.shuffle_state, device_id).await?;
                }
                return Ok(playback);
            }
            _ => {}
        }

        let mut playback = playback.context("no playback found")?;
        let device_id = playback.device_id.as_deref();

        match request {
            PlayerRequest::NextTrack => self.next_track(device_id).await?,
            PlayerRequest::PreviousTrack => self.previous_track(device_id).await?,
            PlayerRequest::Resume => {
                if !playback.is_playing {
                    self.resume_playback(device_id, None).await?;
                    playback.is_playing = true;
                }
            }

            PlayerRequest::Pause => {
                if playback.is_playing {
                    self.pause_playback(device_id).await?;
                    playback.is_playing = false;
                }
            }
            PlayerRequest::ResumePause => {
                if !playback.is_playing {
                    self.resume_playback(device_id, None).await?
                } else {
                    self.pause_playback(device_id).await?
                }
                playback.is_playing = !playback.is_playing;
            }
            PlayerRequest::SeekTrack(position_ms) => {
                self.seek_track(position_ms, device_id).await?
            }
            PlayerRequest::Repeat => {
                let next_repeat_state = match playback.repeat_state {
                    rspotify_model::RepeatState::Off => rspotify_model::RepeatState::Track,
                    rspotify_model::RepeatState::Track => rspotify_model::RepeatState::Context,
                    rspotify_model::RepeatState::Context => rspotify_model::RepeatState::Off,
                };

                self.repeat(next_repeat_state, device_id).await?;

                playback.repeat_state = next_repeat_state;
            }
            PlayerRequest::Shuffle => {
                self.shuffle(!playback.shuffle_state, device_id).await?;

                playback.shuffle_state = !playback.shuffle_state;
            }
            PlayerRequest::Volume(volume) => {
                self.volume(volume, device_id).await?;

                playback.volume = Some(volume as u32);
                playback.mute_state = None;
            }
            PlayerRequest::ToggleMute => {
                let new_mute_state = match playback.mute_state {
                    None => {
                        self.volume(0, device_id).await?;
                        Some(playback.volume.unwrap_or_default())
                    }
                    Some(volume) => {
                        self.volume(volume as u8, device_id).await?;
                        None
                    }
                };

                playback.mute_state = new_mute_state;
            }
            PlayerRequest::StartPlayback(..) => {
                anyhow::bail!("`StartPlayback` should be handled earlier")
            }
            PlayerRequest::TransferPlayback(..) => {
                anyhow::bail!("`TransferPlayback` should be handled earlier")
            }
        };

        Ok(Some(playback))
    }

    /// Handle a client request
    pub(crate) async fn handle_request(
        &self,
        state: &SharedState,
        request: ClientRequest,
    ) -> Result<()> {
        let timer = tokio::time::Instant::now();

        match request {
            ClientRequest::GetBrowseCategories => {
                let categories = self.browse_categories().await?;
                state.data.write().browse.categories = categories;
            }
            ClientRequest::GetBrowseCategoryPlaylists(category) => {
                let playlists = self.browse_category_playlists(&category.id).await?;
                state
                    .data
                    .write()
                    .browse
                    .category_playlists
                    .insert(category.id, playlists);
            }
            #[cfg(feature = "lyric-finder")]
            ClientRequest::GetLyric { track, artists } => {
                let client = lyric_finder::Client::from_http_client(&self.http);
                let query = format!("{track} {artists}");

                if !state.data.read().caches.lyrics.contains_key(&query) {
                    let result = client.get_lyric(&query).await.context(format!(
                        "failed to get lyric for track {track} - artists {artists}"
                    ))?;

                    state
                        .data
                        .write()
                        .caches
                        .lyrics
                        .insert(query, result, *TTL_CACHE_DURATION);
                }
            }
            ClientRequest::ConnectDevice => {
                self.connect_device(state).await;
            }
            #[cfg(feature = "streaming")]
            ClientRequest::RestartIntegratedClient => {
                self.new_session(state).await?;
            }
            ClientRequest::GetCurrentUser => {
                let user = self.current_user().await?;
                state.data.write().user_data.user = Some(user);
            }
            ClientRequest::Player(request) => {
                let playback = state.player.read().buffered_playback.clone();
                let playback = self.handle_player_request(request, playback).await?;
                state.player.write().buffered_playback = playback;
                self.update_playback(state);
            }
            ClientRequest::GetCurrentPlayback => {
                self.retrieve_current_playback(state, true).await?;
            }
            ClientRequest::GetDevices => {
                let devices = self.device().await?;
                state.player.write().devices = devices
                    .into_iter()
                    .filter_map(Device::try_from_device)
                    .collect();
            }
            ClientRequest::GetUserPlaylists => {
                let playlists = self.current_user_playlists().await?;
                store_data_into_file_cache(
                    FileCacheKey::Playlists,
                    &config::get_config().cache_folder,
                    &playlists,
                )
                .context("store user's playlists into the cache folder")?;
                state.data.write().user_data.playlists = playlists;
            }
            ClientRequest::GetUserFollowedArtists => {
                let artists = self.current_user_followed_artists().await?;
                store_data_into_file_cache(
                    FileCacheKey::FollowedArtists,
                    &config::get_config().cache_folder,
                    &artists,
                )
                .context("store user's followed artists into the cache folder")?;
                state.data.write().user_data.followed_artists = artists;
            }
            ClientRequest::GetUserSavedAlbums => {
                let albums = self.current_user_saved_albums().await?;
                store_data_into_file_cache(
                    FileCacheKey::SavedAlbums,
                    &config::get_config().cache_folder,
                    &albums,
                )
                .context("store user's saved albums into the cache folder")?;
                state.data.write().user_data.saved_albums = albums;
            }
            ClientRequest::GetUserTopTracks => {
                let uri = &USER_TOP_TRACKS_ID.uri;
                if !state.data.read().caches.context.contains_key(uri) {
                    let tracks = self.current_user_top_tracks().await?;
                    state.data.write().caches.context.insert(
                        uri.to_owned(),
                        Context::Tracks {
                            tracks,
                            desc: "User's top tracks".to_string(),
                        },
                        *TTL_CACHE_DURATION,
                    );
                }
            }
            ClientRequest::GetUserSavedTracks => {
                let tracks = self.current_user_saved_tracks().await?;
                let tracks_hm = tracks
                    .iter()
                    .map(|t| (t.id.uri(), t.clone()))
                    .collect::<HashMap<_, _>>();
                store_data_into_file_cache(
                    FileCacheKey::SavedTracks,
                    &config::get_config().cache_folder,
                    &tracks_hm,
                )
                .context("store user's saved tracks into the cache folder")?;

                let mut data = state.data.write();
                data.user_data.saved_tracks = tracks_hm;
                data.caches.context.insert(
                    USER_LIKED_TRACKS_ID.uri.to_owned(),
                    Context::Tracks {
                        tracks,
                        desc: "User's liked tracks".to_string(),
                    },
                    *TTL_CACHE_DURATION,
                );
            }
            ClientRequest::GetUserRecentlyPlayedTracks => {
                let uri = &USER_RECENTLY_PLAYED_TRACKS_ID.uri;
                if !state.data.read().caches.context.contains_key(uri) {
                    let tracks = self.current_user_recently_played_tracks().await?;
                    state.data.write().caches.context.insert(
                        uri.to_owned(),
                        Context::Tracks {
                            tracks,
                            desc: "User's recently played tracks".to_string(),
                        },
                        *TTL_CACHE_DURATION,
                    );
                }
            }
            ClientRequest::GetContext(context) => {
                let uri = context.uri();
                if !state.data.read().caches.context.contains_key(&uri) {
                    let context = match context {
                        ContextId::Playlist(playlist_id) => {
                            self.playlist_context(playlist_id).await?
                        }
                        ContextId::Album(album_id) => self.album_context(album_id).await?,
                        ContextId::Artist(artist_id) => self.artist_context(artist_id).await?,
                        ContextId::Tracks(_) => {
                            anyhow::bail!(
                                "`GetContext` request for `tracks` context is not supported!"
                            );
                        }
                    };

                    state
                        .data
                        .write()
                        .caches
                        .context
                        .insert(uri, context, *TTL_CACHE_DURATION);
                }
            }
            ClientRequest::Search(query) => {
                if !state.data.read().caches.search.contains_key(&query) {
                    let results = self.search(&query).await?;

                    state
                        .data
                        .write()
                        .caches
                        .search
                        .insert(query, results, *TTL_CACHE_DURATION);
                }
            }
            ClientRequest::GetRadioTracks {
                seed_uri: uri,
                seed_name: name,
            } => {
                let radio_uri = format!("radio:{uri}");
                if !state.data.read().caches.context.contains_key(&radio_uri) {
                    let tracks = self.radio_tracks(uri).await?;

                    state.data.write().caches.context.insert(
                        radio_uri,
                        Context::Tracks {
                            tracks,
                            desc: format!("{name} Radio"),
                        },
                        *TTL_CACHE_DURATION,
                    );
                }
            }
            ClientRequest::AddTrackToQueue(track_id) => {
                self.add_item_to_queue(PlayableId::Track(track_id), None)
                    .await?
            }
            ClientRequest::AddTrackToPlaylist(playlist_id, track_id) => {
                self.add_track_to_playlist(state, playlist_id, track_id)
                    .await?;
            }
            ClientRequest::AddAlbumToQueue(album_id) => {
                let album_context = self.album_context(album_id).await?;

                if let Context::Album { album: _, tracks } = album_context {
                    for track in tracks {
                        self.add_item_to_queue(PlayableId::Track(track.id), None)
                            .await?;
                    }
                }
            }
            ClientRequest::DeleteTrackFromPlaylist(playlist_id, track_id) => {
                self.delete_track_from_playlist(state, playlist_id, track_id)
                    .await?;
            }
            ClientRequest::AddToLibrary(item) => {
                self.add_to_library(state, item).await?;
            }
            ClientRequest::DeleteFromLibrary(id) => {
                self.delete_from_library(state, id).await?;
            }
            ClientRequest::GetCurrentUserQueue => {
                let queue = self.current_user_queue().await?;
                state.player.write().queue = Some(queue);
            }
            ClientRequest::ReorderPlaylistItems {
                playlist_id,
                insert_index,
                range_start,
                range_length,
                snapshot_id,
            } => {
                self.reorder_playlist_items(
                    state,
                    playlist_id,
                    insert_index,
                    range_start,
                    range_length,
                    snapshot_id.as_deref(),
                )
                .await?;
            }
            ClientRequest::CreatePlaylist {
                playlist_name,
                public,
                collab,
                desc,
            } => {
                let user_id = state
                    .data
                    .read()
                    .user_data
                    .user
                    .as_ref()
                    .map(|u| u.id.to_owned())
                    .unwrap();
                self.create_new_playlist(
                    state,
                    user_id,
                    playlist_name.as_str(),
                    public,
                    collab,
                    desc.as_str(),
                )
                .await?;
            }
        };

        tracing::info!(
            "Successfully handled the client request, took: {}ms",
            timer.elapsed().as_millis()
        );

        Ok(())
    }

    /// Connect to a Spotify device
    async fn connect_device(&self, state: &SharedState) {
        // Device connection can fail when the specified device hasn't shown up
        // in the Spotify's server, resulting in a failed `TransferPlayback` API request.
        // This is why a retry mechanism is needed to ensure a successful connection.
        let delay = std::time::Duration::from_secs(1);

        for _ in 0..10 {
            tokio::time::sleep(delay).await;

            let id = match self.find_available_device().await {
                Ok(Some(id)) => Some(Cow::Owned(id)),
                Ok(None) => None,
                Err(err) => {
                    tracing::error!("Failed to find an available device: {err:#}");
                    None
                }
            };

            if let Some(id) = id {
                tracing::info!("Trying to connect to device (id={id})");
                if let Err(err) = self.transfer_playback(&id, Some(false)).await {
                    tracing::warn!("Connection failed (device_id={id}): {err:#}");
                } else {
                    tracing::info!("Connection succeeded (device_id={id})!");
                    // upon new connection, reset the buffered playback
                    state.player.write().buffered_playback = None;
                    self.update_playback(state);
                    break;
                }
            }
        }
    }

    pub fn update_playback(&self, state: &SharedState) {
        // After handling a request changing the player's playback,
        // update the playback state by making multiple get-playback requests.
        //
        // Q: Why do we need more than one request to update the playback?
        // A: It might take a while for Spotify server to relfect the new change,
        // making additional requests can help ensure that the playback state is always up-to-date.
        let client = self.clone();
        let state = state.clone();
        tokio::task::spawn(async move {
            let delay = std::time::Duration::from_secs(1);
            for _ in 0..5 {
                tokio::time::sleep(delay).await;
                if let Err(err) = client.retrieve_current_playback(&state, false).await {
                    tracing::error!(
                        "Encountered an error when updating the playback state: {err:#}"
                    );
                }
            }
        });
    }

    /// Get Spotify's available browse categories
    pub async fn browse_categories(&self) -> Result<Vec<Category>> {
        let first_page = self
            .categories_manual(Some("EN"), None, Some(50), None)
            .await?;

        Ok(first_page.items.into_iter().map(Category::from).collect())
    }

    /// Get Spotify's available browse playlists of a given category
    pub async fn browse_category_playlists(&self, category_id: &str) -> Result<Vec<Playlist>> {
        let first_page = self
            .category_playlists_manual(category_id, None, Some(50), None)
            .await?;

        Ok(first_page.items.into_iter().map(Playlist::from).collect())
    }

    /// Find an available device. If found, return the device's ID.
    async fn find_available_device(&self) -> Result<Option<String>> {
        let devices = self.device().await?.into_iter().collect::<Vec<_>>();
        if devices.is_empty() {
            tracing::warn!("No device found. Please make sure you already setup Spotify Connect \
                            support as described in https://github.com/aome510/spotify-player#spotify-connect.");
        } else {
            tracing::info!("Available devices: {devices:?}");
        }

        // convert a vector of `Device` items into `(name, id)` pairs
        let mut devices = devices
            .into_iter()
            .filter_map(|d| d.id.map(|id| (d.name, id)))
            .collect::<Vec<_>>();

        let configs = config::get_config();

        // Manually append the integrated device to the device list if `streaming` feature is enabled.
        // The integrated device may not show up in the device list returned by the Spotify API because
        // 1. The device is just initialized and hasn't been registered in Spotify server.
        //    Related issue/discussion: https://github.com/aome510/spotify-player/issues/79
        // 2. The device list is empty. This might be because user doesn't specify their own client ID.
        //    By default, the application uses Spotify web app's client ID, which doesn't have
        //    access to user's active devices.
        #[cfg(feature = "streaming")]
        {
            let session = self.session().await;
            devices.push((
                configs.app_config.device.name.clone(),
                session.device_id().to_string(),
            ));
        }

        if devices.is_empty() {
            return Ok(None);
        }

        // Prioritize the `default_device` specified in the application's configurations,
        // otherwise, use the first available device.
        let id = devices
            .iter()
            .position(|d| d.0 == configs.app_config.default_device)
            .unwrap_or_default();

        Ok(Some(devices.remove(id).1))
    }

    /// Get the saved (liked) tracks of the current user
    pub async fn current_user_saved_tracks(&self) -> Result<Vec<Track>> {
        let first_page = self
            .current_user_saved_tracks_manual(Some(Market::FromToken), Some(50), None)
            .await?;
        let tracks = self.all_paging_items(first_page, &market_query()).await?;
        Ok(tracks
            .into_iter()
            .filter_map(|t| Track::try_from_full_track(t.track))
            .collect())
    }

    /// Get the recently played tracks of the current user
    pub async fn current_user_recently_played_tracks(&self) -> Result<Vec<Track>> {
        let first_page = self.current_user_recently_played(Some(50), None).await?;

        let play_histories = self.all_cursor_based_paging_items(first_page).await?;

        // de-duplicate the tracks returned from the recently-played API
        let mut tracks = Vec::<Track>::new();
        for history in play_histories {
            if !tracks.iter().any(|t| t.name == history.track.name) {
                if let Some(track) = Track::try_from_full_track(history.track) {
                    tracks.push(track);
                }
            }
        }
        Ok(tracks)
    }

    /// Get the top tracks of the current user
    pub async fn current_user_top_tracks(&self) -> Result<Vec<Track>> {
        let first_page = self
            .current_user_top_tracks_manual(None, Some(50), None)
            .await?;

        let tracks = self.all_paging_items(first_page, &Query::new()).await?;
        Ok(tracks
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect())
    }

    /// Get all playlists of the current user
    pub async fn current_user_playlists(&self) -> Result<Vec<Playlist>> {
        // TODO: this should use `rspotify::current_user_playlists_manual` API instead of `internal_call`
        // See: https://github.com/ramsayleung/rspotify/issues/459
        let first_page = self
            .http_get::<Page<SimplifiedPlaylist>>(
                &format!("{SPOTIFY_API_ENDPOINT}/me/playlists"),
                &Query::from([("limit", "50")]),
            )
            .await?;
        // let first_page = self
        //     .current_user_playlists_manual(Some(50), None)
        //     .await?;

        let playlists = self.all_paging_items(first_page, &Query::new()).await?;
        Ok(playlists.into_iter().map(|p| p.into()).collect())
    }

    /// Get all followed artists of the current user
    pub async fn current_user_followed_artists(&self) -> Result<Vec<Artist>> {
        let first_page = self
            .spotify
            .current_user_followed_artists(None, None)
            .await?;

        // followed artists pagination is handled different from
        // other paginations. The endpoint uses cursor-based pagination.
        let mut artists = first_page.items;
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self
                .http_get::<rspotify_model::CursorPageFullArtists>(&url, &Query::new())
                .await?
                .artists;
            artists.append(&mut next_page.items);
            maybe_next = next_page.next;
        }

        // converts `rspotify_model::FullArtist` into `state::Artist`
        Ok(artists.into_iter().map(|a| a.into()).collect())
    }

    /// Get all saved albums of the current user
    pub async fn current_user_saved_albums(&self) -> Result<Vec<Album>> {
        let first_page = self
            .current_user_saved_albums_manual(Some(Market::FromToken), Some(50), None)
            .await?;

        let albums = self.all_paging_items(first_page, &Query::new()).await?;

        // converts `rspotify_model::SavedAlbum` into `state::Album`
        Ok(albums.into_iter().map(|a| a.album.into()).collect())
    }

    /// Get all albums of an artist
    pub async fn artist_albums(&self, artist_id: ArtistId<'_>) -> Result<Vec<Album>> {
        let payload = market_query();

        let mut singles = {
            let first_page = self
                .artist_albums_manual(
                    artist_id.as_ref(),
                    Some(rspotify_model::AlbumType::Single),
                    Some(Market::FromToken),
                    Some(50),
                    None,
                )
                .await?;
            self.all_paging_items(first_page, &payload).await
        }?;
        let mut albums = {
            let first_page = self
                .artist_albums_manual(
                    artist_id.as_ref(),
                    Some(rspotify_model::AlbumType::Album),
                    Some(Market::FromToken),
                    Some(50),
                    None,
                )
                .await?;
            self.all_paging_items(first_page, &payload).await
        }?;
        albums.append(&mut singles);

        // converts `rspotify_model::SimplifiedAlbum` into `state::Album`
        let albums = albums
            .into_iter()
            .filter_map(Album::try_from_simplified_album)
            .collect();
        Ok(self.process_artist_albums(albums))
    }

    /// Start a playback
    async fn start_playback(&self, playback: Playback, device_id: Option<&str>) -> Result<()> {
        match playback {
            Playback::Context(id, offset) => match id {
                ContextId::Album(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?
                }
                ContextId::Artist(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?
                }
                ContextId::Playlist(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?
                }
                ContextId::Tracks(_) => {
                    anyhow::bail!("`StartPlayback` request for `tracks` context is not supported")
                }
            },
            Playback::URIs(track_ids, offset) => {
                self.start_uris_playback(
                    track_ids.into_iter().map(PlayableId::from),
                    device_id,
                    offset,
                    None,
                )
                .await?
            }
        }

        Ok(())
    }

    /// Get recommendation (radio) tracks based on a seed
    pub async fn radio_tracks(&self, seed_uri: String) -> Result<Vec<Track>> {
        let session = self.session().await;

        // Get an autoplay URI from the seed URI.
        // The return URI is a Spotify station's URI
        let autoplay_query_url = format!("hm://autoplay-enabled/query?uri={seed_uri}");
        let response = session
            .mercury()
            .get(autoplay_query_url)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to get autoplay URI: got a Mercury error"))?;
        if response.status_code != 200 {
            anyhow::bail!(
                "Failed to get autoplay URI: got non-OK status code: {}",
                response.status_code
            );
        }
        let autoplay_uri = String::from_utf8(response.payload[0].to_vec())?;

        // Retrieve radio's data based on the autoplay URI
        let radio_query_url = format!("hm://radio-apollo/v3/stations/{autoplay_uri}");
        let response = session.mercury().get(radio_query_url).await.map_err(|_| {
            anyhow::anyhow!("Failed to get radio data of {autoplay_uri}: got a Mercury error")
        })?;
        if response.status_code != 200 {
            anyhow::bail!(
                "Failed to get radio data of {autoplay_uri}: got non-OK status code: {}",
                response.status_code
            );
        }

        #[derive(Debug, Deserialize)]
        struct TrackData {
            original_gid: String,
        }
        #[derive(Debug, Deserialize)]
        struct RadioStationResponse {
            tracks: Vec<TrackData>,
        }
        // Parse a list consisting of IDs of tracks inside the radio station
        let track_ids = serde_json::from_slice::<RadioStationResponse>(&response.payload[0])?
            .tracks
            .into_iter()
            .filter_map(|t| TrackId::from_id(t.original_gid).ok());

        // Retrieve tracks based on IDs
        let tracks = self.tracks(track_ids, Some(Market::FromToken)).await?;
        let tracks = tracks
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect();

        Ok(tracks)
    }

    /// Search for items (tracks, artists, albums, playlists) matching a given query
    pub async fn search(&self, query: &str) -> Result<SearchResults> {
        let (track_result, artist_result, album_result, playlist_result) = tokio::try_join!(
            self.search_specific_type(query, rspotify_model::SearchType::Track),
            self.search_specific_type(query, rspotify_model::SearchType::Artist),
            self.search_specific_type(query, rspotify_model::SearchType::Album),
            self.search_specific_type(query, rspotify_model::SearchType::Playlist)
        )?;

        let (tracks, artists, albums, playlists) = (
            match track_result {
                rspotify_model::SearchResult::Tracks(p) => p
                    .items
                    .into_iter()
                    .filter_map(Track::try_from_full_track)
                    .collect(),
                _ => anyhow::bail!("expect a track search result"),
            },
            match artist_result {
                rspotify_model::SearchResult::Artists(p) => {
                    p.items.into_iter().map(|a| a.into()).collect()
                }
                _ => anyhow::bail!("expect an artist search result"),
            },
            match album_result {
                rspotify_model::SearchResult::Albums(p) => p
                    .items
                    .into_iter()
                    .filter_map(Album::try_from_simplified_album)
                    .collect(),
                _ => anyhow::bail!("expect an album search result"),
            },
            match playlist_result {
                rspotify_model::SearchResult::Playlists(p) => {
                    p.items.into_iter().map(|i| i.into()).collect()
                }
                _ => anyhow::bail!("expect a playlist search result"),
            },
        );

        Ok(SearchResults {
            tracks,
            artists,
            albums,
            playlists,
        })
    }

    /// Search for items of a specific type matching a given query
    pub async fn search_specific_type(
        &self,
        query: &str,
        _type: rspotify_model::SearchType,
    ) -> Result<rspotify_model::SearchResult> {
        Ok(self
            .spotify
            .search(query, _type, None, None, None, None)
            .await?)
    }

    /// Add a track to a playlist
    pub async fn add_track_to_playlist(
        &self,
        state: &SharedState,
        playlist_id: PlaylistId<'_>,
        track_id: TrackId<'_>,
    ) -> Result<()> {
        // remove all the occurrences of the track to ensure no duplication in the playlist
        self.playlist_remove_all_occurrences_of_items(
            playlist_id.as_ref(),
            [PlayableId::Track(track_id.as_ref())],
            None,
        )
        .await?;

        self.playlist_add_items(
            playlist_id.as_ref(),
            [PlayableId::Track(track_id.as_ref())],
            None,
        )
        .await?;

        // After adding a new track to a playlist, remove the cache of that playlist to force refetching new data
        state.data.write().caches.context.remove(&playlist_id.uri());

        Ok(())
    }

    /// Remove a track from a playlist
    pub async fn delete_track_from_playlist(
        &self,
        state: &SharedState,
        playlist_id: PlaylistId<'_>,
        track_id: TrackId<'_>,
    ) -> Result<()> {
        // remove all the occurrences of the track to ensure no duplication in the playlist
        self.playlist_remove_all_occurrences_of_items(
            playlist_id.as_ref(),
            [PlayableId::Track(track_id.as_ref())],
            None,
        )
        .await?;

        // After making a delete request, update the playlist in-memory data stored inside the app caches.
        if let Some(Context::Playlist { tracks, .. }) = state
            .data
            .write()
            .caches
            .context
            .get_mut(&playlist_id.uri())
        {
            tracks.retain(|t| t.id != track_id);
        }

        Ok(())
    }

    /// Reorder items in a playlist
    async fn reorder_playlist_items(
        &self,
        state: &SharedState,
        playlist_id: PlaylistId<'_>,
        insert_index: usize,
        range_start: usize,
        range_length: Option<usize>,
        snapshot_id: Option<&str>,
    ) -> Result<()> {
        let insert_before = match insert_index > range_start {
            true => insert_index + 1,
            false => insert_index,
        };

        self.playlist_reorder_items(
            playlist_id.clone(),
            Some(range_start as i32),
            Some(insert_before as i32),
            range_length.map(|range_length| range_length as u32),
            snapshot_id,
        )
        .await?;

        // After making a reorder request, update the playlist in-memory data stored inside the app caches.
        if let Some(Context::Playlist { tracks, .. }) = state
            .data
            .write()
            .caches
            .context
            .get_mut(&playlist_id.uri())
        {
            let track = tracks.remove(range_start);
            tracks.insert(insert_index, track);
        }

        Ok(())
    }

    /// Add a Spotify item to current user's library.
    async fn add_to_library(&self, state: &SharedState, item: Item) -> Result<()> {
        // Before adding new item, checks if that item already exists in the library to avoid adding a duplicated item.
        match item {
            Item::Track(track) => {
                let contains = self
                    .current_user_saved_tracks_contains([track.id.as_ref()])
                    .await?;
                if !contains[0] {
                    self.current_user_saved_tracks_add([track.id.as_ref()])
                        .await?;
                    // update the in-memory `user_data`
                    state
                        .data
                        .write()
                        .user_data
                        .saved_tracks
                        .insert(track.id.uri(), track);
                }
            }
            Item::Album(album) => {
                let contains = self
                    .current_user_saved_albums_contains([album.id.as_ref()])
                    .await?;
                if !contains[0] {
                    self.current_user_saved_albums_add([album.id.as_ref()])
                        .await?;
                    // update the in-memory `user_data`
                    state.data.write().user_data.saved_albums.insert(0, album);
                }
            }
            Item::Artist(artist) => {
                let follows = self.user_artist_check_follow([artist.id.as_ref()]).await?;
                if !follows[0] {
                    self.user_follow_artists([artist.id.as_ref()]).await?;
                    // update the in-memory `user_data`
                    state
                        .data
                        .write()
                        .user_data
                        .followed_artists
                        .insert(0, artist);
                }
            }
            Item::Playlist(playlist) => {
                let user_id = state
                    .data
                    .read()
                    .user_data
                    .user
                    .as_ref()
                    .map(|u| u.id.clone());

                if let Some(user_id) = user_id {
                    let follows = self
                        .playlist_check_follow(playlist.id.as_ref(), &[user_id])
                        .await?;
                    if !follows[0] {
                        self.playlist_follow(playlist.id.as_ref(), None).await?;
                        // update the in-memory `user_data`
                        state.data.write().user_data.playlists.insert(0, playlist);
                    }
                }
            }
        }
        Ok(())
    }

    // Delete a Spotify item from user's library
    async fn delete_from_library(&self, state: &SharedState, id: ItemId) -> Result<()> {
        match id {
            ItemId::Track(id) => {
                let uri = id.uri();
                self.current_user_saved_tracks_delete([id]).await?;
                state.data.write().user_data.saved_tracks.remove(&uri);
            }
            ItemId::Album(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .saved_albums
                    .retain(|a| a.id != id);
                self.current_user_saved_albums_delete([id]).await?;
            }
            ItemId::Artist(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .followed_artists
                    .retain(|a| a.id != id);
                self.user_unfollow_artists([id]).await?;
            }
            ItemId::Playlist(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .playlists
                    .retain(|p| p.id != id);
                self.playlist_unfollow(id).await?;
            }
        }
        Ok(())
    }

    /// Get a track data
    pub async fn track(&self, track_id: TrackId<'_>) -> Result<Track> {
        Track::try_from_full_track(
            self.spotify
                .track(track_id, Some(Market::FromToken))
                .await?,
        )
        .context("convert FullTrack into Track")
    }

    /// Get a playlist context data
    pub async fn playlist_context(&self, playlist_id: PlaylistId<'_>) -> Result<Context> {
        let playlist_uri = playlist_id.uri();
        tracing::info!("Get playlist context: {}", playlist_uri);

        // TODO: this should use `rspotify::playlist` API instead of `internal_call`
        // See: https://github.com/ramsayleung/rspotify/issues/459
        // let playlist = self
        //     .playlist(playlist_id, None, Some(Market::FromToken))
        //     .await?;
        let playlist = self
            .http_get::<FullPlaylist>(
                &format!("{SPOTIFY_API_ENDPOINT}/playlists/{}", playlist_id.id()),
                &market_query(),
            )
            .await?;

        // get the playlist's tracks
        let first_page = playlist.tracks.clone();
        let tracks = self
            .all_paging_items(first_page, &market_query())
            .await?
            .into_iter()
            .filter_map(|item| match item.track {
                Some(rspotify_model::PlayableItem::Track(track)) => {
                    Track::try_from_full_track(track)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        Ok(Context::Playlist {
            playlist: playlist.into(),
            tracks,
        })
    }

    /// Get an album context data
    pub async fn album_context(&self, album_id: AlbumId<'_>) -> Result<Context> {
        let album_uri = album_id.uri();
        tracing::info!("Get album context: {}", album_uri);

        let album = self.album(album_id, Some(Market::FromToken)).await?;
        let first_page = album.tracks.clone();

        // converts `rspotify_model::FullAlbum` into `state::Album`
        let album: Album = album.into();

        // get the album's tracks
        let tracks = self
            .all_paging_items(first_page, &Query::new())
            .await?
            .into_iter()
            .filter_map(|t| {
                // simplified track doesn't have album so
                // we need to manually include one during
                // converting into `state::Track`
                Track::try_from_simplified_track(t).map(|mut t| {
                    t.album = Some(album.clone());
                    t
                })
            })
            .collect::<Vec<_>>();

        Ok(Context::Album { album, tracks })
    }

    /// Get an artist context data
    pub async fn artist_context(&self, artist_id: ArtistId<'_>) -> Result<Context> {
        let artist_uri = artist_id.uri();
        tracing::info!("Get artist context: {}", artist_uri);

        // get the artist's information, including top tracks, related artists, and albums

        let artist = self.artist(artist_id.as_ref()).await?.into();

        let top_tracks = self
            .artist_top_tracks(artist_id.as_ref(), Some(Market::FromToken))
            .await?;
        let top_tracks = top_tracks
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect::<Vec<_>>();

        let related_artists = self.artist_related_artists(artist_id.as_ref()).await?;
        let related_artists = related_artists
            .into_iter()
            .map(|a| a.into())
            .collect::<Vec<_>>();

        let albums = self.artist_albums(artist_id.as_ref()).await?;

        Ok(Context::Artist {
            artist,
            top_tracks,
            albums,
            related_artists,
        })
    }

    /// Make a GET HTTP request to the Spotify server
    async fn http_get<T>(&self, url: &str, payload: &Query<'_>) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        /// a helper function to process an API response from Spotify server
        ///
        /// This function is mainly used to patch upstream API bugs , resulting in
        /// a type error when a third-party library like `rspotify` parses the response
        fn process_spotify_api_response(text: String) -> String {
            // See: https://github.com/ramsayleung/rspotify/issues/459
            text.replace("\"images\":null", "\"images\":[]")
        }

        let access_token = self.access_token().await?;

        tracing::debug!("{access_token} {url}");

        let response = self
            .http
            .get(url)
            .query(payload)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {access_token}"),
            )
            .send()
            .await?;

        let text = process_spotify_api_response(response.text().await?);
        tracing::debug!("{text}");

        Ok(serde_json::from_str(&text)?)
    }

    /// Get all paging items starting from a pagination object of the first page
    async fn all_paging_items<T>(
        &self,
        first_page: rspotify_model::Page<T>,
        payload: &Query<'_>,
    ) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;

        while let Some(url) = maybe_next {
            let mut next_page = self
                .http_get::<rspotify_model::Page<T>>(&url, payload)
                .await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// Get all cursor-based paging items starting from a pagination object of the first page
    async fn all_cursor_based_paging_items<T>(
        &self,
        first_page: rspotify_model::CursorBasedPage<T>,
    ) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self
                .http_get::<rspotify_model::CursorBasedPage<T>>(&url, &Query::new())
                .await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// Retrieve the latest playback state
    pub async fn retrieve_current_playback(
        &self,
        state: &SharedState,
        reset_buffered_playback: bool,
    ) -> Result<()> {
        let new_track = {
            // update the playback state
            let playback = self.current_playback(None, None::<Vec<_>>).await?;
            let mut player = state.player.write();

            let prev_track_name = player
                .current_playing_track()
                .map(|t| t.name.to_owned())
                .unwrap_or_default();

            player.playback = playback;
            player.playback_last_updated_time = Some(std::time::Instant::now());

            let curr_track_name = player
                .current_playing_track()
                .map(|t| t.name.to_owned())
                .unwrap_or_default();

            let new_track = prev_track_name != curr_track_name && !curr_track_name.is_empty();
            // check if we need to update the buffered playback
            let needs_update = match (&player.buffered_playback, &player.playback) {
                (Some(bp), Some(p)) => bp.device_id != p.device.id || new_track,
                (None, None) => false,
                _ => true,
            };

            if reset_buffered_playback || needs_update {
                player.buffered_playback = player.playback.as_ref().map(|p| {
                    let mut playback = PlaybackMetadata::from_playback(p);

                    // handle additional data from the previous buffered state
                    // that is not available in a standard Spotify playback's state
                    if let Some(bp) = &player.buffered_playback {
                        if let Some(volume) = bp.mute_state {
                            playback.volume = Some(volume);
                        }
                        playback.mute_state = bp.mute_state;
                        playback.fake_track_repeat_state = bp.fake_track_repeat_state;
                    }
                    playback
                });
            }

            new_track
        };

        if !new_track {
            return Ok(());
        }
        #[cfg(any(feature = "image", feature = "notify"))]
        self.handle_new_track_event(state).await?;

        Ok(())
    }

    // Handle new track event
    #[cfg(any(feature = "image", feature = "notify"))]
    async fn handle_new_track_event(&self, state: &SharedState) -> Result<()> {
        let configs = config::get_config();

        let track = match state.player.read().current_playing_track() {
            None => return Ok(()),
            Some(track) => track.clone(),
        };

        let url = match crate::utils::get_track_album_image_url(&track) {
            Some(url) => url,
            None => return Ok(()),
        };

        let path = (format!(
            "{}-{}-cover.jpg",
            track.album.name,
            crate::utils::map_join(&track.album.artists, |a| &a.name, ", ")
        ))
        .replace('/', ""); // remove invalid characters from the file's name
        let path = configs.cache_folder.join("image").join(path);

        #[cfg(feature = "image")]
        if !state.data.read().caches.images.contains_key(url) {
            let bytes = self
                .retrieve_image(url, &path, configs.app_config.enable_cover_image_cache)
                .await?;
            let image =
                image::load_from_memory(&bytes).context("Failed to load image from memory")?;
            state
                .data
                .write()
                .caches
                .images
                .insert(url.to_owned(), image, *TTL_CACHE_DURATION);
        }

        // notify user about the playback's change if any
        #[cfg(feature = "notify")]
        if configs.app_config.enable_notify {
            // for Linux, ensure that the cached cover image is available to render the notification's thumbnail
            #[cfg(all(unix, not(target_os = "macos")))]
            self.retrieve_image(url, &path, true).await?;

            if !configs.app_config.notify_streaming_only || self.stream_conn.lock().is_some() {
                Self::notify_new_track(track, &path)?;
            }
            #[cfg(not(feature = "streaming"))]
            Self::notify_new_track(track, &path, state)?;
        }

        Ok(())
    }

    /// Create a new playlist
    async fn create_new_playlist(
        &self,
        state: &SharedState,
        user_id: UserId<'static>,
        playlist_name: &str,
        public: bool,
        collab: bool,
        desc: &str,
    ) -> Result<()> {
        let playlist: Playlist = self
            .user_playlist_create(
                user_id,
                playlist_name,
                Some(public),
                Some(collab),
                Some(desc),
            )
            .await?
            .into();
        tracing::info!(
            "new playlist (name={},id={}) was successfully created",
            playlist.name,
            playlist.id
        );
        state.data.write().user_data.playlists.insert(0, playlist);
        Ok(())
    }

    #[cfg(feature = "notify")]
    /// Create a notification for a new track
    fn notify_new_track(
        track: rspotify_model::FullTrack,
        cover_img_path: &std::path::Path,
    ) -> Result<()> {
        let mut n = notify_rust::Notification::new();

        let re = regex::Regex::new(r"\{.*?\}").unwrap();
        // Generate a text described a track from a format string.
        // For example, a format string "{track} - {artists}" will generate
        // a text consisting of the track's name followed by a dash then artists' names.
        let get_text_from_format_str = |format_str: &str| {
            let mut text = String::new();

            let mut ptr = 0;
            for m in re.find_iter(format_str) {
                let s = m.start();
                let e = m.end();

                if ptr < s {
                    text += &format_str[ptr..s];
                }
                ptr = e;
                match m.as_str() {
                    "{track}" => text += &track.name,
                    "{artists}" => {
                        text += &crate::utils::map_join(&track.artists, |a| &a.name, ", ")
                    }
                    "{album}" => text += &track.album.name,
                    _ => continue,
                }
            }
            if ptr < format_str.len() {
                text += &format_str[ptr..];
            }

            text
        };

        let configs = config::get_config();

        n.appname("spotify_player")
            .icon(cover_img_path.to_str().unwrap())
            .summary(&get_text_from_format_str(
                &configs.app_config.notify_format.summary,
            ))
            .body(&get_text_from_format_str(
                &configs.app_config.notify_format.body,
            ));
        if configs.app_config.notify_timeout_in_secs > 0 {
            n.timeout(std::time::Duration::from_secs(
                configs.app_config.notify_timeout_in_secs,
            ));
        }
        n.show()?;

        Ok(())
    }

    /// Retrieve an image from a `url` or a cached `path`.
    /// If `saved` is specified, the retrieved image is saved to the cached `path`.
    #[cfg(any(feature = "image", feature = "notify"))]
    async fn retrieve_image(
        &self,
        url: &str,
        path: &std::path::Path,
        saved: bool,
    ) -> Result<Vec<u8>> {
        use std::io::Write;

        if path.exists() {
            tracing::info!("Retrieving image from file: {}", path.display());
            return Ok(std::fs::read(path)?);
        }

        tracing::info!("Retrieving image from url: {url}");

        let bytes = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("get image from url {url}"))?
            .bytes()
            .await?;

        if saved {
            tracing::info!("Saving the retrieved image into {}", path.display());
            let mut file = std::fs::File::create(path)?;
            file.write_all(&bytes)?;
        }

        Ok(bytes.to_vec())
    }

    /// Process a list of albums, which includes
    /// - sort albums by the release date
    /// - remove albums with duplicated names
    fn process_artist_albums(&self, albums: Vec<Album>) -> Vec<Album> {
        let mut albums = albums.into_iter().collect::<Vec<_>>();

        albums.sort_by(|x, y| x.release_date.partial_cmp(&y.release_date).unwrap());

        // use a HashSet to keep track albums with the same name
        let mut seen_names = std::collections::HashSet::new();

        albums.into_iter().rfold(vec![], |mut acc, a| {
            if !seen_names.contains(&a.name) {
                seen_names.insert(a.name.clone());
                acc.push(a);
            }
            acc
        })
    }
}
