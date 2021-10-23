use std::sync::Arc;

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::*,
    utils,
};
use anyhow::{anyhow, Result};
use librespot_core::session::Session;
use rspotify::{model, prelude::*};

mod handlers;
mod spotify;

pub use handlers::*;

/// The application's client
#[derive(Clone)]
pub struct Client {
    spotify: Arc<spotify::Spotify>,
    http: reqwest::Client,
}

impl Client {
    /// creates a new client
    pub fn new(session: Session, client_id: String) -> Self {
        Self {
            spotify: Arc::new(spotify::Spotify::new(session, client_id)),
            http: reqwest::Client::new(),
        }
    }

    /// initializes the authorization token stored inside the Spotify client
    pub async fn init_token(&self) -> Result<()> {
        self.spotify.refresh_token().await?;
        log::info!(
            "auth token: {:#?}",
            self.spotify.get_token().lock().await.unwrap()
        );
        Ok(())
    }

    /// handles a player request
    async fn handle_player_request(
        &self,
        state: &SharedState,
        request: PlayerRequest,
    ) -> Result<()> {
        log::info!("handle player request: {:?}", request);

        // `TransferPlayback` needs to be handled separately
        // because it doesn't require an active playback
        if let PlayerRequest::TransferPlayback(device_id, force_play) = request {
            return Ok(self
                .spotify
                .transfer_playback(&device_id, Some(force_play))
                .await?);
        }

        let playback = match state.player.read().unwrap().simplified_playback() {
            Some(playback) => playback,
            None => {
                return Err(anyhow!(
                    "failed to handle the player request: there is no active playback"
                ));
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
            }
            PlayerRequest::SeekTrack(position_ms) => {
                self.spotify.seek_track(position_ms, device_id).await?
            }
            PlayerRequest::Repeat => {
                let next_repeat_state = match playback.repeat_state {
                    model::RepeatState::Off => model::RepeatState::Track,
                    model::RepeatState::Track => model::RepeatState::Context,
                    model::RepeatState::Context => model::RepeatState::Off,
                };

                self.spotify.repeat(&next_repeat_state, device_id).await?
            }
            PlayerRequest::Shuffle => {
                self.spotify
                    .shuffle(!playback.shuffle_state, device_id)
                    .await?
            }
            PlayerRequest::Volume(volume) => self.spotify.volume(volume, device_id).await?,
            PlayerRequest::StartPlayback(p) => {
                self.start_playback(p, device_id).await?;
                // for some reasons, when starting a new playback, the integrated `spotify-player`
                // client doesn't respect the initial shuffle state, so we need to manually update the state
                self.spotify
                    .shuffle(playback.shuffle_state, device_id)
                    .await?
            }
            PlayerRequest::TransferPlayback(..) => unreachable!(),
        };

        Ok(())
    }

