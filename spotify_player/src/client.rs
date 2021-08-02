use crate::event;
use crate::state;
use anyhow::{anyhow, Result};
use rspotify::{
    client::Spotify,
    model::*,
    oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo},
    senum::*,
};
use std::sync::RwLockReadGuard;
use std::time::{Duration, SystemTime};

/// A spotify client
pub struct Client {
    spotify: Spotify,
    http: reqwest::Client,
    oauth: SpotifyOAuth,
}

impl Client {
    /// creates the new `Client` given a spotify authorization
    pub fn new(oauth: SpotifyOAuth) -> Self {
        Self {
            spotify: Spotify::default(),
            http: reqwest::Client::new(),
            oauth,
        }
    }

    /// handles a client event
    pub async fn handle_event(
        &mut self,
        state: &state::SharedState,
        event: event::Event,
    ) -> Result<()> {
        log::info!("handle the client event {:?}", event);

        match event {
            event::Event::GetCurrentPlayback => {
                state.write().unwrap().playback = self.get_current_playback().await?;
            }
            event::Event::RefreshToken => {
                let expires_at = state.read().unwrap().auth_token_expires_at;
                if SystemTime::now() > expires_at {
                    state.write().unwrap().auth_token_expires_at = self.refresh_token().await?;
                }
            }
            event::Event::NextTrack => {
                self.next_track(state.read().unwrap().devices[0].id.clone())
                    .await?;
            }
            event::Event::PreviousTrack => {
                self.previous_track(state.read().unwrap().devices[0].id.clone())
                    .await?;
            }
            event::Event::ResumePause => {
                let state = state.read().unwrap();
                self.toggle_playing_state(&state).await?;
            }
            event::Event::Shuffle => {
                let state = state.read().unwrap();
                self.toggle_shuffle(&state).await?;
            }
            event::Event::Repeat => {
                let state = state.read().unwrap();
                self.cycle_repeat(&state).await?;
            }
            event::Event::PlaySelectedTrack => {
                let state = state.read().unwrap();
                if let (Some(id), Some(playback)) = (
                    state.context_tracks_table_ui_state.selected(),
                    state.playback.as_ref(),
                ) {
                    if let Some(ref context) = playback.context {
                        self.play_track_with_context(
                            context.uri.clone(),
                            Some(state.get_context_filtered_tracks()[id].uri.clone()),
                        )
                        .await?;
                    }
                }
            }
            event::Event::PlaySelectedPlaylist => {
                let state = state.read().unwrap();
                if let Some(id) = state.playlists_list_ui_state.selected() {
                    self.play_track_with_context(state.user_playlists[id].uri.clone(), None)
                        .await?;
                }
            }
            event::Event::PlaylistAsContext(playlist_id) => {
                self.update_context_to_playlist(playlist_id, state).await?;
            }
            event::Event::AlbumAsContext(album_id) => {
                self.update_context_to_album(album_id, state).await?;
            }
            event::Event::SearchTracksInContext => {
                self.search_tracks_in_current_playing_context(state).await?;
            }
            event::Event::SortTracksInContext(order) => {
                state.write().unwrap().sort_context_tracks(order);
            }
        }
        Ok(())
    }

    /// refreshes the client's authentication token.
    /// Returns the token's expired time.
    pub async fn refresh_token(&mut self) -> Result<SystemTime> {
        log::info!("refresh auth token...");

        let token = rspotify::util::get_token(&mut self.oauth)
            .await
            .expect("failed to get access token");
        let refresh_token = token
            .refresh_token
            .expect("failed to get the refresh token from the access token");
        let new_token = self
            .oauth
            .refresh_access_token(&refresh_token)
            .await
            .expect("failed to refresh the access token");

        let expires_at = SystemTime::UNIX_EPOCH
            + Duration::from_secs(
                new_token
                    .expires_at
                    .expect("failed to get token's `expires_at`") as u64,
            )
            - Duration::from_secs(10);

        // build a new spotify client from the new token
        self.spotify = Self::build_spotify_client(new_token);

        log::info!(
            "token will expire in {:?} seconds",
            expires_at
                .duration_since(SystemTime::now())
                .unwrap()
                .as_secs()
        );
        Ok(expires_at)
    }

    /// gets all available devices
    pub async fn get_devices(&self) -> Result<Vec<device::Device>> {
        Ok(Self::handle_rspotify_result(self.spotify.device().await)?.devices)
    }

