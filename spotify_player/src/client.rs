use crate::event;
use crate::state;
use crate::utils;
use anyhow::{anyhow, Result};
use rspotify::{
    client::Spotify,
    model::*,
    oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo},
    senum::*,
};
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
        send: &std::sync::mpsc::Sender<event::Event>,
        event: event::Event,
    ) -> Result<()> {
        log::info!("handle the client event {:?}", event);

        let need_update_playback = match event {
            event::Event::GetDevices => {
                state.player.write().unwrap().devices = self.get_devices().await?;
                false
            }
            event::Event::GetCurrentPlayback => {
                self.update_current_playback_state(state).await?;
                false
            }
            event::Event::RefreshToken => {
                let expires_at = state.player.read().unwrap().auth_token_expires_at;
                if SystemTime::now() > expires_at {
                    state.player.write().unwrap().auth_token_expires_at =
                        self.refresh_token().await?;
                }
                false
            }
            event::Event::NextTrack => {
                self.next_track().await?;
                true
            }
            event::Event::PreviousTrack => {
                self.previous_track().await?;
                true
            }
            event::Event::ResumePause => {
                self.toggle_playing_state(state).await?;
                true
            }
            event::Event::SeekTrack(position_ms) => {
                self.seek_track(position_ms).await?;
                true
            }
            event::Event::Shuffle => {
                self.toggle_shuffle(state).await?;
                true
            }
            event::Event::Repeat => {
                self.cycle_repeat(state).await?;
                true
            }
            event::Event::PlayTrack(context_uri, uris, offset) => {
                self.start_playback(context_uri, uris, offset).await?;
                true
            }
            event::Event::PlayContext(uri) => {
                self.start_playback(Some(uri), None, None).await?;
                true
            }
            event::Event::TransferPlayback(device_id) => {
                self.transfer_playback(device_id).await?;
                true
            }
            event::Event::GetContext(context) => {
                match context {
                    event::Context::Playlist(playlist_uri) => {
                        self.get_playlist_context(playlist_uri, state).await?;
                    }
                    event::Context::Album(album_uri) => {
                        self.get_album_context(album_uri, state).await?;
                    }
                    event::Context::Artist(artist_uri) => {
                        self.get_artist_context(artist_uri, state).await?;
                    }
                    event::Context::Unknown(uri) => {
                        state
                            .player
                            .write()
                            .unwrap()
                            .context_cache
                            .put(uri.clone(), state::Context::Unknown(uri));
                    }
                };
                false
            }
        };

        if need_update_playback {
            utils::update_playback(state, send);
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
    pub async fn start_playback(
        &self,
        context_uri: Option<String>,
        uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .start_playback(None, context_uri, uris, offset, None)
                .await,
        )
    }

    /// transfers the current playback to another device
    pub async fn transfer_playback(&self, device_id: String) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.transfer_playback(&device_id, None).await)
    }

    /// gets a playlist given its id
    pub async fn get_playlist(&self, playlist_uri: &str) -> Result<playlist::FullPlaylist> {
        Self::handle_rspotify_result(self.spotify.playlist(playlist_uri, None, None).await)
    }

    /// gets an album given its id
    pub async fn get_album(&self, album_uri: &str) -> Result<album::FullAlbum> {
        Self::handle_rspotify_result(self.spotify.album(album_uri).await)
    }

    /// gets an artist given id
    pub async fn get_artist(&self, artist_uri: &str) -> Result<artist::FullArtist> {
        Self::handle_rspotify_result(self.spotify.artist(artist_uri).await)
    }

    /// gets a list of top tracks of an artist
    pub async fn get_artist_top_tracks(&self, artist_uri: &str) -> Result<track::FullTracks> {
        Self::handle_rspotify_result(self.spotify.artist_top_tracks(artist_uri, None).await)
    }

    /// gets all albums of an artist
    pub async fn get_artist_albums(&self, artist_uri: &str) -> Result<Vec<album::SimplifiedAlbum>> {
        let first_page = Self::handle_rspotify_result(
            self.spotify
                .artist_albums(artist_uri, None, None, None, None)
                .await,
        )?;
        self.get_all_paging_items(first_page).await
    }

    /// cycles through the repeat state of the current playback
    pub async fn cycle_repeat(&self, state: &state::SharedState) -> Result<()> {
        let player = state.player.read().unwrap();
        let state = Self::get_current_playback_state(&player)?;
        let next_repeat_state = match state.repeat_state {
            RepeatState::Off => RepeatState::Track,
            RepeatState::Track => RepeatState::Context,
            RepeatState::Context => RepeatState::Off,
        };
        Self::handle_rspotify_result(self.spotify.repeat(next_repeat_state, None).await)
    }

    /// toggles the shuffle state of the current playback
    pub async fn toggle_shuffle(&self, state: &state::SharedState) -> Result<()> {
        let player = state.player.read().unwrap();
        let state = Self::get_current_playback_state(&player)?;
        Self::handle_rspotify_result(self.spotify.shuffle(!state.shuffle_state, None).await)
    }

    /// toggles the current playing state (pause/resume a track)
    pub async fn toggle_playing_state(&self, state: &state::SharedState) -> Result<()> {
        match state.player.read().unwrap().playback {
            Some(ref playback) => {
                if playback.is_playing {
                    self.pause_track().await
                } else {
                    self.resume_track().await
                }
            }
            // TODO: find out if this works
            None => self.resume_track().await,
        }
    }

    /// seeks to a position in the current playing track
    pub async fn seek_track(&self, position_ms: u32) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.seek_track(position_ms, None).await)
    }

    /// resumes the previously paused/played track
    pub async fn resume_track(&self) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .start_playback(None, None, None, None, None)
                .await,
        )
    }

    /// pauses the currently playing track
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

    /// gets the current playing context
    pub async fn get_current_playback(&self) -> Result<Option<context::CurrentlyPlaybackContext>> {
        Self::handle_rspotify_result(self.spotify.current_playback(None, None).await)
    }

    /// gets a playlist context data
    async fn get_playlist_context(
        &self,
        playlist_uri: String,
        state: &state::SharedState,
    ) -> Result<()> {
        log::info!("get playlist context: {}", playlist_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&playlist_uri)
        {
            // get the playlist
            let playlist = self.get_playlist(&playlist_uri).await?;
            // get the playlist's tracks
            let playlist_tracks = self.get_all_paging_items(playlist.tracks.clone()).await?;
            // filter tracks that are either unaccessible or deleted from album
            let tracks = playlist_tracks
                .into_iter()
                .filter(|t| t.track.is_some())
                .map(|t| t.into())
                .collect::<Vec<_>>();
            state
                .player
                .write()
                .unwrap()
                .context_cache
                .put(playlist_uri, state::Context::Playlist(playlist, tracks));
        }

        Ok(())
    }

    /// gets an album context data
    async fn get_album_context(&self, album_uri: String, state: &state::SharedState) -> Result<()> {
        log::info!("get album context: {}", album_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&album_uri)
        {
            // get the album
            let album = self.get_album(&album_uri).await?;
            // get the album's tracks
            let album_tracks = self.get_all_paging_items(album.tracks.clone()).await?;
            // filter tracks that are either unaccessible or deleted from album
            let tracks = album_tracks
                .into_iter()
                .map(|t| {
                    let mut track: state::Track = t.into();
                    track.album = state::Album {
                        name: album.name.clone(),
                        id: Some(album.id.clone()),
                        uri: Some(album.uri.clone()),
                    };
                    track
                })
                .collect::<Vec<_>>();
            state
                .player
                .write()
                .unwrap()
                .context_cache
                .put(album_uri, state::Context::Album(album, tracks));
        }

        Ok(())
    }

    /// gets an artist context data
    async fn get_artist_context(
        &self,
        artist_uri: String,
        state: &state::SharedState,
    ) -> Result<()> {
        log::info!("get artist context: {}", artist_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&artist_uri)
        {
            // get a information, top tracks and all albums
            let artist = self.get_artist(&artist_uri).await?;
            let top_tracks = self
                .get_artist_top_tracks(&artist_uri)
                .await?
                .tracks
                .into_iter()
                .map(|t| t.into())
                .collect::<Vec<_>>();
            let albums = self
                .get_artist_albums(&artist_uri)
                .await?
                .into_iter()
                .map(|a| state::Album {
                    name: a.name,
                    uri: a.uri,
                    id: a.id,
                })
                .collect::<Vec<_>>();

            state.player.write().unwrap().context_cache.put(
                artist_uri,
                state::Context::Artist(artist, top_tracks, albums),
            );
        }
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

    /// gets the current playback state from the application state
    fn get_current_playback_state<'a>(
        player: &'a std::sync::RwLockReadGuard<'a, state::PlayerState>,
    ) -> Result<&'a context::CurrentlyPlaybackContext> {
        match player.playback {
            Some(ref playback) => Ok(playback),
            None => Err(anyhow!("failed to get the current playback context")),
        }
    }

    /// updates the current playback state by fetching data from the spotify client
    async fn update_current_playback_state(&self, state: &state::SharedState) -> Result<()> {
        let mut player = state.player.write().unwrap();
        player.playback_last_updated = Some(std::time::SystemTime::now());
        let playback = self.get_current_playback().await?;
        player.playback = playback;
        Ok(())
    }
}

#[tokio::main]
/// starts the client's event watcher
pub async fn start_watcher(
    state: state::SharedState,
    mut client: Client,
    send: std::sync::mpsc::Sender<event::Event>,
    recv: std::sync::mpsc::Receiver<event::Event>,
) {
    match client.get_current_user_playlists().await {
        Ok(playlists) => {
            log::info!("user's playlists: {:#?}", playlists);
            // update the state
            if !playlists.is_empty() {
                state
                    .ui
                    .lock()
                    .unwrap()
                    .playlists_list_ui_state
                    .select(Some(0));
            }
            state.player.write().unwrap().user_playlists = playlists;
        }
        Err(err) => {
            log::warn!("failed to get user's playlists: {:#?}", err);
        }
    }

    while let Ok(event) = recv.recv() {
        if let Err(err) = client.handle_event(&state, &send, event).await {
            log::warn!("{:#?}", err);
        }
    }
}
