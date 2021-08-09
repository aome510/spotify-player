use serde::Deserialize;

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
/// Application's command
pub enum Command {
    NextTrack,
    PreviousTrack,
    ResumePause,
    Repeat,
    Shuffle,

    Quit,
    OpenCommandHelp,
    ClosePopup,

    SelectNext,
    SelectPrevious,
    ChooseSelected,

    RefreshPlayback,

    SwitchTheme,
    SwitchDevice,

    SearchContextTracks,

    PreviousFrame,
    BrowseUserPlaylist,
    BrowseSelectedTrackAlbum,

    SortByTrack,
    SortByArtists,
    SortByAlbum,
    SortByDuration,
    SortByAddedDate,
    ReverseOrder,
}
