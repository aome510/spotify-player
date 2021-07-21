use crate::prelude::*;

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
