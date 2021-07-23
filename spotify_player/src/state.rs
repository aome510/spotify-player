use crate::prelude::*;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub type SharedState = Arc<RwLock<State>>;

pub struct State {
    pub is_running: bool,
    pub auth_token_expires_at: std::time::SystemTime,
    pub current_playback_context: Option<context::CurrentlyPlaybackContext>,
    pub current_playlist: Option<playlist::FullPlaylist>,
    pub current_context_tracks: Vec<Track>,

    // event states
    pub current_event_state: EventState,
    pub context_search_state: ContextSearchState,

    // UI states
    pub ui_context_tracks_table_state: TableState,
}

#[derive(Default)]
pub struct ContextSearchState {
    pub query: Option<String>,
    pub tracks: Vec<Track>,
}

#[derive(Debug)]
pub enum ContextSortOrder {
    AddedAt(bool),
    TrackName(bool),
    Album(bool),
    Artists(bool),
    Duration(bool),
}

#[derive(Clone)]
pub enum EventState {
    Default,
    Sort,
    ContextSearch,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub uri: String,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    pub duration: u32,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub uri: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Artist {
    pub uri: Option<String>,
    pub name: String,
}

impl Default for State {
    fn default() -> Self {
        State {
            is_running: true,
            auth_token_expires_at: std::time::SystemTime::now(),
            current_playlist: None,
            current_context_tracks: vec![],
            current_playback_context: None,

            current_event_state: EventState::Default,
            context_search_state: ContextSearchState::default(),

            ui_context_tracks_table_state: TableState::default(),
        }
    }
}

impl State {
    pub fn new() -> SharedState {
        Arc::new(RwLock::new(State::default()))
    }

    pub fn sort_context_tracks(&mut self, sort_oder: ContextSortOrder) {
        self.current_context_tracks
            .sort_by(|x, y| sort_oder.compare(x, y));
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
    pub fn get_artists_info(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(",")
    }

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
            uri: track.uri,
            name: track.name,
            artists: track
                .artists
                .into_iter()
                .map(|a| Artist {
                    uri: a.uri,
                    name: a.name,
                })
                .collect(),
            album: Album {
                uri: track.album.uri,
                name: track.album.name,
            },
            duration: track.duration_ms,
            added_at: 0,
        }
    }
}

impl ContextSortOrder {
    pub fn compare(&self, x: &Track, y: &Track) -> std::cmp::Ordering {
        match *self {
            Self::AddedAt(asc) => {
                if asc {
                    x.added_at.cmp(&y.added_at)
                } else {
                    y.added_at.cmp(&x.added_at)
                }
            }
            Self::TrackName(asc) => {
                if asc {
                    x.name.cmp(&y.name)
                } else {
                    y.name.cmp(&x.name)
                }
            }
            Self::Album(asc) => {
                if asc {
                    x.album.name.cmp(&y.album.name)
                } else {
                    y.album.name.cmp(&x.album.name)
                }
            }
            Self::Duration(asc) => {
                if asc {
                    x.duration.cmp(&y.duration)
                } else {
                    y.duration.cmp(&x.duration)
                }
            }
            Self::Artists(asc) => {
                if asc {
                    x.get_artists_info().cmp(&y.get_artists_info())
                } else {
                    y.get_artists_info().cmp(&x.get_artists_info())
                }
            }
        }
    }
}

/// truncates a string whose length exceeds a given `max_len` length.
/// Such string will be appended with `...` at the end.
pub fn truncate_string(s: String, max_len: usize) -> String {
    let len = UnicodeWidthStr::width(s.as_str());
    if len > max_len {
        // get the longest prefix of the string such that its unicode width
        // is still within the `max_len` limit
        let mut s: String = s
            .chars()
            .fold(("".to_owned(), 0_usize), |(mut cs, cw), c| {
                let w = UnicodeWidthChar::width(c).unwrap_or(2);
                if cw + w + 3 > max_len {
                    (cs, cw)
                } else {
                    cs.push(c);
                    (cs, cw + w)
                }
            })
            .0;
        s.push_str("...");
        s
    } else {
        s
    }
}
