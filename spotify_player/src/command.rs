use crate::state::{
    Album, Artist, DataReadGuard, Episode, Playlist, PlaylistFolder, PlaylistFolderItem, Show,
    Track,
};
use serde::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
/// Application's command
pub enum Command {
    None,

    NextTrack,
    PreviousTrack,
    ResumePause,
    PlayRandom,
    Repeat,
    ToggleFakeTrackRepeatMode,
    Shuffle,
    VolumeChange {
        offset: i32,
    },
    Mute,
    SeekForward,
    SeekBackward,

    Quit,
    OpenCommandHelp,
    ClosePopup,

    SelectNextOrScrollDown,
    SelectPreviousOrScrollUp,
    PageSelectNextOrScrollDown,
    PageSelectPreviousOrScrollUp,
    SelectFirstOrScrollToTop,
    SelectLastOrScrollToBottom,

    JumpToCurrentTrackInContext,
    ChooseSelected,

    RefreshPlayback,

    #[cfg(feature = "streaming")]
    RestartIntegratedClient,

    FocusNextWindow,
    FocusPreviousWindow,

    SwitchTheme,
    SwitchDevice,
    Search,
    Queue,

    ShowActionsOnSelectedItem,
    ShowActionsOnCurrentTrack,
    AddSelectedItemToQueue,

    BrowseUserPlaylists,
    BrowseUserFollowedArtists,
    BrowseUserSavedAlbums,

    CurrentlyPlayingContextPage,
    TopTrackPage,
    RecentlyPlayedTrackPage,
    LikedTrackPage,
    LyricsPage,
    LibraryPage,
    SearchPage,
    BrowsePage,
    PreviousPage,
    OpenSpotifyLinkFromClipboard,

    SortTrackByTitle,
    SortTrackByArtists,
    SortTrackByAlbum,
    SortTrackByDuration,
    SortTrackByAddedDate,
    ReverseTrackOrder,

    MovePlaylistItemUp,
    MovePlaylistItemDown,

