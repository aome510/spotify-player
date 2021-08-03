use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
    ChoseSelected,

    SearchContextTracks,

    SwitchPlaylist,
    SwitchTheme,

    SortByTrack,
    SortByArtists,
    SortByAlbum,
    SortByDuration,
    SortByAddedDate,
    ReverseOrder,
}
