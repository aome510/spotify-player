pub use super::*;
use std::sync::LazyLock;

pub static USER_TOP_TRACKS_ID: LazyLock<TracksId> =
    LazyLock::new(|| TracksId::new("tracks:user-top-tracks", "Top Tracks"));

pub static USER_RECENTLY_PLAYED_TRACKS_ID: LazyLock<TracksId> = LazyLock::new(|| {
    TracksId::new(
        "tracks:user-recently-played-tracks",
        "Recently Played Tracks",
    )
});

pub static USER_LIKED_TRACKS_ID: LazyLock<TracksId> =
    LazyLock::new(|| TracksId::new("tracks:user-liked-tracks", "Liked Tracks"));