    /// gets all playlists of the current user
    pub async fn get_current_user_playlists(&self) -> Result<Vec<playlist::SimplifiedPlaylist>> {
        let first_page =
            Self::handle_rspotify_result(self.spotify.current_user_playlists(None, None).await)?;
        Ok(self.get_all_paging_items(first_page).await?)
    }

    /// plays a track given a context URI
    pub async fn play_track_with_context(
        &self,
        context_uri: String,
        track_uri: Option<String>,
    ) -> Result<()> {
        let offset = match track_uri {
            None => None,
            Some(uri) => offset::for_uri(uri),
        };
        Self::handle_rspotify_result(
            self.spotify
                .start_playback(None, Some(context_uri), None, offset, None)
                .await,
        )
    }

    /// gets a playlist given its id
    pub async fn get_playlist(&self, playlist_id: &str) -> Result<playlist::FullPlaylist> {
        Self::handle_rspotify_result(self.spotify.playlist(playlist_id, None, None).await)
    }

    /// gets an album given its id
    pub async fn get_album(&self, album_id: &str) -> Result<album::FullAlbum> {
        Self::handle_rspotify_result(self.spotify.album(album_id).await)
    }

    /// cycles through the repeat state of the current playback
    pub async fn cycle_repeat(&self, state: &RwLockReadGuard<'_, state::State>) -> Result<()> {
        let state = Self::get_current_playback_state(state)?;
        let next_repeat_state = match state.repeat_state {
            RepeatState::Off => RepeatState::Track,
            RepeatState::Track => RepeatState::Context,
            RepeatState::Context => RepeatState::Off,
        };
        Self::handle_rspotify_result(self.spotify.repeat(next_repeat_state, None).await)
    }

    /// toggles the shuffle state of the current playback
    pub async fn toggle_shuffle(&self, state: &RwLockReadGuard<'_, state::State>) -> Result<()> {
        let state = Self::get_current_playback_state(state)?;
        Self::handle_rspotify_result(self.spotify.shuffle(!state.shuffle_state, None).await)
    }

    /// toggles the current playing state (pause/resume a track)
    pub async fn toggle_playing_state(
        &self,
        state: &RwLockReadGuard<'_, state::State>,
    ) -> Result<()> {
        let device_id = state.devices[0].id.clone();
        match state.playback {
            Some(ref playback) => {
                if playback.is_playing {
                    self.pause_track(device_id).await
                } else {
                    self.resume_track(device_id).await
                }
            }
            None => self.resume_track(device_id).await,
        }
    }

