use crate::state::*;

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
    GetUserFollowedArtists,
    GetUserSavedTracks,
    GetUserTopTracks,
    GetUserRecentlyPlayedTracks,
    GetContext(ContextId),
    GetCurrentPlayback,
    GetRadioTracks {
        seed_uri: String,
        seed_name: String,
    },
    Search(String),
    AddTrackToQueue(TrackId<'static>),
    AddTrackToPlaylist(PlaylistId<'static>, TrackId<'static>),
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
    ConnectDevice(Option<String>),
    Player(PlayerRequest),
    GetCurrentUserQueue,
    #[cfg(feature = "lyric-finder")]
    GetLyric {
        track: String,
        artists: String,
    },
    #[cfg(feature = "streaming")]
    RestartIntegratedClient,
    CreatePlaylist {
        playlist_name: String,
        public: bool,
        collab: bool,
        desc: String,
    },
    #[cfg(feature = "streaming")]
    HandleStreamingEvent(crate::streaming::PlayerEvent),
}
