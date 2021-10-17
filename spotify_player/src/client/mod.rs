use std::sync::Arc;

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::*,
};
use anyhow::{anyhow, Result};
use librespot::core::session::Session;
use rspotify::{model, prelude::*};

mod handlers;
mod spotify;

pub use handlers::*;

/// A spotify client
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
        Ok(self.spotify.refresh_token().await?)
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

        let player = state.player.read().unwrap();
        let playback = match player.playback {
            Some(ref playback) => playback,
            None => {
                return Err(anyhow!("failed to get the current playback context"));
            }
        };
        let device_id = playback.device.id.as_deref();

        Ok(match request {
            PlayerRequest::NextTrack => self.spotify.next_track(device_id).await?,
            PlayerRequest::PreviousTrack => self.spotify.previous_track(device_id).await?,
            PlayerRequest::ResumePause => {
                if playback.is_playing {
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

            PlayerRequest::PlayTrack(context_uri, track_uris, offset) => {
                self.start_playback(playback, context_uri, track_uris, offset)
                    .await?
            }
            PlayerRequest::TransferPlayback(..) => unreachable!(),
        })
    }

    /// handles a client request
    pub async fn handle_request(&self, state: &SharedState, request: ClientRequest) -> Result<()> {
        log::info!("handle the client request {:?}", request);

        match request {
            ClientRequest::GetCurrentUser => {
                let user = self.spotify.current_user().await?;
                state.player.write().unwrap().user = Some(user);
            }
            ClientRequest::Player(event) => {
                self.handle_player_request(state, event).await?;

                // After handling a request that modifies the player's playback,
                // update the playback state by making `n_refreshes` refresh requests.
                //
                // - Why needs more than one request to update the playback?
                // Spotify API may take a while to update with the new changes,
                // so we need to make additional requests to ensure that
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
                    .filter(Option::is_some)
                    .map(Option::unwrap)
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
                    ContextId::Unknown(uri) => {
                        state
                            .player
                            .write()
                            .unwrap()
                            .context_cache
                            .put(uri.clone(), Context::Unknown(uri));
                    }
                };
            }
            ClientRequest::Search(query) => {
                self.search(state, query)?;
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
            .current_user_playlists_manual(None, None)
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
            .current_user_saved_albums_manual(None, None, None)
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
            .map(|a| Album::try_from_simplified_album(a))
            .filter(Option::is_some)
            .map(Option::unwrap)
            .collect();
        Ok(self.clean_up_artist_albums(albums))
    }

    /// plays a track given a context URI
    pub async fn start_playback(
        &self,
        playback: &model::CurrentPlaybackContext,
        context_uri: Option<String>,
        uris: Option<Vec<String>>,
        offset: Option<model::Offset>,
    ) -> Result<()> {
        // self.spotify.start_playback(
        //     Some(playback.device.id.clone()),
        //     context_uri,
        //     uris,
        //     offset,
        //     None,
        // )?;
        // // NOTE: for some reasons, `librespot` device does not keep the shuffle state
        // // after starting a playback. A work around for this is to make an additional
        // // shuffle request to keep the playback's original shuffle state.

        //     self.spotify
        //         .shuffle(playback.shuffle_state, Some(playback.device.id.clone())),
        //         Ok(())

        unimplemented!()
    }

    /// searchs for items (tracks, artists, albums, playlists) that match a given query string.
    pub fn search(&self, state: &SharedState, query: String) -> Result<()> {
        // // searching for tracks, artists, albums, and playlists that match
        // // a given query string. Each class of items will be handled separately
        // // in a separate thread.
        // let update_ui_states = |results: SearchResults| {
        //     let mut ui = state.ui.lock().unwrap();
        //     if let PageState::Searching(_, ref mut state_results) = ui.current_page_mut() {
        //         *state_results = Box::new(results);
        //         ui.window = WindowState::Search(
        //             utils::new_list_state(),
        //             utils::new_list_state(),
        //             utils::new_list_state(),
        //             utils::new_list_state(),
        //             SearchFocusState::Input,
        //         );
        //     }
        // };

        // // already search the query before, updating the ui page state directly
        // if let Some(search_results) = state.player.read().unwrap().search_cache.peek(&query) {
        //     update_ui_states(search_results.clone());
        //     return Ok(());
        // }

        // let tracks_thread = thread::spawn({
        //     let spotify = self.spotify.clone();
        //     let query = query.clone();
        //     move || -> Result<_> {
        //         let search_result = spotify.search(
        //             &query,
        //             &model::SearchType::Track,
        //             None,
        //             None,
        //             None,
        //             None,
        //         ).await?;

        //         Ok(match search_result {
        //             model::SearchResult::Tracks(page) => page,
        //             _ => unreachable!(),
        //         })
        //     }
        // });

        // let artists_thread = thread::spawn({
        //     let spotify = self.spotify.clone();
        //     let query = query.clone();
        //     move || -> Result<_> {
        //         let search_result = spotify.search(
        //             &query,
        //             &model::SearchType::Artist,
        //             None,
        //             None,
        //             None,
        //             None,
        //         )?;

        //         Ok(match search_result {
        //             model::SearchResult::Artists(page) => page,
        //             _ => unreachable!(),
        //         })
        //     }
        // });

        // let albums_thread = thread::spawn({
        //     let spotify = self.spotify.clone();
        //     let query = query.clone();
        //     move || -> Result<_> {
        //        Self::handle_rspotify_result(spotify.search(
        //             &query,
        //             &model::SearchType::Album,
        //             None,
        //             None,
        //             None,
        //             None,
        //         )?;

        //         Ok(match search_result {
        //             model::SearchResult::Albums(page) => page,
        //             _ => unreachable!(),
        //         })
        //     }
        // });

        // let playlists_thread = thread::spawn({
        //     let spotify = self.spotify.clone();
        //     let query = query.clone();
        //     move || -> Result<_> {
        //        Self::handle_rspotify_result(spotify.search(
        //             &query,
        //             &model::SearchType::Playlist,
        //             None,
        //             None,
        //             None,
        //             None,
        //         )?;

        //         Ok(match search_result {
        //             model::SearchResult::Playlists(page) => page,
        //             _ => unreachable!(),
        //         })
        //     }
        // });

        // let tracks = tracks_thread.join().unwrap()?;
        // let artists = artists_thread.join().unwrap()?;
        // let albums = albums_thread.join().unwrap()?;
        // let playlists = playlists_thread.join().unwrap()?;

        // let search_results = SearchResults {
        //     tracks: Self::into_page(tracks),
        //     artists: Self::into_page(artists),
        //     albums: Self::into_page(albums),
        //     playlists: Self::into_page(playlists),
        // };

        // // update the search cache stored inside the player state
        // state
        //     .player
        //     .write()
        //     .unwrap()
        //     .search_cache
        //     .put(query, search_results.clone());

        // update_ui_states(search_results);
        // Ok(())
        unimplemented!()
    }

    /// adds track to a user's playlist
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

    /// saves a Spotify item to current user's library
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
                if let Some(ref user) = state.player.read().unwrap().user {
                    let follows = self
                        .spotify
                        .playlist_check_follow(&playlist.id, &[&user.id])
                        .await?;
                    if !follows[0] {
                        self.spotify.playlist_follow(&playlist.id, None).await?;
                    }
                }
            }
        }
        Ok(())
    }

    /// converts a page of items with type `Y` into a page of items with type `X`
    /// given that type `Y` can be converted to type `X` through the `Into<X>` trait
    fn into_page<X, Y: Into<X>>(page_y: model::Page<Y>) -> model::Page<X> {
        model::Page {
            items: page_y
                .items
                .into_iter()
                .map(|y| y.into())
                .collect::<Vec<_>>(),
            href: page_y.href,
            limit: page_y.limit,
            next: page_y.next,
            offset: page_y.offset,
            previous: page_y.previous,
            total: page_y.total,
        }
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
                .filter(Option::is_some)
                .map(Option::unwrap)
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
                .filter(Option::is_some)
                .map(Option::unwrap)
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
        unimplemented!()
        // let artist_uri = artist_id.uri();
        // log::info!("get artist context: {}", artist_uri);

        // if !state
        //     .player
        //     .read()
        //     .unwrap()
        //     .context_cache
        //     .contains(&artist_uri)
        // {
        //     // get a information, top tracks and all albums
        //     let artist = self.spotify.artist(artist_id).await?;
        //     let top_tracks = self
        //         .spotify
        //         .artist_top_tracks(artist_id)
        //         .await?
        //         .tracks
        //         .into_iter()
        //         .map(|t| t.into())
        //         .collect::<Vec<_>>();
        //     let related_artists = self
        //         .spotify
        //         .related_artists(&artist_uri)?
        //         .artists
        //         .into_iter()
        //         .map(|a| a.into())
        //         .collect::<Vec<_>>();

        //     state.player.write().unwrap().context_cache.put(
        //         artist_uri.clone(),
        //         Context::Artist(artist, top_tracks, vec![], related_artists),
        //     );

        //     // delay the request for getting artist's albums to not block the UI
        //     let albums = self
        //         .artist_albums(&artist_uri)
        //         .await?
        //         .into_iter()
        //         .map(|a| a.into())
        //         .collect::<Vec<_>>();

        //     if let Some(Context::Artist(_, _, ref mut old, _)) = state
        //         .player
        //         .write()
        //         .unwrap()
        //         .context_cache
        //         .peek_mut(&artist_uri)
        //     {
        //         *old = albums;
        //     }
        // }
        // Ok(())
    }

    async fn internal_call<T>(&self, url: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        unimplemented!()
        // Ok(self
        //     .http
        //     .get(url)
        //     .header(
        //         reqwest::header::AUTHORIZATION,
        //         format!(
        //             "Bearer {}",
        //             self.spotify.token().clone().unwrap_or_else(|| {
        //                 log::warn!("failed to get spotify client's access token");
        //                 "".to_string()
        //             })
        //         ),
        //     )
        //     .send()
        //     .await?
        //     .json::<T>()
        //     .await?)
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

    /// updates the current playback state by fetching data from the spotify client
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
