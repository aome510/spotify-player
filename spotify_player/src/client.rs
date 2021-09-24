use crate::{
    event::{ClientRequest, ContextURI, PlayerRequest},
    state::{Context, PageState, SharedState, Track},
    token, utils,
};
use anyhow::{anyhow, Result};
use librespot_core::session::Session;
use rspotify::{blocking::client::Spotify, model::*, senum::*};

/// A spotify client
#[derive(Clone)]
pub struct Client {
    spotify: Spotify,
    http: reqwest::Client,
}

impl Client {
    /// creates the new client
    pub fn new() -> Self {
        Self {
            spotify: Spotify::default(),
            http: reqwest::Client::new(),
        }
    }

    /// handles a player request
    fn handle_player_request(&self, state: &SharedState, event: PlayerRequest) -> Result<()> {
        log::info!("handle player request: {:?}", event);

        let player = state.player.read().unwrap();
        let playback = match player.playback {
            Some(ref playback) => playback,
            None => {
                return Err(anyhow!("failed to get the current playback context"));
            }
        };
        match event {
            PlayerRequest::NextTrack => self.next_track(playback),
            PlayerRequest::PreviousTrack => self.previous_track(playback),
            PlayerRequest::ResumePause => self.resume_pause(playback),
            PlayerRequest::SeekTrack(position_ms) => self.seek_track(playback, position_ms),
            PlayerRequest::Repeat => self.repeat(playback),
            PlayerRequest::Shuffle => self.shuffle(playback),
            PlayerRequest::Volume(volume) => self.volume(playback, volume),
            PlayerRequest::PlayTrack(context_uri, track_uris, offset) => {
                self.start_playback(playback, context_uri, track_uris, offset)
            }
            PlayerRequest::TransferPlayback(device_id, force_play) => {
                self.transfer_playback(device_id, force_play)
            }
        }
    }

    /// handles a client request
    pub async fn handle_request(&self, state: &SharedState, request: ClientRequest) -> Result<()> {
        log::info!("handle the client request {:?}", request);

        match request {
            ClientRequest::Player(event) => {
                self.handle_player_request(state, event)?;

                // After handling a request that modifies the player's playback,
                // try to update the playback state by making `n_refreshes` refresh requests.
                //
                // # Why needs more than one request to update the playback?
                // Spotify API may take a while to update with the new changes,
                // so we may need to make additional requests to ensure that
                // the current playback is synced with the latest change.
                let n_refreshes = state.app_config.n_refreshes_each_playback_update;
                let delay_duration = std::time::Duration::from_millis(
                    state.app_config.refresh_delay_in_ms_each_playback_update,
                );

                for _ in 0..n_refreshes {
                    std::thread::sleep(delay_duration);
                    self.update_current_playback_state(state)?;
                }
            }
            ClientRequest::GetCurrentPlayback => {
                self.update_current_playback_state(state)?;
            }
            ClientRequest::GetDevices => {
                let devices = self.get_devices()?;
                state.player.write().unwrap().devices = devices;
            }
            ClientRequest::GetUserPlaylists => {
                let playlists = self.get_current_user_playlists().await?;
                state.player.write().unwrap().user_playlists = playlists;
            }
            ClientRequest::GetUserFollowedArtists => {
                let artists = self
                    .get_current_user_followed_artists()
                    .await?
                    .into_iter()
                    .map(|a| a.into())
                    .collect::<Vec<_>>();
                state.player.write().unwrap().user_followed_artists = artists;
            }
            ClientRequest::GetUserSavedAlbums => {
                let albums = self
                    .get_current_user_saved_albums()
                    .await?
                    .into_iter()
                    .map(|a| a.album.into())
                    .collect::<Vec<_>>();
                state.player.write().unwrap().user_saved_albums = albums;
            }
            ClientRequest::GetContext(context) => {
                match context {
                    ContextURI::Playlist(playlist_uri) => {
                        self.get_playlist_context(playlist_uri, state).await?;
                    }
                    ContextURI::Album(album_uri) => {
                        self.get_album_context(album_uri, state).await?;
                    }
                    ContextURI::Artist(artist_uri) => {
                        self.get_artist_context(artist_uri, state).await?;
                    }
                    ContextURI::Unknown(uri) => {
                        state
                            .player
                            .write()
                            .unwrap()
                            .context_cache
                            .put(uri.clone(), Context::Unknown(uri));
                    }
                };
            }
        };

        Ok(())
    }

