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

    SearchContextTracks,

    PreviousFrame,
    BrowseUserPlaylist,
    BrowsePlayingContext,
    BrowsePlayingTrackAlbum,
    BrowsePlayingTrackArtist,
    BrowseSelectedTrackAlbum,
    BrowseSelectedTrackArtist,

    SortByTrack,
    SortByArtists,
    SortByAlbum,
    SortByDuration,
    SortByAddedDate,
    ReverseOrder,
}
