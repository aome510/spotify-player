use std::{borrow::Cow, io::Write, sync::Arc};

#[cfg(feature = "streaming")]
use crate::streaming;
use crate::{
    auth::AuthConfig,
    event::{ClientRequest, PlayerRequest},
    state::*,
};

use anyhow::Context as _;
use anyhow::Result;
#[cfg(feature = "streaming")]
use librespot_connect::spirc::Spirc;
use librespot_core::session::Session;
use rspotify::prelude::*;

mod handlers;
mod spotify;

pub use handlers::*;
use serde::Deserialize;

/// The application's client
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    pub spotify: Arc<spotify::Spotify>,
    pub client_pub: flume::Sender<ClientRequest>,
    #[cfg(feature = "streaming")]
    stream_conn: Arc<Mutex<Option<Spirc>>>,
}

#[derive(Debug, Deserialize)]
struct RadioStationResponse {
    tracks: Vec<TrackData>,
}

#[derive(Debug, Deserialize)]
struct TrackData {
    original_gid: String,
}

impl Client {
    /// creates a new client
    pub fn new(
        session: Session,
        auth_config: AuthConfig,
        client_id: String,
        client_pub: flume::Sender<ClientRequest>,
    ) -> Self {
        Self {
            spotify: Arc::new(spotify::Spotify::new(session, auth_config, client_id)),
            http: reqwest::Client::new(),
            #[cfg(feature = "streaming")]
            stream_conn: Arc::new(Mutex::new(None)),
            client_pub,
        }
    }

    pub async fn new_session(&self, state: &SharedState) -> Result<()> {
        let session = crate::auth::new_session(&self.spotify.auth_config, false).await?;
        *self.spotify.session.lock().await = Some(session);
        tracing::info!("Used a new session for Spotify client.");

        // upon creating a new session, also create a new streaming connection
        #[cfg(feature = "streaming")]
        {
            self.new_streaming_connection(state).await;
            self.client_pub.send(ClientRequest::ConnectDevice(None))?;
        }

        Ok(())
    }

    /// creates a new streaming connection
    #[cfg(feature = "streaming")]
    pub async fn new_streaming_connection(&self, state: &SharedState) -> String {
        let session = self.spotify.session().await;
        let device_id = session.device_id().to_string();
        let new_conn = streaming::new_connection(
            session,
            state.app_config.device.clone(),
            self.client_pub.clone(),
        );

        let mut stream_conn = self.stream_conn.lock();
        // shutdown old streaming connection and replace it with a new connection
        if let Some(conn) = stream_conn.as_ref() {
            conn.shutdown();
        }
        *stream_conn = Some(new_conn);

        device_id
    }

    /// initializes the authentication token inside the Spotify client
    pub async fn init_token(&self) -> Result<()> {
        self.spotify.refresh_token().await?;
        Ok(())
    }