    /// gets all available devices
    pub fn get_devices(&self) -> Result<Vec<device::Device>> {
        Ok(Self::handle_rspotify_result(self.spotify.device())?.devices)
    }

    /// gets all playlists of the current user
    pub async fn get_current_user_playlists(&self) -> Result<Vec<playlist::SimplifiedPlaylist>> {
        let first_page =
            Self::handle_rspotify_result(self.spotify.current_user_playlists(50, None))?;
        self.get_all_paging_items(first_page).await
    }

    /// gets all followed artists of the current user
    pub async fn get_current_user_followed_artists(&self) -> Result<Vec<artist::FullArtist>> {
        let first_page =
            Self::handle_rspotify_result(self.spotify.current_user_followed_artists(50, None))?
                .artists;
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self
                .internal_call::<artist::CursorPageFullArtists>(&url)
                .await?
                .artists;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// gets all saved albums of the current user
    pub async fn get_current_user_saved_albums(&self) -> Result<Vec<album::SavedAlbum>> {
        let first_page =
            Self::handle_rspotify_result(self.spotify.current_user_saved_albums(50, None))?;
        self.get_all_paging_items(first_page).await
    }

    /// gets a playlist given its id
    pub fn get_playlist(&self, playlist_uri: &str) -> Result<playlist::FullPlaylist> {
        Self::handle_rspotify_result(self.spotify.playlist(playlist_uri, None, None))
    }

    /// gets an album given its id
    pub fn get_album(&self, album_uri: &str) -> Result<album::FullAlbum> {
        Self::handle_rspotify_result(self.spotify.album(album_uri))
    }

    /// gets an artist given id
    pub fn get_artist(&self, artist_uri: &str) -> Result<artist::FullArtist> {
        Self::handle_rspotify_result(self.spotify.artist(artist_uri))
    }

    /// gets a list of top tracks of an artist
    pub fn get_artist_top_tracks(&self, artist_uri: &str) -> Result<track::FullTracks> {
        Self::handle_rspotify_result(self.spotify.artist_top_tracks(artist_uri, None))
    }

    /// gets all albums of an artist
    pub async fn get_artist_albums(&self, artist_uri: &str) -> Result<Vec<album::SimplifiedAlbum>> {
        let mut singles = {
            let first_page = Self::handle_rspotify_result(self.spotify.artist_albums(
                artist_uri,
                Some(AlbumType::Single),
                None,
                Some(50),
                None,
            ))?;
            self.get_all_paging_items(first_page).await
        }?;
        let mut albums = {
            let first_page = Self::handle_rspotify_result(self.spotify.artist_albums(
                artist_uri,
                Some(AlbumType::Album),
                None,
                Some(50),
                None,
            ))?;
            self.get_all_paging_items(first_page).await
        }?;
        albums.append(&mut singles);
        Ok(self.clean_up_artist_albums(albums))
    }

    /// gets related artists from a given artist
    pub fn get_related_artists(&self, artist_uri: &str) -> Result<artist::FullArtists> {
        Self::handle_rspotify_result(self.spotify.artist_related_artists(artist_uri))
    }

    /// plays a track given a context URI
    pub fn start_playback(
        &self,
        playback: &context::CurrentlyPlaybackContext,
        context_uri: Option<String>,
        uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.start_playback(
            Some(playback.device.id.clone()),
            context_uri,
            uris,
            offset,
            None,
        ))?;
        // NOTE: for some reasons, `librespot` device does not keep the shuffle state
        // after starting a playback. A work around for this is to make an additional
        // shuffle request to keep the playback's original shuffle state.
        Self::handle_rspotify_result(
            self.spotify
                .shuffle(playback.shuffle_state, Some(playback.device.id.clone())),
        )
    }

