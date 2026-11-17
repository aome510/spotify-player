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
    pub fn description(&self) -> Option<&'static str> {
        let description = match self {
            Self::GetCurrentPlayback | Self::GetCurrentUserQueue => return None,
            Self::GetCurrentUser => "load user profile",
            Self::GetDevices => "load devices",
            Self::GetBrowseCategories => "load browse categories",
            Self::GetBrowseCategoryPlaylists(_) => "load category playlists",
            Self::GetUserPlaylists => "load playlists",
            Self::GetUserSavedAlbums => "load saved albums",
            Self::GetUserSavedShows => "load saved shows",
            Self::GetUserFollowedArtists => "load followed artists",
            Self::GetContext(_) => "load page",
            Self::Search(_) => "search",
            Self::AddPlayableToQueue(_) | Self::AddAlbumToQueue(_) => "add to queue",
            Self::AddPlayableToPlaylist(..) => "add to playlist",
            Self::DeleteTrackFromPlaylist(..) => "remove from playlist",
            Self::ReorderPlaylistItems { .. } => "reorder playlist",
            Self::AddToLibrary(_) => "add to library",
            Self::DeleteFromLibrary(_) => "remove from library",
            Self::Player(_) => "control playback",
            Self::GetLyrics { .. } => "load lyrics",
            #[cfg(feature = "streaming")]
            Self::RestartIntegratedClient => "restart integrated client",
            Self::CreatePlaylist { .. } => "create playlist",
        };
        Some(description)
    }
}
