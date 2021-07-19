use crate::prelude::*;

pub struct State {
    pub is_running: bool,
    pub auth_token_expires_at: std::time::SystemTime,
    pub current_playlist: Option<playlist::FullPlaylist>,
    pub current_playlist_tracks: Option<Vec<playlist::PlaylistTrack>>,
    pub current_playback_context: Option<context::CurrentlyPlaybackContext>,
}

pub type SharedState = Arc<RwLock<State>>;

impl Default for State {
    fn default() -> Self {
        State {
            is_running: true,
            auth_token_expires_at: std::time::SystemTime::now(),
            current_playlist: None,
            current_playlist_tracks: None,
            current_playback_context: None,
        }
    }
}

impl State {
    pub fn new() -> SharedState {
        Arc::new(RwLock::new(State::default()))
    }
}