    /// transfers the current playback to another device
    pub fn transfer_playback(&self, device_id: String, force_play: bool) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.transfer_playback(&device_id, Some(force_play)))
    }

    /// cycles through the repeat state of the current playback
    pub fn repeat(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        let next_repeat_state = match playback.repeat_state {
            RepeatState::Off => RepeatState::Track,
            RepeatState::Track => RepeatState::Context,
            RepeatState::Context => RepeatState::Off,
        };
        Self::handle_rspotify_result(
            self.spotify
                .repeat(next_repeat_state, Some(playback.device.id.clone())),
        )
    }

    /// toggles the current playing state (pause/resume a track)
    pub fn resume_pause(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        if playback.is_playing {
            self.pause_track(playback)
        } else {
            self.resume_track(playback)
        }
    }

    /// seeks to a position in the current playing track
    pub fn seek_track(
        &self,
        playback: &context::CurrentlyPlaybackContext,
        position_ms: u32,
    ) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .seek_track(position_ms, Some(playback.device.id.clone())),
        )
    }

    /// resumes the previously paused/played track
    pub fn resume_track(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.start_playback(
            Some(playback.device.id.clone()),
            None,
            None,
            None,
            None,
        ))
    }

    /// pauses the currently playing track
    pub fn pause_track(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .pause_playback(Some(playback.device.id.clone())),
        )
    }

    /// skips to the next track
    pub fn next_track(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.next_track(Some(playback.device.id.clone())))
    }

    /// skips to the previous track
    pub fn previous_track(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .previous_track(Some(playback.device.id.clone())),
        )
    }

    /// toggles the shuffle state of the current playback
    pub fn shuffle(&self, playback: &context::CurrentlyPlaybackContext) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .shuffle(!playback.shuffle_state, Some(playback.device.id.clone())),
        )
    }

    /// sets volume of the current playback
    pub fn volume(&self, playback: &context::CurrentlyPlaybackContext, volume: u8) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .volume(volume, Some(playback.device.id.clone())),
        )
    }

    /// gets the current playing context
    pub fn get_current_playback(&self) -> Result<Option<context::CurrentlyPlaybackContext>> {
        Self::handle_rspotify_result(self.spotify.current_playback(None, None))
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

    /// gets a playlist context data
    async fn get_playlist_context(&self, playlist_uri: String, state: &SharedState) -> Result<()> {
        log::info!("get playlist context: {}", playlist_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&playlist_uri)
        {
            // get the playlist
            let playlist = self.get_playlist(&playlist_uri)?;
            let first_page = playlist.tracks.clone();
            // get the playlist's tracks
            state.player.write().unwrap().context_cache.put(
                playlist_uri.clone(),
                Context::Playlist(
                    playlist,
                    first_page
                        .items
                        .clone()
                        .into_iter()
                        .filter(|t| t.track.is_some())
                        .map(|t| t.into())
                        .collect::<Vec<_>>(),
                ),
            );

            let playlist_tracks = self.get_all_paging_items(first_page).await?;

            // delay the request for getting playlist tracks to not block the UI

            // filter tracks that are either unaccessible or deleted from the playlist
            let tracks = playlist_tracks
                .into_iter()
                .filter(|t| t.track.is_some())
                .map(|t| t.into())
                .collect::<Vec<_>>();

            if let Some(Context::Playlist(_, ref mut old)) = state
                .player
                .write()
                .unwrap()
                .context_cache
                .peek_mut(&playlist_uri)
            {
                *old = tracks;
            }
        }

        Ok(())
    }

    /// gets an album context data
    async fn get_album_context(&self, album_uri: String, state: &SharedState) -> Result<()> {
        log::info!("get album context: {}", album_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&album_uri)
        {
            // get the album
            let album = self.get_album(&album_uri)?;
            // get the album's tracks
            let album_tracks = self.get_all_paging_items(album.tracks.clone()).await?;
            let tracks = album_tracks
                .into_iter()
                .map(|t| {
                    let mut track: Track = t.into();
                    track.album = album.clone().into();
                    track
                })
                .collect::<Vec<_>>();
            state
                .player
                .write()
                .unwrap()
                .context_cache
                .put(album_uri, Context::Album(album, tracks));
        }

        Ok(())
    }

    /// gets an artist context data
    async fn get_artist_context(&self, artist_uri: String, state: &SharedState) -> Result<()> {
        log::info!("get artist context: {}", artist_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&artist_uri)
        {
            // get a information, top tracks and all albums
            let artist = self.get_artist(&artist_uri)?;
            let top_tracks = self
                .get_artist_top_tracks(&artist_uri)?
                .tracks
                .into_iter()
                .map(|t| t.into())
                .collect::<Vec<_>>();
            let related_artists = self
                .get_related_artists(&artist_uri)?
                .artists
                .into_iter()
                .map(|a| a.into())
                .collect::<Vec<_>>();

            state.player.write().unwrap().context_cache.put(
                artist_uri.clone(),
                Context::Artist(artist, top_tracks, vec![], related_artists),
            );

            // delay the request for getting artist's albums to not block the UI
            let albums = self
                .get_artist_albums(&artist_uri)
                .await?
                .into_iter()
                .map(|a| a.into())
                .collect::<Vec<_>>();

            if let Some(Context::Artist(_, _, ref mut old, _)) = state
                .player
                .write()
                .unwrap()
                .context_cache
                .peek_mut(&artist_uri)
            {
                *old = albums;
            }
        }
        Ok(())
    }

    async fn internal_call<T>(&self, url: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(self
            .http
            .get(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!(
                    "Bearer {}",
                    self.spotify.access_token.clone().unwrap_or_else(|| {
                        log::warn!("failed to get spotify client's access token");
                        "".to_string()
                    })
                ),
            )
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
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self.internal_call::<page::Page<T>>(&url).await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// updates the current playback state by fetching data from the spotify client
    fn update_current_playback_state(&self, state: &SharedState) -> Result<()> {
        let playback = self.get_current_playback()?;
        let mut player = state.player.write().unwrap();
        player.playback = playback;
        player.playback_last_updated = Some(std::time::Instant::now());
        Ok(())
    }

    /// cleans up a list of albums (sort by date, remove duplicated entries, etc)
    fn clean_up_artist_albums(
        &self,
        albums: Vec<album::SimplifiedAlbum>,
    ) -> Vec<album::SimplifiedAlbum> {
        let mut albums = albums
            .into_iter()
            .filter(|a| a.release_date.is_some())
            .collect::<Vec<_>>();

        albums.sort_by(|x, y| {
            let date_x = x.release_date.clone().unwrap();
            let date_y = y.release_date.clone().unwrap();
            date_x.partial_cmp(&date_y).unwrap()
        });

        let mut visits = std::collections::HashSet::new();
        albums.into_iter().rfold(vec![], |mut acc, a| {
            if !visits.contains(&a.name) {
                visits.insert(a.name.clone());
                acc.push(a);
            }
            acc
        })
    }
}

/// starts the client's request handler
#[tokio::main]
pub async fn start_client_handler(
    state: SharedState,
    mut client: Client,
    recv: std::sync::mpsc::Receiver<ClientRequest>,
) {
    while let Ok(request) = recv.recv() {
        // use the authentication token stored inside the player state
        // for making API calls to Spotify
        client.spotify.access_token =
            Some(state.player.read().unwrap().token.access_token.to_owned());

        {
            // handle the client request while trying not to block the current thread
            let client = client.clone();
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(err) = client.handle_request(&state, request).await {
                    log::warn!("{:#?}", err);
                }
            });
        }
    }
}