    CreatePlaylist,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum Action {
    GoToArtist,
    GoToAlbum,
    GoToRadio,
    GoToShow,
    AddToLibrary,
    AddToPlaylist,
    AddToQueue,
    AddToLiked,
    DeleteFromLiked,
    DeleteFromLibrary,
    DeleteFromPlaylist,
    ShowActionsOnAlbum,
    ShowActionsOnArtist,
    ShowActionsOnShow,
    ToggleLiked,
    CopyLink,
    Follow,
    Unfollow,
}

#[derive(Debug)]
pub enum ActionContext {
    Track(Track),
    Album(Album),
    Artist(Artist),
    Playlist(Playlist),
    Episode(Episode),
    #[allow(dead_code)]
    // TODO: support actions for playlist folders
    PlaylistFolder(PlaylistFolder),
    Show(Show),
}

#[derive(Debug, PartialEq, Clone, Deserialize, Default, Copy)]
pub enum ActionTarget {
    PlayingTrack,
    #[default]
    SelectedItem,
}

pub enum CommandOrAction {
    Command(Command),
    Action(Action, ActionTarget),
}

impl From<Track> for ActionContext {
    fn from(v: Track) -> Self {
        Self::Track(v)
    }
}

impl From<Artist> for ActionContext {
    fn from(v: Artist) -> Self {
        Self::Artist(v)
    }
}

impl From<Album> for ActionContext {
    fn from(v: Album) -> Self {
        Self::Album(v)
    }
}

impl From<Playlist> for ActionContext {
    fn from(v: Playlist) -> Self {
        Self::Playlist(v)
    }
}

impl From<Show> for ActionContext {
    fn from(v: Show) -> Self {
        Self::Show(v)
    }
}

impl From<Episode> for ActionContext {
    fn from(v: Episode) -> Self {
        Self::Episode(v)
    }
}

impl From<PlaylistFolderItem> for ActionContext {
    fn from(value: PlaylistFolderItem) -> Self {
        match value {
            PlaylistFolderItem::Playlist(p) => ActionContext::Playlist(p),
            PlaylistFolderItem::Folder(f) => ActionContext::PlaylistFolder(f),
        }
    }
}

impl ActionContext {
    pub fn get_available_actions(&self, data: &DataReadGuard) -> Vec<Action> {
        match self {
            Self::Track(track) => construct_track_actions(track, data),
            Self::Album(album) => construct_album_actions(album, data),
            Self::Artist(artist) => construct_artist_actions(artist, data),
            Self::Playlist(playlist) => construct_playlist_actions(playlist, data),
            Self::Episode(episode) => construct_episode_actions(episode, data),
            // TODO: support actions for playlist folders
            Self::PlaylistFolder(_) => vec![],
            Self::Show(show) => construct_show_actions(show, data),
        }
    }
}

/// constructs a list of actions on a track
pub fn construct_track_actions(track: &Track, data: &DataReadGuard) -> Vec<Action> {
    let mut actions = vec![
        Action::GoToArtist,
        Action::GoToAlbum,
        Action::GoToRadio,
        Action::ShowActionsOnAlbum,
        Action::ShowActionsOnArtist,
        Action::CopyLink,
        Action::AddToPlaylist,
        Action::AddToQueue,
    ];

    if data.user_data.is_liked_track(track) {
        actions.push(Action::DeleteFromLiked);
    } else {
        actions.push(Action::AddToLiked);
    }

    actions
}

/// constructs a list of actions on an album
pub fn construct_album_actions(album: &Album, data: &DataReadGuard) -> Vec<Action> {
    let mut actions = vec![
        Action::GoToArtist,
        Action::GoToRadio,
        Action::ShowActionsOnArtist,
        Action::CopyLink,
        Action::AddToQueue,
    ];
    if data.user_data.saved_albums.iter().any(|a| a.id == album.id) {
        actions.push(Action::DeleteFromLibrary);
    } else {
        actions.push(Action::AddToLibrary);
    }
    actions
}

/// constructs a list of actions on an artist
pub fn construct_artist_actions(artist: &Artist, data: &DataReadGuard) -> Vec<Action> {
    let mut actions = vec![Action::GoToRadio, Action::CopyLink];

    if data
        .user_data
        .followed_artists
        .iter()
        .any(|a| a.id == artist.id)
    {
        actions.push(Action::Unfollow);
    } else {
        actions.push(Action::Follow);
    }
    actions
}

/// constructs a list of actions on an playlist
pub fn construct_playlist_actions(playlist: &Playlist, data: &DataReadGuard) -> Vec<Action> {
    let mut actions = vec![Action::GoToRadio, Action::CopyLink];

    if data
        .user_data
        .playlists
        .iter()
        .any(|item| matches!(item, PlaylistFolderItem::Playlist(p) if p.id == playlist.id))
    {
        actions.push(Action::DeleteFromLibrary);
    } else {
        actions.push(Action::AddToLibrary);
    }
    actions
}

/// constructs a list of actions on a show
pub fn construct_show_actions(show: &Show, data: &DataReadGuard) -> Vec<Action> {
    let mut actions = vec![Action::CopyLink];
    if data.user_data.saved_shows.iter().any(|s| s.id == show.id) {
        actions.push(Action::DeleteFromLibrary);
    } else {
        actions.push(Action::AddToLibrary);
    }
    actions
}

/// constructs a list of actions on an episode
pub fn construct_episode_actions(episode: &Episode, _data: &DataReadGuard) -> Vec<Action> {
    let mut actions = vec![Action::CopyLink, Action::AddToPlaylist, Action::AddToQueue];
    if episode.show.is_some() {
        actions.push(Action::ShowActionsOnShow);
        actions.push(Action::GoToShow);
    }
    actions
}

impl Command {
    pub fn desc(self) -> String {
        if let Self::VolumeChange { offset } = self {
            return format!("change playback volume by {offset}");
        }

        match self {
            Self::None => "do nothing",
            Self::NextTrack => "next track",
            Self::PreviousTrack => "previous track",
            Self::ResumePause => "resume/pause based on the current playback",
            Self::PlayRandom => "play a random track in the current context",
            Self::Repeat => "cycle the repeat mode",
            Self::ToggleFakeTrackRepeatMode => "toggle fake track repeat mode",
            Self::Shuffle => "toggle the shuffle mode",
            Self::Mute => "toggle playback volume between 0% and previous level",
            Self::SeekForward => "seek forward by 5s",
            Self::SeekBackward => "seek backward by 5s",
            Self::Quit => "quit the application",
            Self::ClosePopup => "close a popup",
            #[cfg(feature = "streaming")]
            Self::RestartIntegratedClient => "restart the integrated client",
            Self::SelectNextOrScrollDown => "select the next item in a list/table or scroll down",
            Self::SelectPreviousOrScrollUp => {
                "select the previous item in a list/table or scroll up"
            }
            Self::PageSelectNextOrScrollDown => {
                "select the next page item in a list/table or scroll a page down"
            }
            Self::PageSelectPreviousOrScrollUp => {
                "select the previous page item in a list/table or scroll a page up"
            }
            Self::SelectFirstOrScrollToTop => {
                "select the first item in a list/table or scroll to the top"
            }
            Self::SelectLastOrScrollToBottom => {
                "select the last item in a list/table or scroll to the bottom"
            }
            Self::ChooseSelected => "choose the selected item and act on it",
            Self::JumpToCurrentTrackInContext => "jump to the current track in the context",
            Self::RefreshPlayback => "manually refresh the current playback",
            Self::ShowActionsOnSelectedItem => "open a popup showing actions on a selected item",
            Self::ShowActionsOnCurrentTrack => "open a popup showing actions on the current track",
            Self::AddSelectedItemToQueue => "add the selected item to queue",
            Self::FocusNextWindow => "focus the next focusable window (if any)",
            Self::FocusPreviousWindow => "focus the previous focusable window (if any)",
            Self::SwitchTheme => "open a popup for switching theme",
            Self::SwitchDevice => "open a popup for switching device",
            Self::Search => "open a popup for searching in the current page",
            Self::BrowseUserPlaylists => "open a popup for browsing user's playlists",
            Self::BrowseUserFollowedArtists => "open a popup for browsing user's followed artists",
            Self::BrowseUserSavedAlbums => "open a popup for browsing user's saved albums",
            Self::CurrentlyPlayingContextPage => "go to the currently playing context page",
            Self::TopTrackPage => "go to the user top track page",
            Self::RecentlyPlayedTrackPage => "go to the user recently played track page",
            Self::LikedTrackPage => "go to the user liked track page",
            Self::LyricsPage => "go to the lyrics page of the current track",
            Self::LibraryPage => "go to the user library page",
            Self::SearchPage => "go to the search page",
            Self::BrowsePage => "go to the browse page",
            Self::Queue => "go to the queue page",
            Self::OpenCommandHelp => "go to the command help page",
            Self::PreviousPage => "go to the previous page",
            Self::OpenSpotifyLinkFromClipboard => "open a Spotify link from clipboard",
            Self::SortTrackByTitle => "sort the track table (if any) by track's title",
            Self::SortTrackByArtists => "sort the track table (if any) by track's artists",
            Self::SortTrackByAlbum => "sort the track table (if any) by track's album",
            Self::SortTrackByDuration => "sort the track table (if any) by track's duration",
            Self::SortTrackByAddedDate => "sort the track table (if any) by track's added date",
            Self::ReverseTrackOrder => "reverse the order of the track table (if any)",
            Self::MovePlaylistItemUp => "move playlist item up one position",
            Self::MovePlaylistItemDown => "move playlist item down one position",
            Self::CreatePlaylist => "create a new playlist",
            Self::VolumeChange { offset: _ } => unreachable!(),
        }
        .to_string()
    }
}
