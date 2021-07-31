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

#[derive(Clone)]
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
            event::Event::RefreshToken => {
                state.write().unwrap().auth_token_expires_at = self.refresh_token().await?;
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
                    state.current_playback_context.as_ref(),
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
                    self.play_track_with_context(state.current_playlists[id].uri.clone(), None)
                        .await?;
                }
            }
            event::Event::GetPlaylist(playlist_id) => {
                self.update_state_current_playlist(&playlist_id, state)
                    .await?;
            }
            event::Event::GetAlbum(album_id) => {
                self.update_state_current_album(&album_id, state).await?;
            }
            event::Event::SearchTrackInContext => {
                self.search_tracks_in_current_playing_context(state).await?;
            }
            event::Event::SortContextTracks(order) => {
                state.write().unwrap().sort_context_tracks(order);
            }
        }
        Ok(())
    }

    /// refreshes the client's authentication token.
    /// Returns the token's `expires_at` time.
    pub async fn refresh_token(&mut self) -> Result<std::time::SystemTime> {
        let token = match rspotify::util::get_token(&mut self.oauth).await {
            Some(token) => token,
            None => return Err(anyhow!("auth failed")),
        };

        let expires_at = token
            .expires_at
            .expect("got `None` for token's `expires_at`");
        self.spotify = Self::build_spotify_client(token);
        Ok(
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(expires_at as u64)
                - std::time::Duration::from_secs(10),
        )
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
        match state.current_playback_context {
            Some(ref context) => {
                if context.is_playing {
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
                .current_context_tracks
                .iter()
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

    /// updates the current playlist stored inside the application state
    async fn update_state_current_playlist(
        &self,
        playlist_id: &str,
        state: &state::SharedState,
    ) -> Result<()> {
        if let Some(ref playlist) = state.read().unwrap().current_playlist {
            // avoid getting the same playlist more than once
            if playlist.id == playlist_id {
                return Ok(());
            }
        }

        // get the playlist
        let playlist = self.get_playlist(playlist_id).await?;
        // get the playlist's tracks
        let playlist_tracks = self.get_all_paging_items(playlist.tracks.clone()).await?;
        // filter tracks that are either unaccessible or deleted from album
        let tracks: Vec<_> = playlist_tracks
            .into_iter()
            .filter(|t| t.track.is_some())
            .map(|t| t.into())
            .collect();

        // update states
        let mut state = state.write().unwrap();
        state.current_playlist = Some(playlist);
        if !tracks.is_empty() {
            state.context_tracks_table_ui_state.select(Some(0));
        }
        state.current_context_tracks = tracks;
        Ok(())
    }

    /// updates the current album stored inside the application state
    async fn update_state_current_album(
        &self,
        album_id: &str,
        state: &state::SharedState,
    ) -> Result<()> {
        if let Some(ref album) = state.read().unwrap().current_album {
            // avoid getting the same album more than once
            if album.id == album_id {
                return Ok(());
            }
        }

        // get the album
        let album = self.get_album(album_id).await?;
        // get the album's tracks
        let album_tracks = self.get_all_paging_items(album.tracks.clone()).await?;
        let tracks: Vec<_> = album_tracks
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
            .collect();

        // update states
        let mut state = state.write().unwrap();
        state.current_album = Some(album);
        if !tracks.is_empty() {
            state.context_tracks_table_ui_state.select(Some(0));
        }
        state.current_context_tracks = tracks;
        Ok(())
    }

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
        match state.current_playback_context {
            Some(ref state) => Ok(state),
            None => Err(anyhow!("unable to get the currently playing context")),
        }
    }
}

/// refreshes the current playback context every `playback_refresh_duration_in_ms` ms
async fn refresh_current_playback_context(state: state::SharedState, client: Client) {
    let playback_refresh_duration = std::time::Duration::from_millis(
        state
            .read()
            .unwrap()
            .app_config
            .playback_refresh_duration_in_ms,
    );
    loop {
        match client.get_current_playback().await {
            Ok(context) => {
                state.write().unwrap().current_playback_context = context;
            }
            Err(err) => {
                log::warn!("{:#?}", err);
            }
        }
        std::thread::sleep(playback_refresh_duration);
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
            state.current_playlists = playlists;
        }
        Err(err) => {
            log::warn!("failed to get user's playlists: {:#?}", err);
        }
    }
    tokio::task::spawn({
        let client = client.clone();
        let state = state.clone();
        async {
            refresh_current_playback_context(state, client).await;
        }
    });
    while let Ok(event) = recv.recv() {
        if let Err(err) = client.handle_event(&state, event).await {
            log::warn!("{:#?}", err);
        }
    }
}
