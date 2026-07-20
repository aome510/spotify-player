use crate::state::{
    AlbumId, Category, ContextId, Item, ItemId, PlayableId, Playback, PlaylistId, TrackId,
};

#[derive(Clone, Debug)]
/// A request that modifies the player's playback
pub enum PlayerRequest {
    NextTrack,
    PreviousTrack,
    Resume,
    Pause,
    ResumePause,
    SeekTrack(chrono::Duration),
    Repeat,
    Shuffle,
    Volume(u8),
    ToggleMute,
    TransferPlayback(String, bool),
    StartPlayback(Playback, Option<bool>),
}

#[derive(Clone, Debug)]
/// A request to the client
pub enum ClientRequest {
    GetCurrentUser,
    GetDevices,
    GetBrowseCategories,
    GetBrowseCategoryPlaylists(Category),
    GetUserPlaylists,
    GetUserSavedAlbums,
    GetUserSavedShows,
    GetUserFollowedArtists,
    GetContext(ContextId),
    GetCurrentPlayback,
    Search(String),
    AddPlayableToQueue(PlayableId<'static>),
    AddAlbumToQueue(AlbumId<'static>),
    AddPlayableToPlaylist(PlaylistId<'static>, PlayableId<'static>),
    DeleteTrackFromPlaylist(PlaylistId<'static>, TrackId<'static>),
    ReorderPlaylistItems {
        playlist_id: PlaylistId<'static>,
        insert_index: usize,
        range_start: usize,
        range_length: Option<usize>,
        snapshot_id: Option<String>,
    },
    AddToLibrary(Item),
    DeleteFromLibrary(ItemId),
    Player(PlayerRequest),
    GetCurrentUserQueue,
    GetLyrics {
        track_id: TrackId<'static>,
    },
    #[cfg(feature = "streaming")]
    RestartIntegratedClient,
    CreatePlaylist {
        playlist_name: String,
        public: bool,
        collab: bool,
        desc: String,
    },
}

impl ClientRequest {
    /// Whether this request must run in the order it was received.
    ///
    /// Requests in this group either change Spotify state or refresh player state that can
    /// conflict with those changes. Other data-fetching requests remain concurrent so slow
    /// catalog or library loads do not delay this lane.
    pub(crate) fn requires_ordered_execution(&self) -> bool {
        match self {
            Self::GetCurrentUser
            | Self::GetDevices
            | Self::GetBrowseCategories
            | Self::GetBrowseCategoryPlaylists(_)
            | Self::GetUserPlaylists
            | Self::GetUserSavedAlbums
            | Self::GetUserSavedShows
            | Self::GetUserFollowedArtists
            | Self::GetContext(_)
            | Self::Search(_)
            | Self::GetLyrics { .. } => false,
            Self::GetCurrentPlayback
            | Self::AddPlayableToQueue(_)
            | Self::AddAlbumToQueue(_)
            | Self::AddPlayableToPlaylist(_, _)
            | Self::DeleteTrackFromPlaylist(_, _)
            | Self::ReorderPlaylistItems { .. }
            | Self::AddToLibrary(_)
            | Self::DeleteFromLibrary(_)
            | Self::Player(_)
            | Self::GetCurrentUserQueue
            | Self::CreatePlaylist { .. } => true,
            #[cfg(feature = "streaming")]
            Self::RestartIntegratedClient => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientRequest, PlayerRequest};

    #[test]
    fn stateful_requests_require_ordered_execution() {
        assert!(ClientRequest::Player(PlayerRequest::Pause).requires_ordered_execution());
        assert!(ClientRequest::GetCurrentPlayback.requires_ordered_execution());
        assert!(ClientRequest::CreatePlaylist {
            playlist_name: "playlist".to_owned(),
            public: false,
            collab: false,
            desc: String::new(),
        }
        .requires_ordered_execution());
    }

    #[test]
    fn independent_reads_allow_concurrent_execution() {
        assert!(!ClientRequest::GetBrowseCategories.requires_ordered_execution());
        assert!(!ClientRequest::GetUserSavedAlbums.requires_ordered_execution());
        assert!(!ClientRequest::Search("query".to_owned()).requires_ordered_execution());
    }
}