// starts multiple event watchers that
// listen to player events and notify the client
// to make additional update requests if needed
#[tokio::main]
pub async fn start_player_event_watchers(
    state: SharedState,
    send: std::sync::mpsc::Sender<ClientRequest>,
    session: Session,
) {
    // on startup, the UI needs to know the current playback to render the current playing context
    send.send(ClientRequest::GetCurrentPlayback)
        .unwrap_or_else(|err| {
            log::warn!("failed to get the current playback: {}", err);
        });
    // needs to know all available devices on startup to connect to the first available device if none is running
    send.send(ClientRequest::GetDevices).unwrap_or_else(|err| {
        log::warn!("failed to get devices: {}", err);
    });

    // start a playback pooling (every `playback_refresh_duration_in_ms` ms) thread
    // A zero-value `playback_refresh_in_ms` indicates no playback pooling thread
    if state.app_config.playback_refresh_duration_in_ms > 0 {
        std::thread::spawn({
            let send = send.clone();
            let playback_refresh_duration =
                std::time::Duration::from_millis(state.app_config.playback_refresh_duration_in_ms);
            move || -> Result<()> {
                loop {
                    send.send(ClientRequest::GetCurrentPlayback)
                        .unwrap_or_else(|err| {
                            log::warn!("failed to get the current playback: {}", err);
                        });
                    std::thread::sleep(playback_refresh_duration);
                }
            }
        });
    }

    // start the main event watcher,
    // try to watch for new events every second
    let refresh_duration = std::time::Duration::from_millis(1000);
    loop {
        watch_player_events(&state, &send, &session)
            .await
            .unwrap_or_else(|err| {
                log::warn!("failed to watch player events: {}", err);
            });

        std::thread::sleep(refresh_duration);
    }
}

