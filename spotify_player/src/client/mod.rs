use std::ops::Deref;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

use crate::state::Lyrics;
use crate::{auth, config};
use crate::{
    auth::AuthConfig,
    state::{
        store_data_into_file_cache, Album, AlbumId, Artist, ArtistId, Category, Context, ContextId,
        Device, FileCacheKey, Item, ItemId, MemoryCaches, Playback, PlaybackMetadata, Playlist,
        PlaylistFolderItem, PlaylistId, SearchResults, SharedState, Show, ShowId, Track, TrackId,
        UserId, TTL_CACHE_DURATION, USER_LIKED_TRACKS_ID, USER_RECENTLY_PLAYED_TRACKS_ID,
        USER_TOP_TRACKS_ID,
    },
};

use std::io::Write;

use anyhow::Context as _;
use anyhow::Result;

use librespot_core::SpotifyUri;
#[cfg(feature = "streaming")]
use parking_lot::Mutex;

use reqwest::StatusCode;
use rspotify::{http::Query, prelude::*};

mod handlers;
mod request;
mod spotify;

pub use handlers::*;
pub use request::*;
use serde::Deserialize;

const SPOTIFY_API_ENDPOINT: &str = "https://api.spotify.com/v1";
const PLAYBACK_TYPES: [&rspotify::model::AdditionalType; 2] = [
    &rspotify::model::AdditionalType::Track,
    &rspotify::model::AdditionalType::Episode,
];

/// The application's Spotify client
#[derive(Clone)]
pub struct AppClient {
    http: reqwest::Client,
    spotify: Arc<spotify::Spotify>,
    auth_config: AuthConfig,
    user_client: Option<rspotify::AuthCodePkceSpotify>,
    #[cfg(feature = "streaming")]
    stream_conn: Arc<Mutex<Option<librespot_connect::Spirc>>>,
}

impl Deref for AppClient {
    type Target = spotify::Spotify;
    fn deref(&self) -> &Self::Target {
        self.spotify.as_ref()
    }
}

fn market_query() -> Query<'static> {
    Query::from([("market", "from_token")])
}

impl AppClient {
    /// Construct a new client
    pub async fn new() -> Result<Self> {
        let configs = config::get_config();
        let auth_config = AuthConfig::new(configs)?;

        // Construct user-provided client.
        // This custom client is needed for Spotify Connect integration because the Spotify client (`AppConfig::spotify`),
        // which `spotify-player` uses to retrieve Spotify data, doesn't have access to user available devices
        let mut user_client = configs.app_config.get_user_client_id()?.clone().map(|id| {
            let creds = rspotify::Credentials { id, secret: None };
            let oauth = rspotify::OAuth {
                scopes: rspotify::scopes!("user-read-playback-state"),
                redirect_uri: configs.app_config.login_redirect_uri.clone(),
                ..Default::default()
            };
            let config = rspotify::Config {
                token_cached: true,
                cache_path: configs.cache_folder.join("user_client_token.json"),
                ..Default::default()
            };
            rspotify::AuthCodePkceSpotify::with_config(creds, oauth, config)
        });

        if let Some(client) = &mut user_client {
            let url = client
                .get_authorize_url(None)
                .context("get authorize URL for user-provided client")?;
            client
                .prompt_for_token(&url)
                .await
                .context("get token for user-provided client")?;
        }

        Ok(Self {
            spotify: Arc::new(spotify::Spotify::new()),
            http: reqwest::Client::new(),
            auth_config,
            user_client,

            #[cfg(feature = "streaming")]
            stream_conn: Arc::new(Mutex::new(None)),
        })
    }

