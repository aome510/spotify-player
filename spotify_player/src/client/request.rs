use crate::state::{
    AlbumId, Category, ContextId, Id, Item, ItemId, PlayableId, Playback, PlaylistId, RequestKey,
    RequestMetadata, RequestOperation, TrackId,
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
    pub(crate) fn metadata(&self) -> RequestMetadata {
        match self {
            Self::GetCurrentUser => RequestMetadata::tracked(RequestKey::CurrentUser),
            Self::GetDevices => RequestMetadata::tracked(RequestKey::Devices),
            Self::GetBrowseCategories => RequestMetadata::tracked(RequestKey::BrowseCategories),
            Self::GetBrowseCategoryPlaylists(category) => {
                RequestMetadata::tracked(RequestKey::BrowseCategory(category.id.clone()))
            }
            Self::GetUserPlaylists => RequestMetadata::tracked(RequestKey::UserPlaylists),
            Self::GetUserSavedAlbums => RequestMetadata::tracked(RequestKey::UserSavedAlbums),
            Self::GetUserSavedShows => RequestMetadata::tracked(RequestKey::UserSavedShows),
            Self::GetUserFollowedArtists => {
                RequestMetadata::tracked(RequestKey::UserFollowedArtists)
            }
            Self::GetContext(context) => {
                RequestMetadata::tracked(RequestKey::Context(context.uri()))
            }
            Self::GetCurrentPlayback => RequestMetadata::default(),
            Self::Search(query) => RequestMetadata::tracked(RequestKey::Search(query.clone())),
            Self::AddPlayableToQueue(_) => RequestMetadata::operation(RequestOperation::new(
                "Adding item to queue...",
                "Item added to queue",
                "Could not add item to queue",
            )),
            Self::AddAlbumToQueue(_) => RequestMetadata::operation(RequestOperation::new(
                "Adding album to queue...",
                "Album added to queue",
                "Could not add album to queue",
            )),
            Self::AddPlayableToPlaylist(_, _) => RequestMetadata::operation(RequestOperation::new(
                "Adding item to playlist...",
                "Item added to playlist",
                "Could not add item to playlist",
            )),
            Self::DeleteTrackFromPlaylist(_, _) => {
                RequestMetadata::operation(RequestOperation::new(
                    "Removing track from playlist...",
                    "Track removed from playlist",
                    "Could not remove track from playlist",
                ))
            }
            Self::ReorderPlaylistItems { .. } => RequestMetadata::operation(RequestOperation::new(
                "Reordering playlist...",
                "Playlist reordered",
                "Could not reorder playlist",
            )),
            Self::AddToLibrary(item) => {
                let kind = item_kind(item);
                RequestMetadata::operation(RequestOperation::new(
                    format!("Adding {kind} to library..."),
                    format!("{kind} added to library"),
                    format!("Could not add {kind} to library"),
                ))
            }
            Self::DeleteFromLibrary(item_id) => {
                let kind = item_id_kind(item_id);
                RequestMetadata::operation(RequestOperation::new(
                    format!("Removing {kind} from library..."),
                    format!("{kind} removed from library"),
                    format!("Could not remove {kind} from library"),
                ))
            }
            Self::Player(request) => RequestMetadata::operation(request.operation()),
            Self::GetCurrentUserQueue => RequestMetadata::tracked(RequestKey::Queue),
            Self::GetLyrics { track_id } => {
                RequestMetadata::tracked(RequestKey::Lyrics(track_id.uri()))
            }
            #[cfg(feature = "streaming")]
            Self::RestartIntegratedClient => RequestMetadata::operation(RequestOperation::new(
                "Restarting integrated client...",
                "Integrated client restarted",
                "Could not restart integrated client",
            )),
            Self::CreatePlaylist { .. } => RequestMetadata::tracked_operation(
                RequestKey::CreatePlaylist,
                RequestOperation::new(
                    "Creating playlist...",
                    "Playlist created",
                    "Could not create playlist",
                ),
            ),
        }
    }
}

impl PlayerRequest {
    fn operation(&self) -> RequestOperation {
        let (pending, success, failure) = match self {
            Self::NextTrack => (
                "Skipping to next track...",
                "Playing next track",
                "Could not skip to next track",
            ),
            Self::PreviousTrack => (
                "Returning to previous track...",
                "Playing previous track",
                "Could not return to previous track",
            ),
            Self::Resume => (
                "Resuming playback...",
                "Playback resumed",
                "Could not resume playback",
            ),
            Self::Pause => (
                "Pausing playback...",
                "Playback paused",
                "Could not pause playback",
            ),
            Self::ResumePause => (
                "Toggling playback...",
                "Playback updated",
                "Could not toggle playback",
            ),
            Self::SeekTrack(_) => (
                "Seeking track...",
                "Track position updated",
                "Could not seek track",
            ),
            Self::Repeat => (
                "Updating repeat mode...",
                "Repeat mode updated",
                "Could not update repeat mode",
            ),
            Self::Shuffle => (
                "Updating shuffle mode...",
                "Shuffle mode updated",
                "Could not update shuffle mode",
            ),
            Self::Volume(_) => (
                "Updating volume...",
                "Volume updated",
                "Could not update volume",
            ),
            Self::ToggleMute => ("Toggling mute...", "Mute updated", "Could not toggle mute"),
            Self::TransferPlayback(_, _) => (
                "Switching playback device...",
                "Playback device switched",
                "Could not switch playback device",
            ),
            Self::StartPlayback(_, _) => (
                "Starting playback...",
                "Playback started",
                "Could not start playback",
            ),
        };
        RequestOperation::new(pending, success, failure)
    }
}

fn item_kind(item: &Item) -> &'static str {
    match item {
        Item::Track(_) => "Track",
        Item::Album(_) => "Album",
        Item::Artist(_) => "Artist",
        Item::Playlist(_) => "Playlist",
        Item::Show(_) => "Show",
    }
}

fn item_id_kind(item_id: &ItemId) -> &'static str {
    match item_id {
        ItemId::Track(_) => "Track",
        ItemId::Album(_) => "Album",
        ItemId::Artist(_) => "Artist",
        ItemId::Playlist(_) => "Playlist",
        ItemId::Show(_) => "Show",
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientRequest, PlayerRequest};
    use crate::state::RequestKey;

    #[test]
    fn page_requests_have_stable_tracking_keys() {
        assert_eq!(
            ClientRequest::Search("query".to_owned()).metadata().key,
            Some(RequestKey::Search("query".to_owned()))
        );
        assert_eq!(
            ClientRequest::GetCurrentUserQueue.metadata().key,
            Some(RequestKey::Queue)
        );
    }

    #[test]
    fn mutations_expose_operation_feedback() {
        assert!(ClientRequest::Player(PlayerRequest::Pause)
            .metadata()
            .operation
            .is_some());
        assert!(ClientRequest::CreatePlaylist {
            playlist_name: "playlist".to_owned(),
            public: false,
            collab: false,
            desc: String::new(),
        }
        .metadata()
        .operation
        .is_some());
    }
}
