use tui::widgets::{ListState, TableState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::state::{self, ArtistFocusState, WindowState};

/// formats a time duration into a "{minutes}:{seconds}" format
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
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
pub fn update_context(state: &state::SharedState, context_id: Option<state::ContextId>) {
    std::thread::spawn({
        let state = state.clone();
        move || {
            log::info!("update state's context id to {:#?}", context_id);

            let is_none_context = context_id.is_none();

            state.player.write().unwrap().context_id = context_id;
            state.ui.lock().unwrap().window = state::WindowState::Unknown;

            // `None` context, skip pooling
            if is_none_context {
                return;
            }

            let refresh_duration =
                std::time::Duration::from_millis(state.app_config.app_refresh_duration_in_ms);

            // spawn a pooling job to check when the context is updated inside the player state
            loop {
                let window_state = match state.player.read().unwrap().context() {
                    Some(context) => match context {
                        state::Context::Artist(..) => WindowState::Artist(
                            new_table_state(),
                            new_list_state(),
                            new_list_state(),
                            ArtistFocusState::TopTracks,
                        ),
                        state::Context::Album(..) => WindowState::Album(new_table_state()),
                        state::Context::Playlist(..) => WindowState::Playlist(new_table_state()),
                    },
                    None => {
                        std::thread::sleep(refresh_duration);
                        continue;
                    }
                };

                // update the UI states based on the new playing context
                let mut ui = state.ui.lock().unwrap();
                ui.window = window_state;
                break;
            }
        }
    });
}