    /// handles a player request
    pub async fn handle_player_request(
        &self,
        state: &SharedState,
        request: PlayerRequest,
    ) -> Result<()> {
        // `TransferPlayback` needs to be handled separately
        // from other play requests because they don't require an active playback
        // transfer the current playback to another device
        if let PlayerRequest::TransferPlayback(device_id, force_play) = request {
            self.spotify
                .transfer_playback(&device_id, Some(force_play))
                .await?;

            tracing::info!("Transfered the playback to device with {} id", device_id);
            return Ok(());
        }

        let mut playback = match state.player.read().buffered_playback {
            Some(ref playback) => playback.clone(),
            None => {
                anyhow::bail!("failed to handle the player request: no playback found");
            }
        };
        let device_id = playback.device_id.as_deref();

        match request {
            PlayerRequest::NextTrack => self.spotify.next_track(device_id).await?,
            PlayerRequest::PreviousTrack => self.spotify.previous_track(device_id).await?,
            PlayerRequest::ResumePause => {
                if !playback.is_playing {
                    self.spotify.resume_playback(device_id, None).await?
                } else {
                    self.spotify.pause_playback(device_id).await?
                }
                playback.is_playing = !playback.is_playing;
                state.player.write().buffered_playback = Some(playback);
            }
            PlayerRequest::SeekTrack(position_ms) => {
                self.spotify.seek_track(position_ms, device_id).await?
            }
            PlayerRequest::Repeat => {
                let next_repeat_state = match playback.repeat_state {
                    rspotify_model::RepeatState::Off => rspotify_model::RepeatState::Track,
                    rspotify_model::RepeatState::Track => rspotify_model::RepeatState::Context,
                    rspotify_model::RepeatState::Context => rspotify_model::RepeatState::Off,
                };

                self.spotify.repeat(next_repeat_state, device_id).await?;

                playback.repeat_state = next_repeat_state;
                state.player.write().buffered_playback = Some(playback);
            }
            PlayerRequest::Shuffle => {
                self.spotify
                    .shuffle(!playback.shuffle_state, device_id)
                    .await?;

                playback.shuffle_state = !playback.shuffle_state;
                state.player.write().buffered_playback = Some(playback);
            }
            PlayerRequest::Volume(volume) => {
                self.spotify.volume(volume, device_id).await?;

                playback.volume = Some(volume as u32);
                state.player.write().buffered_playback = Some(playback);
            }
            PlayerRequest::StartPlayback(p) => {
                self.start_playback(p, device_id).await?;
                // for some reasons, when starting a new playback, the integrated `spotify_player`
                // client doesn't respect the initial shuffle state, so we need to manually update the state
                self.spotify
                    .shuffle(playback.shuffle_state, device_id)
                    .await?
            }
            PlayerRequest::TransferPlayback(..) => {
                anyhow::bail!("`TransferPlayback` should be handled ealier")
            }
        };

        Ok(())
    }

