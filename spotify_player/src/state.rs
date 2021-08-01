use crate::{config, key};
use rspotify::{model::*, senum::*};
use std::sync::{Arc, RwLock};
use tui::widgets::*;
pub type SharedState = Arc<RwLock<State>>;

// TODO: improve variable naming for the application states

/// Application's state
pub struct State {
    pub app_config: config::AppConfig,
    pub keymap_config: config::KeymapConfig,

    pub current_key_prefix: key::KeySequence,

    pub is_running: bool,
    pub auth_token_expires_at: std::time::SystemTime,

    pub devices: Vec<device::Device>,

    pub current_playback_context: Option<context::CurrentlyPlaybackContext>,
    pub current_playlist: Option<playlist::FullPlaylist>,
    pub current_album: Option<album::FullAlbum>,
    pub current_playlists: Vec<playlist::SimplifiedPlaylist>,
    pub current_context_tracks: Vec<Track>,

    pub context_search_state: ContextSearchState,

    pub popup_state: PopupState,
    pub context_tracks_table_ui_state: TableState,
    pub playlists_list_ui_state: ListState,
    pub shortcuts_help_ui_state: bool,
}

#[derive(Default)]
/// Context search state
pub struct ContextSearchState {
    pub query: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug)]
/// Context sort order
pub enum ContextSortOrder {
    AddedAt,
    TrackName,
    Album,
    Artists,
    Duration,
}

#[derive(Clone)]
/// Popup state
pub enum PopupState {
    None,
    ContextSearch,
    PlaylistSwitch,
    CommandHelp,
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
            keymap_config: config::KeymapConfig::default(),

            is_running: true,
            auth_token_expires_at: std::time::SystemTime::now(),
            devices: vec![],

            current_playlist: None,
            current_album: None,
            current_context_tracks: vec![],
            current_playlists: vec![],
            current_playback_context: None,

            current_key_prefix: key::KeySequence { keys: vec![] },

            popup_state: PopupState::None,
            context_search_state: ContextSearchState::default(),

            context_tracks_table_ui_state: TableState::default(),
            playlists_list_ui_state: ListState::default(),
            shortcuts_help_ui_state: false,
        }
    }
}

impl State {
    /// creates new state
    pub fn new() -> SharedState {
        Arc::new(RwLock::new(State::default()))
    }

    /// sorts tracks in the current playing context given a context sort oder
    pub fn sort_context_tracks(&mut self, sort_oder: ContextSortOrder) {
        self.current_context_tracks
            .sort_by(|x, y| sort_oder.compare(x, y));
    }

    /// gets the type (Album, Artist, Playlist, etc) of current playing context
    pub fn get_context_type(&self) -> Option<Type> {
        match self.current_playback_context {
            None => None,
            Some(ref playback_context) => playback_context
                .context
                .as_ref()
                .map(|context| context._type),
        }
    }

    /// gets the description of current playing context
    pub fn get_context_description(&self) -> String {
        match self.get_context_type() {
            None => "Cannot infer the playing context from the current playback".to_owned(),
            Some(ty) => match ty {
                rspotify::senum::Type::Album => {
                    format!(
                        "Album: {}",
                        match self.current_album {
                            None => "loading...",
                            Some(ref album) => &album.name,
                        }
                    )
                }
                rspotify::senum::Type::Playlist => {
                    format!(
                        "Playlist: {}",
                        match self.current_playlist {
                            None => "loading...",
                            Some(ref playlist) => &playlist.name,
                        }
                    )
                }
                _ => "Unknown context type".to_owned(),
            },
        }
    }

    /// returns the list of tracks in the current playback context (album, playlist, etc)
    /// filtered by a search query
    pub fn get_context_filtered_tracks(&self) -> Vec<&Track> {
        if self.context_search_state.query.is_some() {
            // in search mode, return the filtered tracks
            self.context_search_state.tracks.iter().collect()
        } else {
            self.current_context_tracks.iter().collect()
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
            .join(",")
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
