use crate::{config, prelude::*};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Default)]
pub struct ContextSearchState {
    pub query: Option<String>,
    pub tracks: Vec<track::FullTrack>,
}

pub struct State {
    pub is_running: bool,
    pub auth_token_expires_at: std::time::SystemTime,
    pub current_playlist: Option<playlist::FullPlaylist>,
    pub current_context_tracks: Vec<playlist::PlaylistTrack>,
    pub current_playback_context: Option<context::CurrentlyPlaybackContext>,

    pub context_search_state: ContextSearchState,

    // UI states
    pub ui_playlist_tracks_list_state: ListState,
}

pub type SharedState = Arc<RwLock<State>>;

impl Default for State {
    fn default() -> Self {
        State {
            is_running: true,
            auth_token_expires_at: std::time::SystemTime::now(),
            current_playlist: None,
            current_context_tracks: vec![],
            current_playback_context: None,

            context_search_state: ContextSearchState::default(),

            ui_playlist_tracks_list_state: ListState::default(),
        }
    }
}

impl State {
    pub fn new() -> SharedState {
        Arc::new(RwLock::new(State::default()))
    }

    pub fn get_context_tracks(&self) -> Vec<&track::FullTrack> {
        self.current_context_tracks
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

pub fn truncate_string(s: String, max_len: usize) -> String {
    let len = UnicodeWidthStr::width(s.as_str());
    if len > max_len {
        // get the longest prefix of the string such that its unicode width
        // is still within the `max_len` limit
        let mut s: String = s
            .chars()
            .fold(("".to_owned(), 0_usize), |(mut cs, cw), c| {
                let w = UnicodeWidthChar::width(c).unwrap_or(2);
                if cw + w > max_len {
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

pub fn fmt_track_descriptions(
    tracks: Vec<TrackDescription>,
    max_horizontal_len: usize,
) -> Vec<String> {
    let layout = [
        max_horizontal_len / 3,
        max_horizontal_len / 3,
        max_horizontal_len - max_horizontal_len / 3 - max_horizontal_len / 3,
    ];
    tracks
        .into_iter()
        .map(|t| {
            [
                truncate_string(t.name, config::TRACK_DESC_ITEM_MAX_LEN),
                truncate_string(t.artists.join(","), config::TRACK_DESC_ITEM_MAX_LEN),
                truncate_string(t.album, config::TRACK_DESC_ITEM_MAX_LEN),
            ]
        })
        .map(|mut descs| {
            descs
                .iter_mut()
                .enumerate()
                .map(|(i, desc)| {
                    let len = UnicodeWidthStr::width(desc.as_str());
                    if len < layout[i] {
                        desc.push_str(&" ".repeat(layout[i] - len));
                    }
                    desc.clone()
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .collect()
}
