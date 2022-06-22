use std::borrow::Cow;

use tui::widgets::*;
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

pub fn map_join<T, F>(v: &[T], f: F, sep: &str) -> String
where
    F: Fn(&T) -> &str,
{
    v.iter().map(f).fold(String::new(), |x, y| {
        if x.is_empty() {
            x + y
        } else {
            x + sep + y
        }
    })
}

pub fn get_track_album_image_url(track: &rspotify::model::FullTrack) -> Option<&str> {
    if track.album.images.is_empty() {
        None
    } else {
        Some(&track.album.images[0].url)
    }
}

pub fn parse_uri(uri: &str) -> Cow<str> {
    let parts = uri.split(':').collect::<Vec<_>>();
    // The below URI probably has a format of `spotify:user:{user_id}:{type}:{id}`,
    // but `rspotify` library expects to receive an URI of format `spotify:{type}:{id}`.
    // We have to modify the URI to a corresponding format.
    // See: https://github.com/aome510/spotify-player/issues/57#issuecomment-1160868626
    if parts.len() == 5 {
        Cow::Owned([parts[0], parts[3], parts[4]].join(":"))
    } else {
        Cow::Borrowed(uri)
    }
}
