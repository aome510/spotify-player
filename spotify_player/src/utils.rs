use crate::prelude::*;

pub fn get_track_description(track: &track::FullTrack) -> String {
    track.name.clone()
}