async fn watch_player_events(
    state: &SharedState,
    send: &std::sync::mpsc::Sender<ClientRequest>,
    session: &Session,
) -> Result<()> {
    // update the auth token if expired
    if std::time::Instant::now() > state.player.read().unwrap().token.expires_at {
        log::info!("auth token inside the player state is expired, trying to get a new token...");
        let token = token::get_token(session, &state.app_config.client_id).await?;
        state.player.write().unwrap().token = token;
    }

    let player = state.player.read().unwrap();
    {
        // if cannot find the current playback, try
        // to connect the first avaiable device
        if player.playback.is_none() && !player.devices.is_empty() {
            log::info!(
                "no playback found, try to connect the first available device {}",
                player.devices[0].name
            );
            // only trying to connect, not transfer the current playback
            send.send(ClientRequest::Player(PlayerRequest::TransferPlayback(
                player.devices[0].id.clone(),
                false,
            )))?;
        }
    }

    {
        // update the playback when the current track ends
        let progress_ms = player.get_playback_progress();
        let duration_ms = player.get_current_playing_track().map(|t| t.duration_ms);
        let is_playing = match player.playback {
            Some(ref playback) => playback.is_playing,
            None => false,
        };
        if let (Some(progress_ms), Some(duration_ms)) = (progress_ms, duration_ms) {
            if progress_ms >= duration_ms && is_playing {
                send.send(ClientRequest::GetCurrentPlayback)?;
            }
        }
    }

    // update the player's context based on the UI's page state
    let page = state.ui.lock().unwrap().page.clone();
    match page {
        PageState::Browsing(uri) => {
            if player.context_uri != uri {
                utils::update_context(state, uri);
            }
        }
        PageState::CurrentPlaying => {
            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = player.playback {
                match playback.context {
                    Some(ref context) => {
                        let uri = context.uri.clone();

                        if uri != player.context_uri {
                            utils::update_context(state, uri.clone());
                            if player.context_cache.peek(&uri).is_none() {
                                match context._type {
                                    rspotify::senum::Type::Playlist => send.send(
                                        ClientRequest::GetContext(ContextURI::Playlist(uri)),
                                    )?,
                                    rspotify::senum::Type::Album => send
                                        .send(ClientRequest::GetContext(ContextURI::Album(uri)))?,
                                    rspotify::senum::Type::Artist => send
                                        .send(ClientRequest::GetContext(ContextURI::Artist(uri)))?,
                                    _ => {
                                        send.send(ClientRequest::GetContext(ContextURI::Unknown(
                                            uri,
                                        )))?;
                                        log::info!(
                                            "encountered not supported context type: {:#?}",
                                            context._type
                                        )
                                    }
                                };
                            }
                        }
                    }
                    None => {
                        if !player.context_uri.is_empty() {
                            utils::update_context(state, "".to_string());
                            send.send(ClientRequest::GetContext(ContextURI::Unknown(
                                "".to_string(),
                            )))?;
                            log::info!("current playback does not have a playing context");
                        }
                    }
                }
            };
        }
    }

    Ok(())
}