    /// resumes the previously paused/played track
    pub async fn resume_track(&self, device_id: String) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .start_playback(Some(device_id), None, None, None, None)
                .await,
        )
    }

    /// pauses the currently playing track
    pub async fn pause_track(&self, device_id: String) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.pause_playback(Some(device_id)).await)
    }

    /// skips to the next track
    pub async fn next_track(&self, device_id: String) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.next_track(Some(device_id)).await)
    }

    /// skips to the previous track
    pub async fn previous_track(&self, device_id: String) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.previous_track(Some(device_id)).await)
    }

    /// gets the current playing context
    pub async fn get_current_playback(&self) -> Result<Option<context::CurrentlyPlaybackContext>> {
        Self::handle_rspotify_result(self.spotify.current_playback(None, None).await)
    }

    /// searchs tracks in the current playing context and updates the context tracks table
    /// UI state accordingly.
    async fn search_tracks_in_current_playing_context(
        &self,
        state: &state::SharedState,
    ) -> Result<()> {
        let mut state = state.write().unwrap();
        if let Some(ref query) = state.context_search_state.query {
            let mut query = query.clone();
            query.remove(0); // remove the '/' character at the beginning of the query string

            log::info!("search tracks in context with query {}", query);
            state.context_search_state.tracks = state
                .get_contex_tracks()
                .into_iter()
                .filter(|&t| t.get_basic_info().to_lowercase().contains(&query))
                .cloned()
                .collect();

            // update the table ui state
            let id = if state.context_search_state.tracks.is_empty() {
                None
            } else {
                Some(0)
            };
            state.context_tracks_table_ui_state.select(id);
            log::info!(
                "after search, context_search_state.tracks = {:?}",
                state.context_search_state.tracks
            );
        }
        Ok(())
    }

    /// updates the playing context state to playlist
    async fn update_context_to_playlist(
        &self,
        playlist_id: String,
        state: &state::SharedState,
    ) -> Result<()> {
        if let state::PlayingContext::Playlist(ref playlist, _) = state.read().unwrap().context {
            // avoid getting the same playlist more than once
            if playlist.id == playlist_id {
                return Ok(());
            }
        }

        log::info!("update context as playlist with id {}", playlist_id);

        // get the playlist
        let playlist = self.get_playlist(&playlist_id).await?;
        // get the playlist's tracks
        let playlist_tracks = self.get_all_paging_items(playlist.tracks.clone()).await?;
        // filter tracks that are either unaccessible or deleted from album
        let tracks = playlist_tracks
            .into_iter()
            .filter(|t| t.track.is_some())
            .map(|t| t.into())
            .collect::<Vec<_>>();

        // update states
        let mut state = state.write().unwrap();
        if !tracks.is_empty() {
            state.context_tracks_table_ui_state.select(Some(0));
        }
        state.context = state::PlayingContext::Playlist(playlist, tracks);
        Ok(())
    }

    /// updates the playing context state to album
    async fn update_context_to_album(
        &self,
        album_id: String,
        state: &state::SharedState,
    ) -> Result<()> {
        if let state::PlayingContext::Album(ref album, _) = state.read().unwrap().context {
            // avoid getting the same album more than once
            if album.id == album_id {
                return Ok(());
            }
        }

        log::info!("update context as album with id {}", album_id);

        // get the album
        let album = self.get_album(&album_id).await?;
        // get the album's tracks
        let album_tracks = self.get_all_paging_items(album.tracks.clone()).await?;
        let tracks = album_tracks
            .into_iter()
            .map(|t| {
                let mut track: state::Track = t.into();
                track.album = state::Album {
                    id: Some(album.id.clone()),
                    uri: Some(album.uri.clone()),
                    name: album.name.clone(),
                };
                track
            })
            .collect::<Vec<_>>();

        // update states
        let mut state = state.write().unwrap();
        if !tracks.is_empty() {
            state.context_tracks_table_ui_state.select(Some(0));
        }
        state.context = state::PlayingContext::Album(album, tracks);
        Ok(())
    }

    async fn get_auth_token(&self) -> String {
        format!(
            "Bearer {}",
            self.spotify
                .client_credentials_manager
                .as_ref()
                .expect("failed to get spotify's client credentials manager")
                .get_access_token()
                .await
        )
    }

    async fn internal_call<T>(&self, url: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(self
            .http
            .get(url)
            .header(reqwest::header::AUTHORIZATION, self.get_auth_token().await)
            .send()
            .await?
            .json::<T>()
            .await?)
    }

    /// gets all paging items starting from a pagination object of the first page
    async fn get_all_paging_items<T>(&self, first_page: page::Page<T>) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next.clone();
        while let Some(url) = maybe_next {
            let mut next_page = self.internal_call::<page::Page<T>>(&url).await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// builds a spotify client from an authentication token
    fn build_spotify_client(token: TokenInfo) -> Spotify {
        let client_credential = SpotifyClientCredentials::default()
            .token_info(token)
            .build();
        Spotify::default()
            .client_credentials_manager(client_credential)
            .build()
    }

    /// handles a `rspotify` client result and converts it into `anyhow` compatible result format
    fn handle_rspotify_result<T, E: std::fmt::Display>(
        result: std::result::Result<T, E>,
    ) -> Result<T> {
        match result {
            Ok(data) => Ok(data),
            Err(err) => Err(anyhow!(format!("{}", err))),
        }
    }

    /// gets the current playing state from the application state
    fn get_current_playback_state<'a>(
        state: &'a RwLockReadGuard<'a, state::State>,
    ) -> Result<&'a context::CurrentlyPlaybackContext> {
        match state.playback {
            Some(ref playback) => Ok(playback),
            None => Err(anyhow!("failed to get the current playback context")),
        }
    }
}

#[tokio::main]
/// starts the client's event watcher
pub async fn start_watcher(
    state: state::SharedState,
    mut client: Client,
    recv: std::sync::mpsc::Receiver<event::Event>,
) {
    match client.get_current_user_playlists().await {
        Ok(playlists) => {
            log::info!("user's playlists: {:#?}", playlists);
            // update the state
            let mut state = state.write().unwrap();
            if !playlists.is_empty() {
                state.playlists_list_ui_state.select(Some(0));
            }
            state.user_playlists = playlists;
        }
        Err(err) => {
            log::warn!("failed to get user's playlists: {:#?}", err);
        }
    }

    while let Ok(event) = recv.recv() {
        if let Err(err) = client.handle_event(&state, event).await {
            log::warn!("{:#?}", err);
        }
    }
}
