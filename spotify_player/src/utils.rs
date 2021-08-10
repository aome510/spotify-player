use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{event, state};

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

/// updates the current playback by fetching playback data from spotify every
/// `AppConfig::refresh_delay_in_ms_each_playback_update`, repeated `AppConfig::n_refreshes_each_playback_update` times.
pub fn update_playback(state: &state::SharedState, send: &std::sync::mpsc::Sender<event::Event>) {
    let n_refreshes = state.app_config.n_refreshes_each_playback_update;
    let delay_duration =
        std::time::Duration::from_millis(state.app_config.refresh_delay_in_ms_each_playback_update);

    std::thread::spawn({
        let send = send.clone();
        move || {
            (0..n_refreshes).for_each(|_| {
                std::thread::sleep(delay_duration);
                send.send(event::Event::GetCurrentPlayback)
                    .unwrap_or_else(|err| {
                        log::warn!("failed to send GetCurrentPlayback event: {:#?}", err);
                    });
            });
        }
    });
}

/// updates the current playing context
pub fn update_context(state: &state::SharedState, context: state::Context) {
    std::thread::spawn({
        let state = state.clone();
        move || {
            // reset UI states upon context switching
            let mut ui = state.ui.lock().unwrap();
            ui.context_tracks_table_ui_state = tui::widgets::TableState::default();
            ui.context_tracks_table_ui_state.select(Some(0));
            state.player.write().unwrap().context = context;
        }
    });
}
