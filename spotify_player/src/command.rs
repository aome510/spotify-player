use serde::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
/// Application's command
pub enum Command {
    NextTrack,
    PreviousTrack,
    ResumePause,
    PlayContext,
    Repeat,
    Shuffle,

    Quit,
    OpenCommandHelp,
    ClosePopup,

    SelectNext,
    SelectPrevious,
    ChooseSelected,

    RefreshPlayback,

    FocusNextWindow,
    FocusPreviousWindow,

    SwitchTheme,
    SwitchDevice,

    SearchContext,

    BrowseUserPlaylists,
    BrowseUserSavedAlbums,
    BrowseUserSavedArtists,
    BrowsePlayingTrackArtists,
    BrowsePlayingTrackAlbum,
    BrowsePlayingContext,
    BrowseSelectedTrackArtists,
    BrowseSelectedTrackAlbum,
    PreviousPage,

    SortTrackByTitle,
    SortTrackByArtists,
    SortTrackByAlbum,
    SortTrackByDuration,
    SortTrackByAddedDate,
    ReverseTrackOrder,
}

impl Command {
    pub fn desc(&self) -> &'static str {
        match self {
            Self::NextTrack => "next track",
            Self::PreviousTrack => "previous track",
            Self::ResumePause => "resume/pause based on the playback",
            Self::PlayContext => "play a random track in the current context",
            Self::Repeat => "cycle the repeat mode",
            Self::Shuffle => "toggle the shuffle mode",
            Self::Quit => "quit the application",
            Self::OpenCommandHelp => "open the help a command help popup",
            Self::ClosePopup => "close a popup",
            Self::SelectNext => "select the next item in the focused list or a table",
            Self::SelectPrevious => "select the previous item in the focused list or a table",
            Self::ChooseSelected => "choose the selected item and act on it",
            Self::RefreshPlayback => "refresh the current playback",
            Self::FocusNextWindow => "focus the next focusable window (if any)",
            Self::FocusPreviousWindow => "focus the previous focusable window (if any)",
            Self::SwitchTheme => "open a popup for switching theme",
            Self::SwitchDevice => "open a popup for switching device",
            Self::SearchContext => "open a search popup for searching in context",
            Self::PreviousPage => "go to the previous page",
            Self::BrowseUserPlaylists => "open a popup for browsing user's playlists",
            Self::BrowseUserSavedAlbums => "open a popup for browsing user's saved albums",
            Self::BrowseUserSavedArtists => "open a popup for browsing user's saved artists",
            Self::BrowsePlayingContext => "browse the current playing context",
            Self::BrowsePlayingTrackArtists => {
                "open a popup for browsing current playing track's artists"
            }
            Self::BrowsePlayingTrackAlbum => "browse to the current playing track's album page",
            Self::BrowseSelectedTrackArtists => {
                "open a popup for browsing current selected track's artists"
            }
            Self::BrowseSelectedTrackAlbum => "browse to the current selected track's album page",
            Self::SortTrackByTitle => "sort the track table (if any) by track's title",
            Self::SortTrackByArtists => "sort the track table (if any) by track's artists",
            Self::SortTrackByAlbum => "sort the track table (if any) by track's album",
            Self::SortTrackByDuration => "sort the track table (if any) by track's duration",
            Self::SortTrackByAddedDate => "sort the track table (if any) by track's added date",
            Self::ReverseTrackOrder => "reverse the order of the track table (if any)",
        }
    }
}
