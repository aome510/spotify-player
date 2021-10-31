use tui::widgets::{ListState, TableState};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