    /// handles a client request
    pub async fn handle_request(&self, state: &SharedState, request: ClientRequest) -> Result<()> {
        log::info!("handle client request {:?}", request);

        match request {
            ClientRequest::GetCurrentUser => {
                let user = self.spotify.current_user().await?;
                state.player.write().unwrap().user = Some(user);
            }
            ClientRequest::GetRecommendations(seed) => {
                let tracks = self.recommendations(&seed).await?;

                // update the recommendation page state if needed
                if let PageState::Recommendations(ref state_seed, ref mut state_tracks) =
                    state.ui.lock().unwrap().current_page_mut()
                {
                    if state_seed.uri() == seed.uri() {
                        *state_tracks = Some(tracks);
                    }
                }
            }
            ClientRequest::Player(event) => {
                self.handle_player_request(state, event).await?;

                // After handling a request that modifies the player's playback,
                // update the playback state by making `n_refreshes` refresh requests.
                //
                // - Why needs more than one request to update the playback?
                // Spotify API may take a while to update the new change,
                // so making additional requests can help ensure that
                // the playback state is in sync with the latest change.
                let n_refreshes = state.app_config.n_refreshes_each_playback_update;
                let delay_duration = std::time::Duration::from_millis(
                    state.app_config.refresh_delay_in_ms_each_playback_update,
                );

                for _ in 0..n_refreshes {
                    std::thread::sleep(delay_duration);
                    self.update_current_playback_state(state).await?;
                }
            }
            ClientRequest::GetCurrentPlayback => {
                self.update_current_playback_state(state).await?;
            }
            ClientRequest::GetDevices => {
                let devices = self.spotify.device().await?;
                state.player.write().unwrap().devices = devices
                    .into_iter()
                    .map(Device::try_from_device)
                    .flatten()
                    .collect();
            }
            ClientRequest::GetUserPlaylists => {
                let playlists = self.current_user_playlists().await?;
                state.player.write().unwrap().user_playlists = playlists;
            }
            ClientRequest::GetUserFollowedArtists => {
                let artists = self.current_user_followed_artists().await?;
                state.player.write().unwrap().user_followed_artists = artists;
            }
            ClientRequest::GetUserSavedAlbums => {
                let albums = self.current_user_saved_albums().await?;
                state.player.write().unwrap().user_saved_albums = albums;
            }
            ClientRequest::GetContext(context) => {
                match context {
                    ContextId::Playlist(playlist_id) => {
                        self.playlist_context(&playlist_id, state).await?;
                    }
                    ContextId::Album(album_id) => {
                        self.album_context(&album_id, state).await?;
                    }
                    ContextId::Artist(artist_id) => {
                        self.artist_context(&artist_id, state).await?;
                    }
                };
            }
            ClientRequest::Search(query) => {
                self.search(state, query).await?;
            }
            ClientRequest::AddTrackToPlaylist(playlist_id, track_id) => {
                self.add_track_to_playlist(&playlist_id, &track_id).await?;
            }
            ClientRequest::SaveToLibrary(item) => {
                self.save_to_library(state, item).await?;
            }
        };

        Ok(())
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
                .internal_call::<model::CursorPageFullArtists>(&url)
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
    pub async fn artist_albums(&self, artist_id: &ArtistId) -> Result<Vec<Album>> {
        let mut singles = {
            let first_page = self
                .spotify
                .artist_albums_manual(
                    artist_id,
                    Some(&model::AlbumType::Single),
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
                    artist_id,
                    Some(&model::AlbumType::Album),
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
            .map(Album::try_from_simplified_album)
            .flatten()
            .collect();
        Ok(self.clean_up_artist_albums(albums))
    }

    /// starts a playback
    pub async fn start_playback(&self, playback: Playback, device_id: Option<&str>) -> Result<()> {
        match playback {
            Playback::Context(context_id, offset) => match context_id {
                ContextId::Album(id) => {
                    self.spotify
                        .start_context_playback(&id, device_id, offset, None)
                        .await?
                }
                ContextId::Artist(id) => {
                    self.spotify
                        .start_context_playback(&id, device_id, offset, None)
                        .await?
                }
                ContextId::Playlist(id) => {
                    self.spotify
                        .start_context_playback(&id, device_id, offset, None)
                        .await?
                }
            },
            Playback::URIs(track_ids, offset) => {
                self.spotify
                    .start_uris_playback(
                        track_ids
                            .iter()
                            .map(|id| id as &dyn model::PlayableId)
                            .collect::<Vec<_>>(),
                        device_id,
                        offset,
                        None,
                    )
                    .await?
            }
        }

        Ok(())
    }

    /// gets recommendation tracks from a recommendation seed
    pub async fn recommendations(&self, seed: &SeedItem) -> Result<Vec<Track>> {
        let attributes = vec![model::RecommendationsAttribute::MinPopularity(50)];

        let tracks = match seed {
            SeedItem::Artist(artist) => {
                self.spotify
                    .recommendations(
                        attributes,
                        Some(vec![&artist.id]),
                        None::<Vec<_>>,
                        None::<Vec<_>>,
                        None,
                        Some(50),
                    )
                    .await?
                    .tracks
            }
            SeedItem::Track(track) => {
                self.spotify
                    .recommendations(
                        attributes,
                        Some(track.artists.iter().map(|a| &a.id).collect::<Vec<_>>()),
                        None::<Vec<_>>,
                        Some(vec![&track.id]),
                        None,
                        Some(50),
                    )
                    .await?
                    .tracks
            }
        };

        Ok(tracks
            .into_iter()
            .map(Track::try_from_simplified_track)
            .flatten()
            .collect())
    }

    /// searchs for items (tracks, artists, albums, playlists) that match a given query string.
    pub async fn search(&self, state: &SharedState, query: String) -> Result<()> {
        let update_ui_states = |results: SearchResults| {
            let mut ui = state.ui.lock().unwrap();
            if let PageState::Searching(_, ref mut state_results) = ui.current_page_mut() {
                *state_results = Box::new(results);
                ui.window = WindowState::Search(
                    utils::new_list_state(),
                    utils::new_list_state(),
                    utils::new_list_state(),
                    utils::new_list_state(),
                    SearchFocusState::Input,
                );
            }
        };

        // already search the query before, updating the ui page state directly
        if let Some(search_results) = state.player.read().unwrap().search_cache.peek(&query) {
            update_ui_states(search_results.clone());
            return Ok(());
        }

        let (track_result, artist_result, album_result, playlist_result) = tokio::try_join!(
            self.search_specific_type(&query, &model::SearchType::Track),
            self.search_specific_type(&query, &model::SearchType::Artist),
            self.search_specific_type(&query, &model::SearchType::Album),
            self.search_specific_type(&query, &model::SearchType::Playlist)
        )?;

        let (tracks, artists, albums, playlists) = (
            match track_result {
                model::SearchResult::Tracks(p) => p.items.into_iter().map(|i| i.into()).collect(),
                _ => unreachable!(),
            },
            match artist_result {
                model::SearchResult::Artists(p) => p.items.into_iter().map(|i| i.into()).collect(),
                _ => unreachable!(),
            },
            match album_result {
                model::SearchResult::Albums(p) => p
                    .items
                    .into_iter()
                    .map(Album::try_from_simplified_album)
                    .flatten()
                    .collect(),
                _ => unreachable!(),
            },
            match playlist_result {
                model::SearchResult::Playlists(p) => {
                    p.items.into_iter().map(|i| i.into()).collect()
                }
                _ => unreachable!(),
            },
        );

        let search_results = SearchResults {
            tracks,
            artists,
            albums,
            playlists,
        };

        // update the search cache stored inside the player state
        state
            .player
            .write()
            .unwrap()
            .search_cache
            .put(query, search_results.clone());

        update_ui_states(search_results);
        Ok(())
    }

    async fn search_specific_type(
        &self,
        query: &str,
        _type: &model::SearchType,
    ) -> Result<model::SearchResult> {
        Ok(self
            .spotify
            .search(query, _type, None, None, None, None)
            .await?)
    }

    /// adds track to a playlist
    pub async fn add_track_to_playlist(
        &self,
        playlist_id: &PlaylistId,
        track_id: &TrackId,
    ) -> Result<()> {
        let dyn_track_id = track_id as &dyn PlayableId;

        // remove all the occurrences of the track to ensure no duplication in the playlist
        self.spotify
            .playlist_remove_all_occurrences_of_items(playlist_id, vec![dyn_track_id], None)
            .await?;

        self.spotify
            .playlist_add_items(playlist_id, vec![dyn_track_id], None)
            .await?;

        Ok(())
    }

    /// saves a Spotify item to current user's library.
    /// Before adding new item, the function checks if that item already exists in the library
    /// to avoid adding a duplicated item.
    pub async fn save_to_library(&self, state: &SharedState, item: Item) -> Result<()> {
        match item {
            Item::Track(track) => {
                let contains = self
                    .spotify
                    .current_user_saved_tracks_contains(vec![&track.id])
                    .await?;
                if !contains[0] {
                    self.spotify
                        .current_user_saved_tracks_add(vec![&track.id])
                        .await?;
                }
            }
            Item::Album(album) => {
                let contains = self
                    .spotify
                    .current_user_saved_albums_contains(vec![&album.id])
                    .await?;
                if !contains[0] {
                    self.spotify
                        .current_user_saved_albums_add(vec![&album.id])
                        .await?;
                }
            }
            Item::Artist(artist) => {
                let follows = self
                    .spotify
                    .user_artist_check_follow(vec![&artist.id])
                    .await?;
                if !follows[0] {
                    self.spotify.user_follow_artists(vec![&artist.id]).await?;
                }
            }
            Item::Playlist(playlist) => {
                let user_id = state
                    .player
                    .read()
                    .unwrap()
                    .user
                    .as_ref()
                    .map(|u| u.id.clone());

                if let Some(user_id) = user_id {
                    let follows = self
                        .spotify
                        .playlist_check_follow(&playlist.id, &[&user_id])
                        .await?;
                    if !follows[0] {
                        self.spotify.playlist_follow(&playlist.id, None).await?;
                    }
                }
            }
        }
        Ok(())
    }

    /// gets a playlist context data
    async fn playlist_context(&self, playlist_id: &PlaylistId, state: &SharedState) -> Result<()> {
        let playlist_uri = playlist_id.uri();
        log::info!("get playlist context: {}", playlist_uri);

        // a helper closure that converts a vector of `rspotify_model::PlaylistItem`
        // into a vector of `state::Track`.
        let playlist_items_into_tracks = |items: Vec<model::PlaylistItem>| -> Vec<Track> {
            items
                .into_iter()
                .map(|item| match item.track {
                    Some(model::PlayableItem::Track(track)) => Some(track.into()),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<_>>()
        };

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&playlist_uri)
        {
            // get the playlist
            let playlist = self.spotify.playlist(playlist_id, None, None).await?;
            let first_page = playlist.tracks.clone();
            // get the playlist's tracks
            state.player.write().unwrap().context_cache.put(
                playlist_uri.clone(),
                Context::Playlist(
                    playlist.into(),
                    playlist_items_into_tracks(first_page.items.clone()),
                ),
            );

            let tracks = playlist_items_into_tracks(self.all_paging_items(first_page).await?);

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
    async fn album_context(&self, album_id: &AlbumId, state: &SharedState) -> Result<()> {
        let album_uri = album_id.uri();
        log::info!("get album context: {}", album_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&album_uri)
        {
            // get the album
            let album = self.spotify.album(album_id).await?;
            // get the album's tracks
            let album_tracks = self.all_paging_items(album.tracks.clone()).await?;

            // convert the `rspotify::FullAlbum` into `state::Album`
            let album: Album = album.into();

            let tracks = album_tracks
                .into_iter()
                .map(|t| {
                    Track::try_from_simplified_track(t).map(|mut t| {
                        t.album = Some(album.clone());
                        t
                    })
                })
                .flatten()
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
    async fn artist_context(&self, artist_id: &ArtistId, state: &SharedState) -> Result<()> {
        let artist_uri = artist_id.uri();
        log::info!("get artist context: {}", artist_uri);

        if !state
            .player
            .read()
            .unwrap()
            .context_cache
            .contains(&artist_uri)
        {
            // get a information, top tracks and all albums
            let artist = self.spotify.artist(artist_id).await?.into();

            let top_tracks = self
                .spotify
                .artist_top_tracks(artist_id, &model::enums::misc::Market::FromToken)
                .await?
                .into_iter()
                .map(|t| t.into())
                .collect::<Vec<_>>();

            let related_artists = self
                .spotify
                .artist_related_artists(artist_id)
                .await?
                .into_iter()
                .map(|a| a.into())
                .collect::<Vec<_>>();

            state.player.write().unwrap().context_cache.put(
                artist_uri.clone(),
                Context::Artist(artist, top_tracks, vec![], related_artists),
            );

            // delay the request for getting artist's albums to not block the UI
            let albums = self.artist_albums(artist_id).await?;

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

    /// calls a GET api to Spotify server by making a http request
    /// and parses the JSON response into a specific return type `T`.
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
                format!("Bearer {}", access_token),
            )
            .send()
            .await?
            .json::<T>()
            .await?)
    }

    /// gets all paging items starting from a pagination object of the first page
    async fn all_paging_items<T>(&self, first_page: model::Page<T>) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut items = first_page.items;
        let mut maybe_next = first_page.next;
        while let Some(url) = maybe_next {
            let mut next_page = self.internal_call::<model::Page<T>>(&url).await?;
            items.append(&mut next_page.items);
            maybe_next = next_page.next;
        }
        Ok(items)
    }

    /// updates the current playback state
    async fn update_current_playback_state(&self, state: &SharedState) -> Result<()> {
        let playback = self.spotify.current_playback(None, None::<Vec<_>>).await?;
        let mut player = state.player.write().unwrap();
        player.playback = playback;
        player.playback_last_updated = Some(std::time::Instant::now());
        Ok(())
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
