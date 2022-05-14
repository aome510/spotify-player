use serde::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
/// Application's command
pub enum Command {
    NextTrack,
    PreviousTrack,
    ResumePause,
    PlayRandom,
    Repeat,
    Shuffle,
    VolumeUp,
    VolumeDown,

    Quit,
    OpenCommandHelp,
    ClosePopup,

    SelectNextOrScrollDown,
    SelectPreviousOrScrollUp,
    ChooseSelected,

    RefreshPlayback,

    ReconnectIntegratedClient,

    FocusNextWindow,
    FocusPreviousWindow,

    SwitchTheme,
    SwitchDevice,
    Search,

    ShowActionsOnSelectedItem,
    ShowActionsOnCurrentTrack,

    BrowseUserPlaylists,
    BrowseUserFollowedArtists,
    BrowseUserSavedAlbums,

    CurrentlyPlayingContextPage,
    TopTrackPage,
    RecentlyPlayedTrackPage,
    LyricPage,
    LibraryPage,
    SearchPage,
    PreviousPage,

    SortTrackByTitle,
    SortTrackByArtists,
    SortTrackByAlbum,
    SortTrackByDuration,
    SortTrackByAddedDate,
    ReverseTrackOrder,
}

/// An action on a Spotify item (track,album,artist,playlist)
#[derive(Copy, Clone, Debug)]
pub enum Action {
    AddTrackToPlaylist,
    SaveToLibrary,
    BrowseArtist,
    BrowseAlbum,
    BrowseRecommendations,
}

impl Command {
    pub fn desc(&self) -> &'static str {
        match self {
            Self::NextTrack => "next track",
            Self::PreviousTrack => "previous track",
            Self::ResumePause => "resume/pause based on the current playback",
            Self::PlayRandom => "play a random track in the current context",
            Self::Repeat => "cycle the repeat mode",
            Self::Shuffle => "toggle the shuffle mode",
            Self::VolumeUp => "increase playback volume by 5%",
            Self::VolumeDown => "decrease playback volume by 5%",
            Self::Quit => "quit the application",
            Self::OpenCommandHelp => "open a command help popup",
            Self::ClosePopup => "close a popup",
            Self::ReconnectIntegratedClient => "reconnect the integrated librespot client",
            Self::SelectNextOrScrollDown => "select the next item in a list/table or scroll down",
            Self::SelectPreviousOrScrollUp => {
                "select the previous item in a list/table or scroll up"
            }
            Self::ChooseSelected => "choose the selected item and act on it",
            Self::RefreshPlayback => "manually refresh the current playback",
            Self::ShowActionsOnSelectedItem => "open a popup showing actions on a selected item",
            Self::ShowActionsOnCurrentTrack => {
                "open a popup showing actions on the currently playing track"
            }
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
            Self::LyricPage => "go to the lyric page",
            Self::LibraryPage => "go to the user libary page",
            Self::SearchPage => "go to the search page",
            Self::PreviousPage => "go to the previous page",
            Self::SortTrackByTitle => "sort the track table (if any) by track's title",
            Self::SortTrackByArtists => "sort the track table (if any) by track's artists",
            Self::SortTrackByAlbum => "sort the track table (if any) by track's album",
            Self::SortTrackByDuration => "sort the track table (if any) by track's duration",
            Self::SortTrackByAddedDate => "sort the track table (if any) by track's added date",
            Self::ReverseTrackOrder => "reverse the order of the track table (if any)",
        }
    }
}
