use tui::widgets::{ListState, TableState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::state::{self, ArtistFocusState, PopupState, WindowState};

/// formats a time duration (in ms) into a "{minutes}:{seconds}" format
pub fn format_duration(duration: u32) -> String {
    format!("{}:{:02}", duration / 60000, (duration / 1000) % 60)
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

pub fn new_list_state() -> ListState {
    let mut state = ListState::default();
    state.select(Some(0));
    state
}

pub fn new_table_state() -> TableState {
    let mut state = TableState::default();
    state.select(Some(0));
    state
}

/// updates the current playing context
pub fn update_context(state: &state::SharedState, context_uri: String) {
    std::thread::spawn({
        let state = state.clone();
        move || {
            log::info!("update state context uri: {}", context_uri);
            state.player.write().unwrap().context_uri = context_uri;

            state.ui.lock().unwrap().window = state::WindowState::Unknown;

            let refresh_duration =
                std::time::Duration::from_millis(state.app_config.app_refresh_duration_in_ms);

            // spawn a pooling job to check when the context is updated inside the player state
            loop {
                if let Some(context) = state.player.read().unwrap().get_context() {
                    // update the UI's context state based on the player's context state
                    let mut ui = state.ui.lock().unwrap();
                    match context {
                        state::Context::Artist(..) => {
                            ui.window = WindowState::Artist(
                                new_table_state(),
                                new_list_state(),
                                new_list_state(),
                                ArtistFocusState::TopTracks,
                            );
                        }
                        state::Context::Album(..) => {
                            ui.window = WindowState::Album(new_table_state());
                        }
                        state::Context::Playlist(..) => {
                            ui.window = WindowState::Playlist(new_table_state());
                        }
                        state::Context::Unknown(_) => {
                            ui.window = WindowState::Unknown;
                        }
                    }
                    ui.popup = PopupState::None;
                    break;
                }
                std::thread::sleep(refresh_duration);
            }
        }
    });
}
