use crate::event;
use crate::prelude::*;
use crate::state;

/// A spotify client
pub struct Client {
    spotify: Spotify,
    http: reqwest::Client,
    oauth: SpotifyOAuth,
}

impl Client {
    /// returns the new `Client`
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
        log::info!("handle event: {:?}", event);
        match event {
            event::Event::RefreshToken => {
                state.write().unwrap().auth_token_expires_at = self.refresh_token().await?;
            }
            event::Event::NextTrack => {
                self.next_track().await?;
            }
            event::Event::PreviousTrack => {
                self.previous_track().await?;
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
            event::Event::Quit => {
                state.write().unwrap().is_running = false;
            }
            event::Event::GetPlaylist(playlist_id) => {
                if let Some(playlist) = state.read().unwrap().current_playlist.as_ref() {
                    // avoid getting the same playlist more than once
                    if playlist.id == playlist_id {
                        return Ok(());
                    }
                }
                // get the playlist
                let playlist = self.get_playlist(&playlist_id).await?;
                state.write().unwrap().current_playlist = Some(playlist);
                // get the playlist's track
                let tracks = self
                    .get_current_playlist_tracks(&state.read().unwrap())
                    .await?;
                // update the state (UI) of the `playlist_tracks_widget`
                if !tracks.is_empty() {
                    state
                        .write()
                        .unwrap()
                        .ui_playlist_tracks_list_state
                        .select(Some(0));
                }
                state.write().unwrap().current_playlist_tracks = Some(tracks);
            }
            event::Event::SelectNextTrack => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.ui_playlist_tracks_list_state.selected() {
                    state.ui_playlist_tracks_list_state.select(Some(id + 1));
                }
            }
            event::Event::SelectPreviousTrack => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.ui_playlist_tracks_list_state.selected() {
                    if id > 0 {
                        state.ui_playlist_tracks_list_state.select(Some(id - 1));
                    }
                }
            }
        }
        Ok(())
    }

    /// handles a client error
    pub fn handle_error(&self, err: anyhow::Error) {
        log::warn!("client error: {:#}", err);
    }

    /// refreshes the client's authentication token, returns
    /// the token's `expires_at` time.
    pub async fn refresh_token(&mut self) -> Result<std::time::SystemTime> {
        let token = match get_token(&mut self.oauth).await {
            Some(token) => token,
            None => return Err(anyhow!("auth failed")),
        };

        let expires_at = token
            .expires_at
            .expect("got `None` for token's `expires_at`");
        self.spotify = Self::get_spotify_client(token);
        Ok(
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(expires_at as u64)
                - std::time::Duration::from_secs(10),
        )
    }

    // client functions

    pub async fn get_current_playlist_tracks(
        &self,
        state: &RwLockReadGuard<'_, state::State>,
    ) -> Result<Vec<playlist::PlaylistTrack>> {
        let mut tracks: Vec<playlist::PlaylistTrack> = vec![];
        if let Some(playlist) = state.current_playlist.as_ref() {
            tracks = playlist.tracks.items.clone();
            let mut next = playlist.tracks.next.clone();
            while let Some(url) = next.as_ref() {
                let mut paged_tracks = self
                    .internal_call::<page::Page<playlist::PlaylistTrack>>(url)
                    .await?;
                tracks.append(&mut paged_tracks.items);
                next = paged_tracks.next;
            }
        }
        Ok(tracks)
    }

    pub async fn get_playlist(&self, playlist_id: &str) -> Result<playlist::FullPlaylist> {
        Self::handle_rspotify_result(self.spotify.playlist(playlist_id, None, None).await)
    }

    /// cycles through the repeat state of the current playback
    pub async fn cycle_repeat(&self, state: &RwLockReadGuard<'_, state::State>) -> Result<()> {
        let state = Self::get_current_playback_state(&state)?;
        let next_repeat_state = match state.repeat_state {
            RepeatState::Off => RepeatState::Track,
            RepeatState::Track => RepeatState::Context,
            RepeatState::Context => RepeatState::Off,
        };
        Self::handle_rspotify_result(self.spotify.repeat(next_repeat_state, None).await)
    }

    /// toggles the shuffle state of the current playback
    pub async fn toggle_shuffle(&self, state: &RwLockReadGuard<'_, state::State>) -> Result<()> {
        let state = Self::get_current_playback_state(&state)?;
        Self::handle_rspotify_result(self.spotify.shuffle(!state.shuffle_state, None).await)
    }

    /// toggles the current playing state (pause/resume a track)
    pub async fn toggle_playing_state(
        &self,
        state: &RwLockReadGuard<'_, state::State>,
    ) -> Result<()> {
        let state = Self::get_current_playback_state(&state)?;
        if state.is_playing {
            self.pause_track().await
        } else {
            self.resume_track().await
        }
    }

    /// resumes a previously paused/played track
    pub async fn resume_track(&self) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .start_playback(None, None, None, None, None)
                .await,
        )
    }

    /// pauses currently playing track
    pub async fn pause_track(&self) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.pause_playback(None).await)
    }

    /// skips to the next track
    pub async fn next_track(&self) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.next_track(None).await)
    }

    /// skips to the previous track
    pub async fn previous_track(&self) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.previous_track(None).await)
    }

    /// returns the current playing context
    pub async fn get_current_playback(&self) -> Result<Option<context::CurrentlyPlaybackContext>> {
        Self::handle_rspotify_result(self.spotify.current_playback(None, None).await)
    }

    // helper functions

    async fn get_auth_token(&self) -> String {
        format!(
            "Bearer {}",
            self.spotify
                .client_credentials_manager
                .as_ref()
                .expect("client credentials manager is `None`")
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

    fn get_spotify_client(token: TokenInfo) -> Spotify {
        let client_credential = SpotifyClientCredentials::default()
            .token_info(token)
            .build();
        Spotify::default()
            .client_credentials_manager(client_credential)
            .build()
    }

    /// converts a `rspotify` result format into `anyhow` compatible result format
    fn handle_rspotify_result<T, E: fmt::Display>(result: std::result::Result<T, E>) -> Result<T> {
        match result {
            Ok(data) => Ok(data),
            Err(err) => Err(anyhow!(format!("{}", err))),
        }
    }

    /// gets the current playing state from the application state
    fn get_current_playback_state<'a>(
        state: &'a RwLockReadGuard<'a, state::State>,
    ) -> Result<&'a context::CurrentlyPlaybackContext> {
        match state.current_playback_context.as_ref() {
            Some(state) => Ok(state),
            None => Err(anyhow!("unable to get the currently playing context")),
        }
    }
}