    /// handles a client request
    pub async fn handle_request(&self, state: &SharedState, request: ClientRequest) -> Result<()> {
        let timer = std::time::SystemTime::now();

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
                        .insert(query, result, *CACHE_DURATION);
                }
            }
            ClientRequest::ConnectDevice(id) => {
                self.connect_device(state, id).await;
            }
            #[cfg(feature = "streaming")]
            ClientRequest::NewStreamingConnection => {
                let device_id = self.new_streaming_connection(state).await;
                // upon creating a new streaming connection, connect to it
                self.connect_device(state, Some(device_id)).await;
            }
            ClientRequest::GetCurrentUser => {
                let user = self.spotify.current_user().await?;
                state.data.write().user_data.user = Some(user);
            }
            ClientRequest::Player(request) => {
                self.handle_player_request(state, request).await?;
                self.update_playback(state);
            }
            ClientRequest::GetCurrentPlayback => {
                self.update_current_playback_state(state).await?;
            }
            ClientRequest::GetDevices => {
                let devices = self.spotify.device().await?;
                state.player.write().devices = devices
                    .into_iter()
                    .filter_map(Device::try_from_device)
                    .collect();
            }
            ClientRequest::GetUserPlaylists => {
                let playlists = self.current_user_playlists().await?;
                state.data.write().user_data.playlists = playlists;
            }
            ClientRequest::GetUserFollowedArtists => {
                let artists = self.current_user_followed_artists().await?;
                state.data.write().user_data.followed_artists = artists;
            }
            ClientRequest::GetUserSavedAlbums => {
                let albums = self.current_user_saved_albums().await?;
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
                        *CACHE_DURATION,
                    );
                }
            }
            ClientRequest::GetUserSavedTracks => {
                let tracks = self.current_user_saved_tracks().await?;
                state.data.write().caches.context.insert(
                    USER_LIKED_TRACKS_ID.uri.to_owned(),
                    Context::Tracks {
                        tracks: tracks.clone(),
                        desc: "User's liked tracks".to_string(),
                    },
                    *CACHE_DURATION,
                );
                state.data.write().user_data.saved_tracks = tracks;
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
                        *CACHE_DURATION,
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
                        .insert(uri, context, *CACHE_DURATION);
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
                        .insert(query, results, *CACHE_DURATION);
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
                        *CACHE_DURATION,
                    );
                }
            }
            ClientRequest::AddTrackToQueue(track_id) => {
                self.add_track_to_queue(track_id).await?;
            }
            ClientRequest::AddTrackToPlaylist(playlist_id, track_id) => {
                self.add_track_to_playlist(state, playlist_id, track_id)
                    .await?;
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
                let queue = self.spotify.current_user_queue().await?;
                {
                    state.player.write().queue = Some(queue);
                }
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
        };

        tracing::info!(
            "successfully handled the client request, took: {}ms",
            timer.elapsed().unwrap().as_millis()
        );

        Ok(())
    }

    pub async fn connect_device(&self, state: &SharedState, id: Option<String>) {
        // Device connection can fail when the specified device hasn't shown up
        // in the Spotify's server, which makes the `TransferPlayback` request fail
        // with an error like "404 Not Found".
        // This is why we need a retry mechanism to make multiple connect requests.
        let delay = std::time::Duration::from_secs(1);

        for _ in 0..10 {
            tokio::time::sleep(delay).await;

            let id = match &id {
                Some(id) => Some(Cow::Borrowed(id)),
                None => {
                    // no device id is specified, try to connect to an available device
                    match self.find_available_device(state).await {
                        Ok(Some(id)) => Some(Cow::Owned(id)),
                        Ok(None) => {
                            tracing::info!("No device found.");
                            None
                        }
                        Err(err) => {
                            tracing::error!("Failed to find an available device: {err:#}");
                            None
                        }
                    }
                }
            };

            if let Some(id) = id {
                tracing::info!("Trying to connect to device (id={id})");
                if let Err(err) = self.spotify.transfer_playback(&id, Some(false)).await {
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
        // After handling a request that updates the player's playback,
        // update the playback state by making additional refresh requests.
        //
        // # Why needs more than one request to update the playback?
        // It may take a while for Spotify to update the new change,
        // making additional requests can help ensure that
        // the playback state is always in sync with the latest change.
        let client = self.clone();
        let state = state.clone();
        tokio::task::spawn(async move {
            let delay = std::time::Duration::from_secs(1);
            for _ in 0..5 {
                tokio::time::sleep(delay).await;
                if let Err(err) = client.update_current_playback_state(&state).await {
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
            .spotify
            .categories_manual(Some("EN"), None, Some(50), None)
            .await?;

        Ok(first_page.items.into_iter().map(Category::from).collect())
    }

    /// Get Spotify's available browse playlists of a given category
    pub async fn browse_category_playlists(&self, category_id: &str) -> Result<Vec<Playlist>> {
        let first_page = self
            .spotify
            .category_playlists_manual(category_id, None, Some(50), None)
            .await?;

        Ok(first_page.items.into_iter().map(Playlist::from).collect())
    }

    /// Find an available device. If found, return the device ID.
    pub async fn find_available_device(&self, state: &SharedState) -> Result<Option<String>> {
        let devices = self.spotify.device().await?.into_iter().collect::<Vec<_>>();
        tracing::info!("Available devices: {devices:?}");

        // convert a vector of `Device` items into `(name, id)` items
        let mut devices = devices
            .into_iter()
            .filter_map(|d| d.id.map(|id| (d.name, id)))
            .collect::<Vec<_>>();

        // Manually append the integrated device to the device list if `streaming` feature is enabled.
        // The integrated device may not show up in the device list returned by the Spotify API because
        // 1. The device is just initialized and hasn't been registered in Spotify server.
        //    Related issue/discussion: https://github.com/aome510/spotify-player/issues/79
        // 2. The device list is empty. This is because user doesn't specify their own client ID.
        //    By default, the application uses Spotify web app's client ID, which doesn't have
        //    access to user's active devices.
        #[cfg(feature = "streaming")]
        {
            let session = self.spotify.session().await;
            devices.push((
                state.app_config.device.name.clone(),
                session.device_id().to_string(),
            ));
        }

        if devices.is_empty() {
            return Ok(None);
        }

        // Prioritize the `default_device` specified in the application's configurations
        let id = if let Some(id) = devices
            .iter()
            .position(|d| d.0 == state.app_config.default_device)
        {
            // prioritize the default device (specified in the app configs) if available
            id
        } else {
            // else, use the first available device
            0
        };

        Ok(Some(devices.remove(id).1))
    }

    /// gets the saved (liked) tracks of the current user
    pub async fn current_user_saved_tracks(&self) -> Result<Vec<Track>> {
        let first_page = self
            .spotify
            .current_user_saved_tracks_manual(None, Some(50), None)
            .await?;

        let tracks = self.all_paging_items(first_page).await?;
        Ok(tracks
            .into_iter()
            .filter_map(|t| Track::try_from_full_track(t.track))
            .collect())
    }

    /// gets the recently played tracks of the current user
    pub async fn current_user_recently_played_tracks(&self) -> Result<Vec<Track>> {
        let first_page = self
            .spotify
            .current_user_recently_played(Some(50), None)
            .await?;

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

    /// gets the top tracks of the current user
    pub async fn current_user_top_tracks(&self) -> Result<Vec<Track>> {
        let first_page = self
            .spotify
            .current_user_top_tracks_manual(None, Some(50), None)
            .await?;

        let tracks = self.all_paging_items(first_page).await?;
        Ok(tracks
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect())
    }

    /// gets all playlists of the current user
    pub async fn current_user_playlists(&self) -> Result<Vec<Playlist>> {
        let first_page = self
            .spotify
            .current_user_playlists_manual(Some(50), None)
            .await?;

        let playlists = self.all_paging_items(first_page).await?;
        Ok(playlists.into_iter().map(|p| p.into()).collect())
    }

    /// gets all followed artists of the current user
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
                .internal_call::<rspotify_model::CursorPageFullArtists>(&url)
                .await?
                .artists;
            artists.append(&mut next_page.items);
            maybe_next = next_page.next;
        }

        // converts `rspotify_model::FullArtist` into `state::Artist`
        Ok(artists.into_iter().map(|a| a.into()).collect())
    }

    /// gets all saved albums of the current user
    pub async fn current_user_saved_albums(&self) -> Result<Vec<Album>> {
        let first_page = self
            .spotify
            .current_user_saved_albums_manual(None, Some(50), None)
            .await?;

        let albums = self.all_paging_items(first_page).await?;

        // converts `rspotify_model::SavedAlbum` into `state::Album`
        Ok(albums.into_iter().map(|a| a.album.into()).collect())
    }

    /// gets all albums of an artist
    pub async fn artist_albums(&self, artist_id: ArtistId<'_>) -> Result<Vec<Album>> {
        let mut singles = {
            let first_page = self
                .spotify
                .artist_albums_manual(
                    artist_id.as_ref(),
                    Some(rspotify_model::AlbumType::Single),
                    None,
                    Some(50),
                    None,
                )
                .await?;
            self.all_paging_items(first_page).await
        }?;
        let mut albums = {
            let first_page = self
                .spotify
                .artist_albums_manual(
                    artist_id.as_ref(),
                    Some(rspotify_model::AlbumType::Album),
                    None,
                    Some(50),
                    None,
                )
                .await?;
            self.all_paging_items(first_page).await
        }?;
        albums.append(&mut singles);

        // converts `rspotify_model::SimplifiedAlbum` into `state::Album`
        let albums = albums
            .into_iter()
            .filter_map(Album::try_from_simplified_album)
            .collect();
        Ok(self.clean_up_artist_albums(albums))
    }

    /// starts a playback
    pub async fn start_playback(&self, playback: Playback, device_id: Option<&str>) -> Result<()> {
        match playback {
            Playback::Context(id, offset) => match id {
                ContextId::Album(id) => {
                    self.spotify
                        .start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?
                }
                ContextId::Artist(id) => {
                    self.spotify
                        .start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?
                }
                ContextId::Playlist(id) => {
                    self.spotify
                        .start_context_playback(PlayContextId::from(id), device_id, offset, None)
                        .await?
                }
                ContextId::Tracks(_) => {
                    anyhow::bail!("`StartPlayback` request for `tracks` context is not supported")
                }
            },
            Playback::URIs(track_ids, offset) => {
                self.spotify
                    .start_uris_playback(
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

    pub async fn radio_tracks(&self, seed_uri: String) -> Result<Vec<Track>> {
        let session = self.spotify.session().await;

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

        // Parse a list consisting of IDs of tracks inside the radio station
        let track_ids = serde_json::from_slice::<RadioStationResponse>(&response.payload[0])?
            .tracks
            .into_iter()
            .filter_map(|t| TrackId::from_id(t.original_gid).ok());

        // Retrieve tracks based on IDs
        let tracks = self
            .spotify
            .tracks(track_ids, None)
            .await?
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect();

        Ok(tracks)
    }

    /// searchs for items (tracks, artists, albums, playlists) that match a given query string.
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

    /// adds track to queue
    pub async fn add_track_to_queue(&self, track_id: TrackId<'_>) -> Result<()> {
        Ok(self
            .spotify
            .add_item_to_queue(PlayableId::Track(track_id), None)
            .await?)
    }

    /// adds track to a playlist
    pub async fn add_track_to_playlist(
        &self,
        state: &SharedState,
        playlist_id: PlaylistId<'_>,
        track_id: TrackId<'_>,
    ) -> Result<()> {
        // remove all the occurrences of the track to ensure no duplication in the playlist
        self.spotify
            .playlist_remove_all_occurrences_of_items(
                playlist_id.as_ref(),
                [PlayableId::Track(track_id.as_ref())],
                None,
            )
            .await?;

        self.spotify
            .playlist_add_items(
                playlist_id.as_ref(),
                [PlayableId::Track(track_id.as_ref())],
                None,
            )
            .await?;

        // After adding a new track to a playlist, remove the cache of that playlist to force refetching new data
        state.data.write().caches.context.remove(&playlist_id.uri());

        Ok(())
    }

    /// removes a track from a playlist
    pub async fn delete_track_from_playlist(
        &self,
        state: &SharedState,
        playlist_id: PlaylistId<'_>,
        track_id: TrackId<'_>,
    ) -> Result<()> {
        // remove all the occurrences of the track to ensure no duplication in the playlist
        self.spotify
            .playlist_remove_all_occurrences_of_items(
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

    /// reorder items in a playlist
    pub async fn reorder_playlist_items(
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

        self.spotify
            .playlist_reorder_items(
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

    /// adds a Spotify item to current user's library.
    /// Before adding new item, the function checks if that item already exists in the library
    /// to avoid adding a duplicated item.
    pub async fn add_to_library(&self, state: &SharedState, item: Item) -> Result<()> {
        match item {
            Item::Track(track) => {
                let contains = self
                    .spotify
                    .current_user_saved_tracks_contains([track.id.as_ref()])
                    .await?;
                if !contains[0] {
                    self.spotify
                        .current_user_saved_tracks_add([track.id.as_ref()])
                        .await?;
                    // update the in-memory `user_data`
                    state.data.write().user_data.saved_tracks.insert(0, track);
                }
            }
            Item::Album(album) => {
                let contains = self
                    .spotify
                    .current_user_saved_albums_contains([album.id.as_ref()])
                    .await?;
                if !contains[0] {
                    self.spotify
                        .current_user_saved_albums_add([album.id.as_ref()])
                        .await?;
                    // update the in-memory `user_data`
                    state.data.write().user_data.saved_albums.insert(0, album);
                }
            }
            Item::Artist(artist) => {
                let follows = self
                    .spotify
                    .user_artist_check_follow([artist.id.as_ref()])
                    .await?;
                if !follows[0] {
                    self.spotify
                        .user_follow_artists([artist.id.as_ref()])
                        .await?;
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
                        .spotify
                        .playlist_check_follow(playlist.id.as_ref(), &[user_id])
                        .await?;
                    if !follows[0] {
                        self.spotify
                            .playlist_follow(playlist.id.as_ref(), None)
                            .await?;
                        // update the in-memory `user_data`
                        state.data.write().user_data.playlists.insert(0, playlist);
                    }
                }
            }
        }
        Ok(())
    }

    // deletes a Spotify item from user's library
    pub async fn delete_from_library(&self, state: &SharedState, id: ItemId) -> Result<()> {
        match id {
            ItemId::Track(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .saved_tracks
                    .retain(|t| t.id != id);
                self.spotify.current_user_saved_tracks_delete([id]).await?;
            }
            ItemId::Album(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .saved_albums
                    .retain(|a| a.id != id);
                self.spotify.current_user_saved_albums_delete([id]).await?;
            }
            ItemId::Artist(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .followed_artists
                    .retain(|a| a.id != id);
                self.spotify.user_unfollow_artists([id]).await?;
            }
            ItemId::Playlist(id) => {
                state
                    .data
                    .write()
                    .user_data
                    .playlists
                    .retain(|p| p.id != id);
                self.spotify.playlist_unfollow(id).await?;
            }
        }
        Ok(())
    }

    /// gets a playlist context data
    pub async fn playlist_context(&self, playlist_id: PlaylistId<'_>) -> Result<Context> {
        let playlist_uri = playlist_id.uri();
        tracing::info!("Get playlist context: {}", playlist_uri);

        let playlist = self.spotify.playlist(playlist_id, None, None).await?;

        // get the playlist's tracks
        let first_page = playlist.tracks.clone();
        let tracks = self
            .all_paging_items(first_page)
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

    /// gets an album context data
    pub async fn album_context(&self, album_id: AlbumId<'_>) -> Result<Context> {
        let album_uri = album_id.uri();
        tracing::info!("Get album context: {}", album_uri);

        let album = self.spotify.album(album_id).await?;
        let first_page = album.tracks.clone();

        // converts `rspotify_model::FullAlbum` into `state::Album`
        let album: Album = album.into();

        // get the album's tracks
        let tracks = self
            .all_paging_items(first_page)
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

    /// gets an artist context data
    pub async fn artist_context(&self, artist_id: ArtistId<'_>) -> Result<Context> {
        let artist_uri = artist_id.uri();
        tracing::info!("Get artist context: {}", artist_uri);

        // get the artist's information, top tracks, related artists and albums
        let artist = self.spotify.artist(artist_id.as_ref()).await?.into();

        let top_tracks = self
            .spotify
            .artist_top_tracks(
                artist_id.as_ref(),
                rspotify_model::enums::misc::Market::FromToken,
            )
            .await?
            .into_iter()
            .filter_map(Track::try_from_full_track)
            .collect::<Vec<_>>();

        let related_artists = self
            .spotify
            .artist_related_artists(artist_id.as_ref())
            .await?
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

    /// calls a GET HTTP request to the Spotify server,
    /// and parses the response into a specific type `T`.
    async fn internal_call<T>(&self, url: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let access_token = self.spotify.access_token().await?;
        Ok(self
            .http
            .get(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {access_token}"),
            )
            .send()
            .await?
            .json::<T>()
            .await?)
    }

    /// gets all paging items starting from a pagination object of the first page
    async fn all_paging_items<T>(&self, first_page: rspotify_model::Page<T>) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self.internal_call::<rspotify_model::Page<T>>(&url).await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// gets all cursor-based paging items starting from a pagination object of the first page
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
                .internal_call::<rspotify_model::CursorBasedPage<T>>(&url)
                .await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// updates the current playback state
    pub async fn update_current_playback_state(&self, state: &SharedState) -> Result<()> {
        // update the playback state
        let new_track = {
            let playback = self.spotify.current_playback(None, None::<Vec<_>>).await?;
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

            if needs_update {
                // new playback updates, the buffered playback becomes invalid and needs to be updated
                player.buffered_playback = player.playback.as_ref().map(|p| SimplifiedPlayback {
                    device_name: p.device.name.clone(),
                    device_id: p.device.id.clone(),
                    is_playing: p.is_playing,
                    volume: p.device.volume_percent,
                    repeat_state: p.repeat_state,
                    shuffle_state: p.shuffle_state,
                });
            }

            new_track
        };

        if !new_track {
            return Ok(());
        }

        let track = match state.player.read().current_playing_track() {
            None => return Ok(()),
            Some(track) => track.clone(),
        };

        let url = match crate::utils::get_track_album_image_url(&track) {
            Some(url) => url,
            None => return Ok(()),
        };

        let path = state.cache_folder.join("image").join(format!(
            "{}-{}-cover.jpg",
            track.album.name,
            crate::utils::map_join(&track.album.artists, |a| &a.name, ", ")
        ));

        // Retrieve and save the new track's cover image into the cache folder.
        // The notify feature still requires the cover images to be stored inside the cache folder.
        if state.app_config.enable_cover_image_cache || cfg!(feature = "notify") {
            self.retrieve_image(url, &path, true).await?;
        }

        #[cfg(feature = "image")]
        if !state.data.read().caches.images.contains_key(url) {
            let bytes = self.retrieve_image(url, &path, false).await?;
            // Get the image from a url
            let image =
                image::load_from_memory(&bytes).context("Failed to load image from memory")?;

            state
                .data
                .write()
                .caches
                .images
                .insert(url.to_owned(), image, *CACHE_DURATION);
        }

        // notify user about the playback's change if any
        #[cfg(feature = "notify")]
        Self::notify_new_track(track, &path, state)?;

        Ok(())
    }

    #[cfg(feature = "notify")]
    fn notify_new_track(
        track: rspotify_model::FullTrack,
        path: &std::path::Path,
        state: &SharedState,
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

        n.appname("spotify_player")
            .icon(path.to_str().unwrap())
            .summary(&get_text_from_format_str(
                &state.app_config.notify_format.summary,
            ))
            .body(&get_text_from_format_str(
                &state.app_config.notify_format.body,
            ));

        n.show()?;

        Ok(())
    }

    /// retrieves an image from a `url` and saves it into a `path` (if specified)
    async fn retrieve_image(
        &self,
        url: &str,
        path: &std::path::Path,
        saved: bool,
    ) -> Result<Vec<u8>> {
        if path.exists() {
            tracing::info!("Retrieving an image from the file: {}", path.display());
            return Ok(std::fs::read(path)?);
        }

        tracing::info!("Retrieving an image from url: {url}");

        let bytes = self
            .http
            .get(url)
            .send()
            .await
            .context(format!("Failed to get image data from url {url}"))?
            .bytes()
            .await?;

        if saved {
            tracing::info!("Saving the retrieved image into {}", path.display());
            let mut file = std::fs::File::create(path)?;
            file.write_all(&bytes)?;
        }

        Ok(bytes.to_vec())
    }

    /// cleans up a list of albums, which includes
    /// - sort albums by the release date
    /// - remove albums with duplicated names
    fn clean_up_artist_albums(&self, albums: Vec<Album>) -> Vec<Album> {
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
