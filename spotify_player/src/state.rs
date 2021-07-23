use crate::prelude::*;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Default)]
pub struct ContextSearchState {
    pub query: Option<String>,
    pub tracks: Vec<track::FullTrack>,
}

#[derive(Debug)]
pub enum PlaylistSortOrder {
    DateAdded(bool),
    TrackName(bool),
    Album(bool),
}

#[derive(Clone)]
pub enum EventState {
    Default,
    Sort,
    ContextSearch,
}

impl PlaylistSortOrder {
    pub fn compare(
        &self,
        x: &playlist::PlaylistTrack,
        y: &playlist::PlaylistTrack,
    ) -> std::cmp::Ordering {
        let x_track = x.track.as_ref().unwrap();
        let y_track = y.track.as_ref().unwrap();
        match *self {
            Self::DateAdded(asc) => {
                if asc {
                    x.added_at.timestamp().cmp(&y.added_at.timestamp())
                } else {
                    y.added_at.timestamp().cmp(&x.added_at.timestamp())
                }
            }
            Self::TrackName(asc) => {
                if asc {
                    x_track.name.cmp(&y_track.name)
                } else {
                    y_track.name.cmp(&x_track.name)
                }
            }
            Self::Album(asc) => {
                if asc {
                    x_track.album.name.cmp(&y_track.album.name)
                } else {
                    y_track.album.name.cmp(&x_track.album.name)
                }
            }
        }
    }
}

pub struct State {
    pub is_running: bool,
    pub auth_token_expires_at: std::time::SystemTime,
    pub current_playlist: Option<playlist::FullPlaylist>,
    pub current_playlist_tracks: Vec<playlist::PlaylistTrack>,
    pub current_playback_context: Option<context::CurrentlyPlaybackContext>,

    // event states
    pub current_event_state: EventState,
    pub context_search_state: ContextSearchState,

    // UI states
    pub ui_context_tracks_table_state: TableState,
}

pub type SharedState = Arc<RwLock<State>>;

impl Default for State {
    fn default() -> Self {
        State {
            is_running: true,
            auth_token_expires_at: std::time::SystemTime::now(),
            current_playlist: None,
            current_playlist_tracks: vec![],
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

    /// sorts the
    pub fn sort_playlist_tracks(&mut self, sort_oder: PlaylistSortOrder) {
        self.current_playlist_tracks
            .sort_by(|x, y| sort_oder.compare(x, y));
    }

    /// returns a list of tracks in the current playback context (album, playlist, etc)
    pub fn get_context_tracks(&self) -> Vec<&track::FullTrack> {
        self.current_playlist_tracks
            .iter()
            .map(|t| t.track.as_ref().unwrap())
            .collect()
    }

    /// returns the list of tracks in the current playback context (album, playlist, etc)
    /// filtered by a search query
    pub fn get_context_filtered_tracks(&self) -> Vec<&track::FullTrack> {
        if self.context_search_state.query.is_some() {
            // in search mode, return the filtered tracks
            self.context_search_state.tracks.iter().collect()
        } else {
            self.get_context_tracks()
        }
    }
}

pub struct TrackDescription {
    pub name: String,
    pub artists: Vec<String>,
    pub album: String,
}

impl fmt::Display for TrackDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{} {} {}",
            self.name,
            self.artists.join(","),
            self.album
        ))
    }
}

pub fn get_track_description(track: &track::FullTrack) -> TrackDescription {
    TrackDescription {
        name: track.name.clone(),
        album: track.album.name.clone(),
        artists: track.artists.iter().map(|a| a.name.clone()).collect(),
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
