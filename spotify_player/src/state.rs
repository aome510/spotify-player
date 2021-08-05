use crate::{config, key};
use rspotify::model::*;
use std::sync::{Arc, Mutex, RwLock};
use tui::widgets::*;
pub type SharedState = Arc<State>;

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
    pub user_playlists: Vec<playlist::SimplifiedPlaylist>,
    pub devices: Vec<device::Device>,
    pub auth_token_expires_at: std::time::SystemTime,
    pub playback: Option<context::CurrentlyPlaybackContext>,
    pub context: PlayingContext,
}

/// UI state
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,
    pub popup_state: PopupState,
    pub context_tracks_table_ui_state: TableState,
    pub playlists_list_ui_state: ListState,
    pub themes_list_ui_state: ListState,
    pub devices_list_ui_state: ListState,
    pub shortcuts_help_ui_state: bool,
}

/// Playing context (album, playlist, etc) of the current track
pub enum PlayingContext {
    Playlist(playlist::FullPlaylist, Vec<Track>),
    Album(album::FullAlbum, Vec<Track>),
    Unknown,
}

/// Popup state
pub enum PopupState {
    None,
    ContextSearch(ContextSearchState),
    PlaylistSwitch,
    ThemeSwitch(config::Theme),
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
            auth_token_expires_at: std::time::SystemTime::now(),
            devices: vec![],
            user_playlists: vec![],
            playback: None,
            context: PlayingContext::Unknown,
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
            context_tracks_table_ui_state: TableState::default(),
            playlists_list_ui_state: ListState::default(),
            themes_list_ui_state: ListState::default(),
            devices_list_ui_state: ListState::default(),
            shortcuts_help_ui_state: false,
        }
    }
}

impl State {
    /// creates new state
    pub fn new() -> SharedState {
        Arc::new(State::default())
    }
}

impl PlayerState {
    /// sorts tracks in the current playing context given a context sort oder
    pub fn sort_context_tracks(&mut self, sort_oder: ContextSortOrder) {
        match self.context {
            PlayingContext::Unknown => {}
            PlayingContext::Album(_, ref mut tracks) => {
                tracks.sort_by(|x, y| sort_oder.compare(x, y));
            }
            PlayingContext::Playlist(_, ref mut tracks) => {
                tracks.sort_by(|x, y| sort_oder.compare(x, y));
            }
        };
    }

    /// reverses order of tracks in the current playing context
    pub fn reverse_context_tracks(&mut self) {
        match self.context {
            PlayingContext::Unknown => {}
            PlayingContext::Album(_, ref mut tracks) => {
                tracks.reverse();
            }
            PlayingContext::Playlist(_, ref mut tracks) => {
                tracks.reverse();
            }
        };
    }

    /// gets the description of current playing context
    pub fn get_context_description(&self) -> String {
        match self.context {
            PlayingContext::Unknown => {
                "Cannot infer the playing context from the current playback".to_owned()
            }
            PlayingContext::Album(ref album, _) => {
                format!("Album: {}", album.name,)
            }
            PlayingContext::Playlist(ref playlist, _) => {
                format!("Playlist: {}", playlist.name)
            }
        }
    }

    /// gets all tracks inside the current playing context
    pub fn get_context_tracks(&self) -> Vec<&Track> {
        match self.context {
            PlayingContext::Unknown => vec![],
            PlayingContext::Album(_, ref tracks) => tracks.iter().collect(),
            PlayingContext::Playlist(_, ref tracks) => tracks.iter().collect(),
        }
    }
}

impl UIState {
    /// searches tracks in the current playing context
    pub fn search_context_tracks(&mut self, tracks: Vec<&Track>) {
        if let PopupState::ContextSearch(ref mut state) = self.popup_state {
            let query = state.query.clone();
            query.remove(0); // remove the '/' character at the beginning of the query string
            log::info!("search tracks in context with query {}", query);
            let id = if tracks.is_empty() { None } else { Some(0) };
            self.context_tracks_table_ui_state.select(id);
            state.tracks = tracks
                .into_iter()
                .filter(|&t| t.get_basic_info().to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
    }

    /// gets all tracks inside the current playing context.
    /// If in the context search mode, returns tracks filtered by the search query.
    pub fn get_context_tracks<'a>(&'a self, tracks: Vec<&'a Track>) -> Vec<&'a Track> {
        match self.popup_state {
            PopupState::ContextSearch(ref state) => state.tracks.iter().collect(),
            _ => tracks,
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