    /// Initialize the application's playback upon creating a new session or during startup
    pub fn initialize_playback(&self, state: &SharedState) {
        tokio::task::spawn({
            let client = self.clone();
            let state = state.clone();
            async move {
                // The main playback initialization logic is simple:
                // if there is no playback, connect to an available device
                //
                // However, because it takes time for Spotify server to show up new changes,
                // a retry logic is implemented to ensure the application's state is properly initialized
                let delay = std::time::Duration::from_secs(1);

                for _ in 0..5 {
                    tokio::time::sleep(delay).await;

                    if let Err(err) = client.retrieve_current_playback(&state, false).await {
                        tracing::error!("Failed to retrieve current playback: {err:#}");
                        return;
                    }

                    // if playback exists, don't connect to a new device
                    if state.player.read().playback.is_some() {
                        continue;
                    }

                    let id = match client.find_available_device().await {
                        Ok(Some(id)) => Some(Cow::Owned(id)),
                        Ok(None) => None,
                        Err(err) => {
                            tracing::error!("Failed to find an available device: {err:#}");
                            None
                        }
                    };

                    if let Some(id) = id {
                        tracing::info!("Trying to connect to device (id={id})");
                        if let Err(err) = client.transfer_playback(&id, Some(false)).await {
                            tracing::warn!("Connection failed (device_id={id}): {err:#}");
                        } else {
                            tracing::info!("Connection succeeded (device_id={id})!");
                            // upon new connection, reset the buffered playback
                            state.player.write().buffered_playback = None;
                            client.update_playback(&state);
                            break;
                        }
                    }
                }
            }
        });
    }

    /// Create a new client session
    pub async fn new_session(&self, state: Option<&SharedState>, reauth: bool) -> Result<()> {
        let session = self.auth_config.session();
        let creds = auth::get_creds(&self.auth_config, reauth, true).context("get credentials")?;
        *self.session.lock().await = Some(session.clone());

        #[allow(unused_mut)]
        let mut connected = false;

        #[cfg(feature = "streaming")]
        if let Some(state) = state {
            if state.is_streaming_enabled() {
                self.new_streaming_connection(state.clone(), session.clone(), creds.clone())
                    .await
                    .context("new streaming connection")?;
                connected = true;
            }
        }

        if !connected {
            // if session is not connected (triggered by `new_streaming_connection`), connect to the session
            session
                .connect(creds, true)
                .await
                .context("connect to a session")?;
        }

        tracing::info!("Used a new session for Spotify client.");

        self.refresh_token().await.context("refresh auth token")?;

        if let Some(state) = state {
            // reset the application's caches
            state.data.write().caches = MemoryCaches::new();
            self.initialize_playback(state);
        }

        Ok(())
    }

    /// Check if the current session is valid and if invalid, create a new session
    pub async fn check_valid_session(&self, state: &SharedState) -> Result<()> {
        if self.session().await.is_invalid() {
            tracing::info!("Client's current session is invalid, creating a new session...");
            self.new_session(Some(state), false)
                .await
                .context("create new client session")?;
        }
        Ok(())
    }

