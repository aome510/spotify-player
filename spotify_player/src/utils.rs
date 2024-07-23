use std::borrow::Cow;

use tui::widgets::*;

use blake3;

/// formats a time duration into a "{minutes}:{seconds}" format
pub fn format_duration(duration: &chrono::Duration) -> String {
    let secs = duration.num_seconds();
    format!("{}:{:02}", secs / 60, secs % 60)
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

#[allow(dead_code)]
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

pub fn hash_filename(filename: &str) -> String {
    // return the first 16 characters of the hash
    let hash = blake3::hash(filename.as_bytes());
    hash.to_hex().to_string().split_at(16).0.to_string()
}