use crate::{config, key};
use rspotify::model::*;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use tui::widgets::*;

pub type SharedState = Arc<State>;
pub type UIStateGuard<'a> = MutexGuard<'a, UIState>;

/// Application's state
pub struct State {
    pub app_config: config::AppConfig,
    pub keymap_config: config::KeymapConfig,
    pub theme_config: config::ThemeConfig,

    pub player: RwLock<PlayerState>,
    pub ui: Mutex<UIState>,
}

/// Player state
pub struct PlayerState {
    pub context_cache: lru::LruCache<String, PlayingContext>,

    pub user_playlists: Vec<playlist::SimplifiedPlaylist>,
    pub devices: Vec<device::Device>,
    pub auth_token_expires_at: std::time::SystemTime,
    pub context: PlayingContext,
    pub playback: Option<context::CurrentlyPlaybackContext>,
    pub playback_last_updated: Option<std::time::SystemTime>,
}

/// UI state
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,
    pub popup_state: PopupState,

    pub progress_bar_rect: tui::layout::Rect,

    pub context_tracks_table_ui_state: TableState,
    pub playlists_list_ui_state: ListState,
    pub themes_list_ui_state: ListState,
    pub devices_list_ui_state: ListState,
    pub shortcuts_help_ui_state: bool,
}

/// Playing context (album, playlist, etc) of the current track
#[derive(Clone, Debug)]
pub enum PlayingContext {
    Playlist(playlist::FullPlaylist, Vec<Track>),
    Album(album::FullAlbum, Vec<Track>),
    Artist(artist::FullArtist, Vec<Track>, Vec<Album>),
    Unknown(String),
}

/// Popup state
pub enum PopupState {
    None,
    ContextSearch(ContextSearchState),
    PlaylistSwitch,
    ThemeSwitch(Vec<config::Theme>),
    DeviceSwitch,
    CommandHelp,
}

/// State for searching tracks in a playing context
pub struct ContextSearchState {
    pub query: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug)]
/// Order of sorting tracks in a playing context
pub enum ContextSortOrder {
    AddedAt,
    TrackName,
    Album,
    Artists,
    Duration,
}

#[derive(Default, Debug, Clone)]
/// A simplified version of `rspotify` track
pub struct Track {
    pub id: Option<String>,
    pub uri: String,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    pub duration: u32,
    pub added_at: u64,
}

#[derive(Default, Debug, Clone)]
/// A simplified version of `rspotify` album
pub struct Album {
    pub id: Option<String>,
    pub uri: Option<String>,
    pub name: String,
}

#[derive(Default, Debug, Clone)]
/// A simplified version of `rspotify` artist
pub struct Artist {
    pub id: Option<String>,
    pub uri: Option<String>,
    pub name: String,
}

impl State {
    /// get a list of application themes with the current theme as the first element
    pub fn get_themes(&self, ui: &MutexGuard<UIState>) -> Vec<config::Theme> {
        let mut themes = self.theme_config.themes.clone();
        let id = themes.iter().position(|t| t.name == ui.theme.name);
        if let Some(id) = id {
            let theme = themes.remove(id);
            themes.insert(0, theme);
        }
        themes
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            app_config: config::AppConfig::default(),
            theme_config: config::ThemeConfig::default(),
            keymap_config: config::KeymapConfig::default(),

            player: RwLock::new(PlayerState::default()),

            ui: Mutex::new(UIState::default()),
        }
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            context_cache: lru::LruCache::new(64),
            auth_token_expires_at: std::time::SystemTime::now(),
            devices: vec![],
            user_playlists: vec![],
            context: PlayingContext::Unknown("".to_owned()),
            playback: None,
            playback_last_updated: None,
        }
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: config::Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },
            popup_state: PopupState::None,

            progress_bar_rect: tui::layout::Rect::default(),

            context_tracks_table_ui_state: TableState::default(),
            playlists_list_ui_state: ListState::default(),
            themes_list_ui_state: ListState::default(),
            devices_list_ui_state: ListState::default(),
            shortcuts_help_ui_state: false,
        }
    }
}

impl PlayerState {
    /// sorts tracks in the current playing context given a context sort oder
    pub fn sort_context_tracks(&mut self, sort_oder: ContextSortOrder) {
        let tracks = self.get_context_tracks_mut();
        if let Some(tracks) = tracks {
            tracks.sort_by(|x, y| sort_oder.compare(x, y));
        }
    }

    /// reverses order of tracks in the current playing context
    pub fn reverse_context_tracks(&mut self) {
        let tracks = self.get_context_tracks_mut();
        if let Some(tracks) = tracks {
            tracks.reverse();
        }
    }

    /// gets the current playing track
    pub fn get_current_playing_track(&self) -> Option<&track::FullTrack> {
        match self.playback {
            None => None,
            Some(ref playback) => match playback.item {
                Some(rspotify::model::PlayingItem::Track(ref track)) => Some(track),
                _ => None,
            },
        }
    }

    /// gets the current playback progress
    pub fn get_playback_progress(&self) -> Option<u32> {
        match self.playback {
            None => None,
            Some(ref playback) => match playback.item {
                Some(rspotify::model::PlayingItem::Track(ref track)) => {
                    let progress_ms = (playback.progress_ms.unwrap() as u128)
                        + if playback.is_playing {
                            std::time::SystemTime::now()
                                .duration_since(self.playback_last_updated.unwrap())
                                .unwrap()
                                .as_millis()
                        } else {
                            0
                        };
                    if progress_ms > (track.duration_ms as u128) {
                        Some(track.duration_ms)
                    } else {
                        Some(progress_ms as u32)
                    }
                }
                _ => None,
            },
        }
    }