    /// Create a new streaming connection
    #[cfg(feature = "streaming")]
    pub async fn new_streaming_connection(
        &self,
        state: SharedState,
        session: librespot_core::Session,
        creds: librespot_core::authentication::Credentials,
    ) -> Result<()> {
        let new_conn =
            crate::streaming::new_connection(self.clone(), state, session, creds).await?;
        let mut stream_conn = self.stream_conn.lock();
        // shutdown old streaming connection and replace it with a new connection
        if let Some(conn) = stream_conn.as_ref() {
            if let Err(err) = conn.shutdown() {
                log::error!("Failed to shutdown old streaming connection: {err:#}");
            }
        }
        *stream_conn = Some(new_conn);
        Ok(())
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
                return Ok(None);
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
                return Ok(None);
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
                if playback.is_playing {
                    self.pause_playback(device_id).await?;
                } else {
                    self.resume_playback(device_id, None).await?;
                }
                playback.is_playing = !playback.is_playing;
            }
            PlayerRequest::SeekTrack(position_ms) => {
                self.seek_track(position_ms, device_id).await?;
            }
            PlayerRequest::Repeat => {
                let next_repeat_state = match playback.repeat_state {
                    rspotify::model::RepeatState::Off => rspotify::model::RepeatState::Track,
                    rspotify::model::RepeatState::Track => rspotify::model::RepeatState::Context,
                    rspotify::model::RepeatState::Context => rspotify::model::RepeatState::Off,
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

                playback.volume = Some(u32::from(volume));
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
        }

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
            ClientRequest::GetLyrics { track_id } => {
                let uri = track_id.uri();
                if !state.data.read().caches.lyrics.contains_key(&uri) {
                    let lyrics = self.lyrics(track_id).await?;
                    state
                        .data
                        .write()
                        .caches
                        .lyrics
                        .insert(uri, lyrics, *TTL_CACHE_DURATION);
                }
            }
            #[cfg(feature = "streaming")]
            ClientRequest::RestartIntegratedClient => {
                self.new_session(Some(state), false).await?;
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
                let devices = self.available_devices().await?;
                state.player.write().devices = devices
                    .into_iter()
                    .filter_map(Device::try_from_device)
                    .collect();
            }
            ClientRequest::GetUserPlaylists => {
                let playlists = self.current_user_playlists().await?;
                let node = state.data.read().user_data.playlist_folder_node.clone();
                let playlists = if let Some(node) = node.filter(|n| !n.children.is_empty()) {
                    crate::playlist_folders::structurize(playlists, &node.children)
                } else {
                    playlists
                        .into_iter()
                        .map(PlaylistFolderItem::Playlist)
                        .collect()
                };
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
            ClientRequest::GetUserSavedShows => {
                let shows = self.current_user_saved_shows().await?;
                store_data_into_file_cache(
                    FileCacheKey::SavedShows,
                    &config::get_config().cache_folder,
                    &shows,
                )
                .context("store user's saved shows into the cache folder")?;
                state.data.write().user_data.saved_shows = shows;
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
                    USER_LIKED_TRACKS_ID.uri.clone(),
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
                        ContextId::Show(show_id) => self.show_context(show_id).await?,
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
            ClientRequest::AddPlayableToQueue(playable_id) => {
                self.add_item_to_queue(playable_id, None).await?;
            }
            ClientRequest::AddPlayableToPlaylist(playlist_id, playable_id) => {
                self.add_item_to_playlist(state, playlist_id, playable_id)
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
                    .map(|u| u.id.clone())
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
        }

        tracing::info!(
            "Successfully handled the client request, took: {}ms",
            timer.elapsed().as_millis()
        );

        Ok(())
    }

    /// Get lyrics of a given track, return None if no lyrics is available
    pub async fn lyrics(&self, track_id: TrackId<'static>) -> Result<Option<Lyrics>> {
        let session = self.session().await;
        let uri = SpotifyUri::from_uri(&track_id.uri())?;
        match uri {
            SpotifyUri::Track { id } => {
                match librespot_metadata::Lyrics::get(&session, &id).await {
                    Ok(lyrics) => Ok(Some(lyrics.into())),
                    Err(err) => {
                        if err.to_string().to_lowercase().contains("not found") {
                            Ok(None)
                        } else {
                            Err(err.into())
                        }
                    }
                }
            }
            _ => Ok(None),
        }
    }

    /// Get user available devices
    pub async fn available_devices(&self) -> Result<Vec<rspotify::model::Device>> {
        match &self.user_client {
            None => {
                tracing::warn!("User-provided client integration is not enabled, no device found.");
                tracing::warn!("Please make sure you setup Spotify Connect as described in https://github.com/aome510/spotify-player#spotify-connect.");
                Ok(vec![])
            }
            Some(client) => Ok(client.device().await?),
        }
    }

    pub fn update_playback(&self, state: &SharedState) {
        // After handling a request changing the player's playback,
        // update the playback state by making multiple get-playback requests.
        //
        // Q: Why do we need more than one request to update the playback?
        // A: It might take a while for Spotify server to reflect the new change,
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
        let devices = self.available_devices().await?;
        tracing::info!("Available devices: {devices:?}");

        // if there is an active device, return it
        if let Some(d) = devices.iter().find(|d| d.is_active) {
            return Ok(d.id.clone());
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
            .current_user_saved_tracks_manual(
                Some(rspotify::model::Market::FromToken),
                Some(50),
                None,
            )
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
        // Fetch the first page of playlists
        let first_page = self
            .http_get::<rspotify::model::Page<rspotify::model::SimplifiedPlaylist>>(
                &format!("{SPOTIFY_API_ENDPOINT}/me/playlists"),
                &Query::from([("limit", "50")]),
            )
            .await?;
        // let first_page = self
        //     .current_user_playlists_manual(Some(50), None)
        //     .await?;

        // Fetch all pages of playlists
        let playlists = self.all_paging_items(first_page, &Query::new()).await?;

        Ok(playlists
            .into_iter()
            .map(std::convert::Into::into)
            .collect())
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
                .http_get::<rspotify::model::CursorPageFullArtists>(&url, &Query::new())
                .await?
                .artists;
            artists.append(&mut next_page.items);
            maybe_next = next_page.next;
        }

        // converts `rspotify::model::FullArtist` into `state::Artist`
        Ok(artists.into_iter().map(std::convert::Into::into).collect())
    }

    /// Get all saved albums of the current user
    pub async fn current_user_saved_albums(&self) -> Result<Vec<Album>> {
        let first_page = self
            .current_user_saved_albums_manual(
                Some(rspotify::model::Market::FromToken),
                Some(50),
                None,
            )
            .await?;

        let albums = self.all_paging_items(first_page, &Query::new()).await?;

        // Converts `rspotify::model::SavedAlbum` into `state::Album`
        Ok(albums.into_iter().map(Album::from).collect())
    }

    /// Get all saved shows of the current user
    pub async fn current_user_saved_shows(&self) -> Result<Vec<Show>> {
        let first_page = self.get_saved_show_manual(Some(50), None).await?;
        let shows = self.all_paging_items(first_page, &Query::new()).await?;
        Ok(shows.into_iter().map(|s| s.show.into()).collect())
    }

    /// Get all albums of an artist
    pub async fn artist_albums(&self, artist_id: ArtistId<'_>) -> Result<Vec<Album>> {
        let payload = market_query();

        let mut singles = {
            let first_page = self
                .artist_albums_manual(
                    artist_id.as_ref(),
                    Some(rspotify::model::AlbumType::Single),
                    Some(rspotify::model::Market::FromToken),
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
                    Some(rspotify::model::AlbumType::Album),
                    Some(rspotify::model::Market::FromToken),
                    Some(50),
                    None,
                )
                .await?;
            self.all_paging_items(first_page, &payload).await
        }?;
        albums.append(&mut singles);

        // converts `rspotify::model::SimplifiedAlbum` into `state::Album`
        let albums = albums
            .into_iter()
            .filter_map(Album::try_from_simplified_album)
            .collect();
        Ok(AppClient::process_artist_albums(albums))
    }

    /// Start a playback
    async fn start_playback(&self, playback: Playback, device_id: Option<&str>) -> Result<()> {
        match playback {
            Playback::Context(id, offset) => match id {
                ContextId::Album(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?;
                }
                ContextId::Artist(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?;
                }
                ContextId::Playlist(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?;
                }
                ContextId::Show(id) => {
                    self.start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?;
                }
                ContextId::Tracks(_) => {
                    anyhow::bail!("`StartPlayback` request for `tracks` context is not supported")
                }
            },
            Playback::URIs(ids, offset) => {
                self.start_uris_playback(ids, device_id, offset, None)
                    .await?;
            }
        }

        Ok(())
    }

    /// Get recommendation (radio) tracks based on a seed
    pub async fn radio_tracks(&self, seed_uri: String) -> Result<Vec<Track>> {
        #[derive(Debug, Deserialize)]
        struct TrackData {
            original_gid: String,
        }
        #[derive(Debug, Deserialize)]
        struct RadioStationResponse {
            tracks: Vec<TrackData>,
        }

        let session = self.session().await;

        // Get an autoplay URI from the seed URI.
        // The return URI is a Spotify station's URI
        let autoplay_query_url = format!("hm://autoplay-enabled/query?uri={seed_uri}");
        let response = session
            .mercury()
            .get(autoplay_query_url)
            .map_err(|err| anyhow::anyhow!("Failed to get autoplay URI: {err:#}"))?
            .await?;
        if response.status_code != 200 {
            anyhow::bail!(
                "Failed to get autoplay URI: got non-OK status code: {}",
                response.status_code
            );
        }
        let autoplay_uri = String::from_utf8(response.payload[0].clone())?;

        // Retrieve radio's data based on the autoplay URI
        let radio_query_url = format!("hm://radio-apollo/v3/stations/{autoplay_uri}");
        let response = session
            .mercury()
            .get(radio_query_url)
            .map_err(|err| anyhow::anyhow!("Failed to get radio data of {autoplay_uri}: {err:#}"))?
            .await?;
        if response.status_code != 200 {
            anyhow::bail!(
                "Failed to get radio data of {autoplay_uri}: got non-OK status code: {}",
                response.status_code
            );
        }

        // Parse a list consisting of IDs of tracks inside the radio station
        let track_ids = serde_json::from_slice::<RadioStationResponse>(&response.payload[0])?
            .tracks
            .into_iter()
            .filter_map(|t| TrackId::from_id(t.original_gid).ok());

        // Retrieve tracks based on IDs
        let tracks = self
            .tracks(track_ids, Some(rspotify::model::Market::FromToken))
            .await?;
        let tracks = tracks
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect();

        Ok(tracks)
    }

    /// Search for items (tracks, artists, albums, playlists) matching a given query
    pub async fn search(&self, query: &str) -> Result<SearchResults> {
        let (
            track_result,
            artist_result,
            album_result,
            playlist_result,
            show_result,
            episode_result,
        ) = tokio::try_join!(
            self.search_specific_type(query, rspotify::model::SearchType::Track),
            self.search_specific_type(query, rspotify::model::SearchType::Artist),
            self.search_specific_type(query, rspotify::model::SearchType::Album),
            self.search_specific_type(query, rspotify::model::SearchType::Playlist),
            self.search_specific_type(query, rspotify::model::SearchType::Show),
            self.search_specific_type(query, rspotify::model::SearchType::Episode)
        )?;

        let (tracks, artists, albums, playlists, shows, episodes) = (
            match track_result {
                rspotify::model::SearchResult::Tracks(p) => p
                    .items
                    .into_iter()
                    .filter_map(Track::try_from_full_track)
                    .collect(),
                _ => anyhow::bail!("expect a track search result"),
            },
            match artist_result {
                rspotify::model::SearchResult::Artists(p) => {
                    p.items.into_iter().map(std::convert::Into::into).collect()
                }
                _ => anyhow::bail!("expect an artist search result"),
            },
            match album_result {
                rspotify::model::SearchResult::Albums(p) => p
                    .items
                    .into_iter()
                    .filter_map(Album::try_from_simplified_album)
                    .collect(),
                _ => anyhow::bail!("expect an album search result"),
            },
            match playlist_result {
                rspotify::model::SearchResult::Playlists(p) => {
                    p.items.into_iter().map(std::convert::Into::into).collect()
                }
                _ => anyhow::bail!("expect a playlist search result"),
            },
            match show_result {
                rspotify::model::SearchResult::Shows(p) => {
                    p.items.into_iter().map(std::convert::Into::into).collect()
                }
                _ => anyhow::bail!("expect a show search result"),
            },
            match episode_result {
                rspotify::model::SearchResult::Episodes(p) => {
                    p.items.into_iter().map(std::convert::Into::into).collect()
                }
                _ => anyhow::bail!("expect a episode search result"),
            },
        );

        Ok(SearchResults {
            tracks,
            artists,
            albums,
            playlists,
            shows,
            episodes,
        })
    }

    /// Search for items of a specific type matching a given query
    pub async fn search_specific_type(
        &self,
        query: &str,
        typ: rspotify::model::SearchType,
    ) -> Result<rspotify::model::SearchResult> {
        Ok(self
            .spotify
            .search(query, typ, None, None, None, None)
            .await?)
    }

    /// Add a playable item to a playlist
    pub async fn add_item_to_playlist(
        &self,
        state: &SharedState,
        playlist_id: PlaylistId<'_>,
        playable_id: PlayableId<'_>,
    ) -> Result<()> {
        // remove all the occurrences of the track to ensure no duplication in the playlist
        self.playlist_remove_all_occurrences_of_items(
            playlist_id.as_ref(),
            [playable_id.as_ref()],
            None,
        )
        .await?;

        self.playlist_add_items(playlist_id.as_ref(), [playable_id.as_ref()], None)
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
        let insert_before = if insert_index > range_start {
            insert_index + 1
        } else {
            insert_index
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
                        state
                            .data
                            .write()
                            .user_data
                            .playlists
                            .insert(0, PlaylistFolderItem::Playlist(playlist));
                    }
                }
            }
            Item::Show(show) => {
                let follows = self.check_users_saved_shows([show.id.as_ref()]).await?;
                if !follows[0] {
                    self.save_shows([show.id.as_ref()]).await?;
                    // update the in-memory `user_data`
                    state.data.write().user_data.saved_shows.insert(0, show);
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
                    .retain(|item| match item {
                        PlaylistFolderItem::Playlist(p) => p.id != id,
                        PlaylistFolderItem::Folder(_) => true,
                    });
                self.playlist_unfollow(id).await?;
            }
            ItemId::Show(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .saved_shows
                    .retain(|s| s.id != id);
                self.remove_users_saved_shows([id], Some(rspotify::model::Market::FromToken))
                    .await?;
            }
        }
        Ok(())
    }

