pub use super::*;
use once_cell::sync::Lazy;

pub const USER_TOP_TRACKS_ID: Lazy<TracksId> =
    Lazy::new(|| TracksId::new("tracks:user-top-tracks", "Top Tracks"));

pub const USER_RECENTLY_PLAYED_TRACKS_ID: Lazy<TracksId> = Lazy::new(|| {
    TracksId::new(
        "tracks:user-recently-played-tracks",
        "Recently Played Tracks",
    )
});

pub const USER_LIKED_TRACKS_ID: Lazy<TracksId> =
    Lazy::new(|| TracksId::new("tracks:user-liked-tracks", "Liked Tracks"));