    /// gets the description of current playing context
    pub fn get_context_description(&self) -> String {
        match self.context {
            PlayingContext::Unknown(_) => {
                "Cannot infer the playing context from the current playback".to_owned()
            }
            PlayingContext::Album(ref album, _) => {
                format!("Album: {}", album.name)
            }
            PlayingContext::Playlist(ref playlist, _) => {
                format!("Playlist: {}", playlist.name)
            }
            PlayingContext::Artist(ref artist, _, _) => {
                format!("Artist: {}", artist.name)
            }
        }
    }

    /// gets all tracks inside the current playing context
    pub fn get_context_tracks(&self) -> Option<&Vec<Track>> {
        match self.context {
            PlayingContext::Unknown(_) => None,
            PlayingContext::Album(_, ref tracks) => Some(tracks),
            PlayingContext::Playlist(_, ref tracks) => Some(tracks),
            PlayingContext::Artist(_, ref tracks, _) => Some(tracks),
        }
    }

    /// gets all tracks inside the current playing context (mutable)
    pub fn get_context_tracks_mut(&mut self) -> Option<&mut Vec<Track>> {
        match self.context {
            PlayingContext::Unknown(_) => None,
            PlayingContext::Album(_, ref mut tracks) => Some(tracks),
            PlayingContext::Playlist(_, ref mut tracks) => Some(tracks),
            PlayingContext::Artist(_, ref mut tracks, _) => Some(tracks),
        }
    }

    /// gets current playing context's uri
    pub fn get_context_uri(&self) -> &str {
        match self.context {
            PlayingContext::Unknown(ref uri) => &uri,
            PlayingContext::Album(ref album, _) => &album.uri,
            PlayingContext::Playlist(ref playlist, _) => &playlist.uri,
            PlayingContext::Artist(ref artist, _, _) => &artist.uri,
        }
    }
}

impl UIState {
    /// searches tracks in the current playing context
    pub fn search_context_tracks(&mut self, player: &RwLockReadGuard<PlayerState>) {
        if let PopupState::ContextSearch(ref mut state) = self.popup_state {
            let mut query = state.query.clone();
            query.remove(0); // remove the '/' character at the beginning of the query string
            log::info!("search tracks in context with query {}", query);
            let tracks = player.get_context_tracks();
            if let Some(tracks) = tracks {
                let id = if tracks.is_empty() { None } else { Some(0) };
                self.context_tracks_table_ui_state.select(id);
                state.tracks = tracks
                    .iter()
                    .filter(|&t| t.get_basic_info().to_lowercase().contains(&query))
                    .cloned()
                    .collect();
            }
        }
    }

    /// gets all tracks inside the current playing context.
    /// If in the context search mode, returns tracks filtered by the search query.
    pub fn get_context_tracks<'a>(
        &'a self,
        player: &'a RwLockReadGuard<'a, PlayerState>,
    ) -> Vec<&'a Track> {
        match self.popup_state {
            PopupState::ContextSearch(ref state) => state.tracks.iter().collect(),
            _ => player
                .get_context_tracks()
                .map(|tracks| tracks.iter().collect::<Vec<_>>())
                .unwrap_or_default(),
        }
    }
}

impl Track {
    /// gets the track's artists information
    pub fn get_artists_info(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// gets the track basic information (track's name, artists' name and album's name)
    pub fn get_basic_info(&self) -> String {
        format!(
            "{} {} {}",
            self.name,
            self.get_artists_info(),
            self.album.name
        )
    }
}

impl From<playlist::PlaylistTrack> for Track {
    fn from(t: playlist::PlaylistTrack) -> Self {
        let track = t.track.unwrap();
        Self {
            id: track.id,
            uri: track.uri,
            name: track.name,
            artists: track
                .artists
                .into_iter()
                .map(|a| Artist {
                    id: a.id,
                    uri: a.uri,
                    name: a.name,
                })
                .collect(),
            album: Album {
                id: track.album.id,
                uri: track.album.uri,
                name: track.album.name,
            },
            duration: track.duration_ms,
            added_at: t.added_at.timestamp() as u64,
        }
    }
}

impl From<track::SimplifiedTrack> for Track {
    fn from(track: track::SimplifiedTrack) -> Self {
        Self {
            id: track.id,
            uri: track.uri,
            name: track.name,
            artists: track
                .artists
                .into_iter()
                .map(|a| Artist {
                    id: a.id,
                    uri: a.uri,
                    name: a.name,
                })
                .collect(),
            album: Album::default(),
            duration: track.duration_ms,
            added_at: 0,
        }
    }
}

impl From<track::FullTrack> for Track {
    fn from(track: track::FullTrack) -> Self {
        Self {
            id: track.id,
            uri: track.uri,
            name: track.name,
            artists: track
                .artists
                .into_iter()
                .map(|a| Artist {
                    id: a.id,
                    uri: a.uri,
                    name: a.name,
                })
                .collect(),
            album: Album {
                name: track.album.name,
                id: track.album.id,
                uri: track.album.uri,
            },
            duration: track.duration_ms,
            added_at: 0,
        }
    }
}

impl ContextSortOrder {
    pub fn compare(&self, x: &Track, y: &Track) -> std::cmp::Ordering {
        match *self {
            Self::AddedAt => x.added_at.cmp(&y.added_at),
            Self::TrackName => x.name.cmp(&y.name),
            Self::Album => x.album.name.cmp(&y.album.name),
            Self::Duration => x.duration.cmp(&y.duration),
            Self::Artists => x.get_artists_info().cmp(&y.get_artists_info()),
        }
    }
}
