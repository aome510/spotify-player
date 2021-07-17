use std::sync::{Arc, RwLock};

use rspotify::model::context;

pub struct State {
    pub is_running: bool,
    pub auth_token_expires_at: std::time::SystemTime,
    pub current_playing_context: Option<context::CurrentlyPlayingContext>,
}

pub type SharedState = Arc<RwLock<State>>;

impl Default for State {
    fn default() -> Self {
        State {
            is_running: true,
            auth_token_expires_at: std::time::SystemTime::now(),
            current_playing_context: None,
        }
    }
}

impl State {
    pub fn new() -> SharedState {
        Arc::new(RwLock::new(State::default()))
    }
}