    /// Get a track data
    pub async fn track(&self, track_id: TrackId<'_>) -> Result<Track> {
        Track::try_from_full_track(
            self.spotify
                .track(track_id, Some(rspotify::model::Market::FromToken))
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
            .http_get::<rspotify::model::FullPlaylist>(
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
            .filter_map(Track::try_from_playlist_item)
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

        let album = self
            .album(album_id, Some(rspotify::model::Market::FromToken))
            .await?;
        let first_page = album.tracks.clone();

        // converts `rspotify::model::FullAlbum` into `state::Album`
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

        let artist = self
            .artist(artist_id.as_ref())
            .await
            .context("get artist")?
            .into();

        let top_tracks = self
            .artist_top_tracks(artist_id.as_ref(), Some(rspotify::model::Market::FromToken))
            .await
            .context("get artist's top tracks")?;
        let top_tracks = top_tracks
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect::<Vec<_>>();

        #[allow(deprecated)]
        let related_artists = self
            .artist_related_artists(artist_id.as_ref())
            .await
            .context("get related artists")?;
        let related_artists = related_artists
            .into_iter()
            .map(std::convert::Into::into)
            .collect::<Vec<_>>();

        let albums = self
            .artist_albums(artist_id.as_ref())
            .await
            .context("get artist's albums")?;

        Ok(Context::Artist {
            artist,
            top_tracks,
            albums,
            related_artists,
        })
    }

    /// Get a show context data
    pub async fn show_context(&self, show_id: ShowId<'_>) -> Result<Context> {
        let show_uri = show_id.uri();
        tracing::info!("Get show context: {}", show_uri);

        let show = self.get_a_show(show_id, None).await?;
        let first_page = show.episodes.clone();

        // Copy first_page but use Page<Option<SimplifiedEpisode>> instead of Page<SimplifiedEpisode>
        // This is a temporary fix for https://github.com/aome510/spotify-player/issues/663
        let first_page = rspotify::model::Page {
            items: first_page.items.into_iter().map(Some).collect(),
            href: first_page.href,
            limit: first_page.limit,
            next: first_page.next,
            offset: first_page.offset,
            previous: first_page.previous,
            total: first_page.total,
        };

        // converts `rspotify::model::FullShow` into `state::Show`
        let show: Show = show.into();

        // get the show's episodes
        let episodes = self
            .all_paging_items(first_page, &Query::new())
            .await?
            .into_iter()
            .flatten()
            .map(std::convert::Into::into)
            .collect::<Vec<_>>();

        Ok(Context::Show { show, episodes })
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
        fn process_spotify_api_response(text: &str) -> String {
            // See: https://github.com/ramsayleung/rspotify/issues/459
            text.replace("\"images\":null", "\"images\":[]")
                // See: https://github.com/aome510/spotify-player/issues/494
                // an item's name can be null while Spotify requires it to be available
                .replace("\"name\":null", "\"name\":\"\"")
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

        let status = response.status();
        let text = process_spotify_api_response(&response.text().await?);
        tracing::debug!("{text}");

        if status != StatusCode::OK {
            anyhow::bail!("failed to send a Spotify API request {url}: {text}");
        }

        Ok(serde_json::from_str(&text)?)
    }

    /// Get all paging items starting from a pagination object of the first page
    async fn all_paging_items<T>(
        &self,
        first_page: rspotify::model::Page<T>,
        payload: &Query<'_>,
    ) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;

        while let Some(url) = maybe_next {
            let mut next_page = self
                .http_get::<rspotify::model::Page<T>>(&url, payload)
                .await?;
            if next_page.items.is_empty() {
                break;
            }
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// Get all cursor-based paging items starting from a pagination object of the first page
    async fn all_cursor_based_paging_items<T>(
        &self,
        first_page: rspotify::model::CursorBasedPage<T>,
    ) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self
                .http_get::<rspotify::model::CursorBasedPage<T>>(&url, &Query::new())
                .await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    pub async fn current_playback2(
        &self,
    ) -> Result<Option<rspotify::model::CurrentPlaybackContext>> {
        Ok(self.current_playback(None, PLAYBACK_TYPES.into()).await?)
    }

    /// Retrieve the latest playback state
    pub async fn retrieve_current_playback(
        &self,
        state: &SharedState,
        reset_buffered_playback: bool,
    ) -> Result<()> {
        let new_playback = {
            // update the playback state
            let playback = self.current_playback2().await?;
            let mut player = state.player.write();

            let prev_item = player.currently_playing();

            let prev_name = match prev_item {
                Some(rspotify::model::PlayableItem::Track(track)) => track.name.clone(),
                Some(rspotify::model::PlayableItem::Episode(episode)) => episode.name.clone(),
                Some(rspotify::model::PlayableItem::Unknown(_)) | None => String::new(),
            };

            player.playback = playback;
            player.playback_last_updated_time = Some(std::time::Instant::now());

            let curr_item = player.currently_playing();

            let curr_name = match curr_item {
                Some(rspotify::model::PlayableItem::Track(track)) => track.name.clone(),
                Some(rspotify::model::PlayableItem::Episode(episode)) => episode.name.clone(),
                Some(rspotify::model::PlayableItem::Unknown(_)) | None => String::new(),
            };

            let new_playback = prev_name != curr_name && !curr_name.is_empty();
            // check if we need to update the buffered playback
            let needs_update = match (&player.buffered_playback, &player.playback) {
                (Some(bp), Some(p)) => bp.device_id != p.device.id || new_playback,
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

            new_playback
        };

        if !new_playback {
            return Ok(());
        }
        self.handle_new_playback_event(state).await?;

        Ok(())
    }

    // Handle new track event
    async fn handle_new_playback_event(&self, state: &SharedState) -> Result<()> {
        let configs = config::get_config();

        let curr_item = {
            let player = state.player.read();
            let Some(track_or_episode) = player.currently_playing() else {
                return Ok(());
            };
            track_or_episode.clone()
        };

        // retrieve current artist for genres if not in cache
        let curr_artist = match &curr_item {
            rspotify::model::PlayableItem::Track(full_track) => {
                let cached = state
                    .data
                    .read()
                    .caches
                    .genres
                    .contains_key(&full_track.artists[0].name);

                if cached {
                    None
                } else {
                    match &full_track.artists[0].id {
                        Some(id) => self.spotify.artist(id.clone()).await.ok(),
                        None => None,
                    }
                }
            }
            rspotify::model::PlayableItem::Episode(_)
            | rspotify::model::PlayableItem::Unknown(_) => None,
        };

        if let Some(artist) = curr_artist {
            if !artist.genres.is_empty() {
                state.data.write().caches.genres.insert(
                    artist.name,
                    artist.genres,
                    *TTL_CACHE_DURATION,
                );
            }
        }

        let url = match curr_item {
            rspotify::model::PlayableItem::Track(ref track) => {
                crate::utils::get_track_album_image_url(track)
                    .ok_or(anyhow::anyhow!("missing image"))?
            }
            rspotify::model::PlayableItem::Episode(ref episode) => {
                crate::utils::get_episode_show_image_url(episode)
                    .ok_or(anyhow::anyhow!("missing image"))?
            }
            rspotify::model::PlayableItem::Unknown(_) => return Ok(()),
        };

        let filename = (match curr_item {
            rspotify::model::PlayableItem::Track(ref track) => {
                format!(
                    "{}-{}-cover-{}.jpg",
                    track.album.name,
                    track.album.artists.first().unwrap().name,
                    // first 6 characters of the album's id
                    &track.album.id.as_ref().unwrap().id()[..6]
                )
            }
            rspotify::model::PlayableItem::Episode(ref episode) => {
                format!(
                    "{}-{}-cover-{}.jpg",
                    episode.show.name,
                    episode.show.publisher,
                    // first 6 characters of the show's id
                    &episode.show.id.as_ref().id()[..6]
                )
            }
            rspotify::model::PlayableItem::Unknown(_) => return Ok(()),
        })
        .replace('/', ""); // remove invalid characters from the file's name
        let path = configs.cache_folder.join("image").join(filename);

        if configs.app_config.enable_cover_image_cache {
            self.retrieve_image(url, &path, true).await?;
        }

        #[cfg(feature = "image")]
        if !state.data.read().caches.images.contains_key(url) {
            let bytes = self.retrieve_image(url, &path, false).await?;

            #[cfg(not(feature = "pixelate"))]
            let image =
                image::load_from_memory(&bytes).context("Failed to load image from memory")?;
            #[cfg(feature = "pixelate")]
            let mut image =
                image::load_from_memory(&bytes).context("Failed to load image from memory")?;

            #[cfg(feature = "pixelate")]
            {
                Self::pixelate_image(&mut image);
            }

            state
                .data
                .write()
                .caches
                .images
                .insert(url.to_owned(), image, *TTL_CACHE_DURATION);
        }

        // notify user about the playback's change if any
        #[cfg(all(feature = "notify", feature = "streaming"))]
        if configs.app_config.enable_notify
            && (!configs.app_config.notify_streaming_only || self.stream_conn.lock().is_some())
        {
            Self::notify_new_playback(&curr_item, &path)?;
        }

        #[cfg(all(feature = "notify", not(feature = "streaming")))]
        if configs.app_config.enable_notify {
            Self::notify_new_playback(&curr_item, &path)?;
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
        state
            .data
            .write()
            .user_data
            .playlists
            .insert(0, PlaylistFolderItem::Playlist(playlist));
        Ok(())
    }

    #[cfg(feature = "notify")]
    /// Create a notification for a new playback
    fn notify_new_playback(
        playable: &rspotify::model::PlayableItem,
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
                    "{track}" => {
                        let name = match playable {
                            rspotify::model::PlayableItem::Track(ref track) => &track.name,
                            rspotify::model::PlayableItem::Episode(ref episode) => &episode.name,
                            rspotify::model::PlayableItem::Unknown(_) => continue,
                        };
                        text += name;
                    }
                    "{artists}" => {
                        if let rspotify::model::PlayableItem::Track(ref track) = playable {
                            text += &crate::utils::map_join(&track.artists, |a| &a.name, ", ");
                        }
                    }
                    "{album}" => match playable {
                        rspotify::model::PlayableItem::Track(ref track) => {
                            text += &track.album.name;
                        }
                        rspotify::model::PlayableItem::Episode(ref episode) => {
                            text += &episode.show.name;
                        }
                        rspotify::model::PlayableItem::Unknown(_) => {}
                    },
                    &_ => {}
                }
            }
            if ptr < format_str.len() {
                text += &format_str[ptr..];
            }

            text
        };

        let configs = config::get_config();

        n.appname("spotify_player")
            .summary(&get_text_from_format_str(
                &configs.app_config.notify_format.summary,
            ))
            .body(&get_text_from_format_str(
                &configs.app_config.notify_format.body,
            ));
        if cover_img_path.exists() {
            n.icon(cover_img_path.to_str().context("valid cover_img_path")?);
        }
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
    async fn retrieve_image(
        &self,
        url: &str,
        path: &std::path::Path,
        saved: bool,
    ) -> Result<Vec<u8>> {
        if path.exists() {
            tracing::debug!("Retrieving image from file: {}", path.display());
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

    #[cfg(feature = "pixelate")]
    fn pixelate_image(image: &mut image::DynamicImage) {
        let pixels = config::get_config().app_config.cover_img_pixels;
        let pixelated_image = image.resize(pixels, pixels, image::imageops::FilterType::Nearest);
        *image = pixelated_image.resize(
            image.width(),
            image.height(),
            image::imageops::FilterType::Nearest,
        );
    }

    /// Process a list of albums, which includes
    /// - sort albums by the release date
    /// - sort albums by the type if `sort_artist_albums_by_type` config is enabled
    fn process_artist_albums(albums: Vec<Album>) -> Vec<Album> {
        let mut albums = albums.into_iter().collect::<Vec<_>>();

        albums.sort_by(|x, y| y.release_date.partial_cmp(&x.release_date).unwrap());

        if config::get_config().app_config.sort_artist_albums_by_type {
            fn get_priority(album_type: &str) -> usize {
                match album_type {
                    "album" => 0,
                    "single" => 1,
                    "appears_on" => 2,
                    "compilation" => 3,
                    _ => 4,
                }
            }
            albums.sort_by_key(|a| get_priority(&a.album_type()));
        }

        albums
    }
}
