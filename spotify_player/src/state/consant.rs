pub use super::*;

pub const USER_TOP_TRACKS_ID: TracksId =
    TracksId::new("tracks:user-top-tracks", "User's top tracks", "Top Tracks");

pub const USER_RECENTLY_PLAYED_TRACKS_ID: TracksId = TracksId::new(
    "tracks:user-recently-played-tracks",
    "User's recently played tracks",
    "Recently Played Tracks",
);

pub const USER_LIKED_TRACKS_ID: TracksId = TracksId::new(
    "tracks:user-liked-tracks",
    "User's liked tracks",
    "Liked Tracks",
);
