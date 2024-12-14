use crate::{
    state::model::{Category, ContextId},
    ui::single_line_input::LineInput,
};
use tui::widgets::{ListState, TableState};

#[derive(Clone, Debug)]
pub enum PageState {
    Library {
        state: LibraryPageUIState,
    },
    Context {
        id: Option<ContextId>,
        context_page_type: ContextPageType,
        state: Option<ContextPageUIState>,
    },
    Search {
        line_input: LineInput,
        current_query: String,
        state: SearchPageUIState,
    },
    Lyrics {
        track_uri: String,
        track: String,
        artists: String,
    },
    Browse {
        state: BrowsePageUIState,
    },
    Queue {
        scroll_offset: usize,
    },
    CommandHelp {
        scroll_offset: usize,
    },
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PageType {
    Library,
    Context,
    Search,
    Browse,
    Lyrics,
    Queue,
    CommandHelp,
}

#[derive(Clone, Debug)]
pub struct LibraryPageUIState {
    pub playlist_list: ListState,
    pub saved_album_list: ListState,
    pub followed_artist_list: ListState,
    pub focus: LibraryFocusState,
    pub playlist_folder_id: usize,
}

#[derive(Clone, Debug)]
pub struct SearchPageUIState {
    pub track_list: ListState,
    pub album_list: ListState,
    pub artist_list: ListState,
    pub playlist_list: ListState,
    pub show_list: ListState,
    pub episode_list: ListState,
    pub focus: SearchFocusState,
}

#[derive(Clone, Debug)]
pub enum ContextPageType {
    CurrentPlaying,
    Browsing(ContextId),
}

#[derive(Clone, Debug)]
pub enum ContextPageUIState {
    Playlist {
        track_table: TableState,
    },
    Album {
        track_table: TableState,
    },
    Artist {
        top_track_table: TableState,
        album_table: TableState,
        related_artist_list: ListState,
        focus: ArtistFocusState,
    },
    Tracks {
        track_table: TableState,
    },
    Show {
        episode_table: TableState,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LibraryFocusState {
    Playlists,
    SavedAlbums,
    FollowedArtists,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArtistFocusState {
    TopTracks,
    Albums,
    RelatedArtists,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SearchFocusState {
    Input,
    Tracks,
    Albums,
    Artists,
    Playlists,
    Shows,
    Episodes,
}

#[derive(Clone, Debug)]
pub enum BrowsePageUIState {
    CategoryList {
        state: ListState,
    },
    CategoryPlaylistList {
        category: Category,
        state: ListState,
    },
}

pub enum MutableWindowState<'a> {
    Table(&'a mut TableState),
    List(&'a mut ListState),
    Scroll(&'a mut usize),
}

impl PageState {
    /// The type of the page.
    pub fn page_type(&self) -> PageType {
        match self {
            PageState::Library { .. } => PageType::Library,
            PageState::Context { .. } => PageType::Context,
            PageState::Search { .. } => PageType::Search,
            PageState::Browse { .. } => PageType::Browse,
            PageState::Lyrics { .. } => PageType::Lyrics,
            PageState::Queue { .. } => PageType::Queue,
            PageState::CommandHelp { .. } => PageType::CommandHelp,
        }
    }

    /// Select a `id`-th item in the currently focused window of the page.
    pub fn select(&mut self, id: usize) {
        if let Some(mut state) = self.focus_window_state_mut() {
            state.select(id);
        }
    }

    /// The selected item's position in the currently focused window of the page.
    pub fn selected(&mut self) -> Option<usize> {
        self.focus_window_state_mut()
            .map(|state| state.selected())?
    }

    /// The currently focused window state of the page.
    pub fn focus_window_state_mut(&mut self) -> Option<MutableWindowState> {
        match self {
            Self::Library {
                state:
                    LibraryPageUIState {
                        playlist_list,
                        saved_album_list,
                        followed_artist_list,
                        focus,
                        ..
                    },
            } => Some(match focus {
                LibraryFocusState::Playlists => MutableWindowState::List(playlist_list),
                LibraryFocusState::SavedAlbums => MutableWindowState::List(saved_album_list),
                LibraryFocusState::FollowedArtists => {
                    MutableWindowState::List(followed_artist_list)
                }
            }),
            Self::Search {
                state:
                    SearchPageUIState {
                        track_list,
                        album_list,
                        artist_list,
                        playlist_list,
                        show_list,
                        episode_list,
                        focus,
                    },
                ..
            } => match focus {
                SearchFocusState::Input => None,
                SearchFocusState::Tracks => Some(MutableWindowState::List(track_list)),
                SearchFocusState::Albums => Some(MutableWindowState::List(album_list)),
                SearchFocusState::Artists => Some(MutableWindowState::List(artist_list)),
                SearchFocusState::Playlists => Some(MutableWindowState::List(playlist_list)),
                SearchFocusState::Shows => Some(MutableWindowState::List(show_list)),
                SearchFocusState::Episodes => Some(MutableWindowState::List(episode_list)),
            },
            Self::Context { state, .. } => state.as_mut().map(|state| match state {
                ContextPageUIState::Tracks { track_table }
                | ContextPageUIState::Playlist { track_table } => {
                    MutableWindowState::Table(track_table)
                }
                ContextPageUIState::Album { track_table } => MutableWindowState::Table(track_table),
                ContextPageUIState::Artist {
                    top_track_table,
                    album_table,
                    related_artist_list,
                    focus,
                } => match focus {
                    ArtistFocusState::TopTracks => MutableWindowState::Table(top_track_table),
                    ArtistFocusState::Albums => MutableWindowState::Table(album_table),
                    ArtistFocusState::RelatedArtists => {
                        MutableWindowState::List(related_artist_list)
                    }
                },
                ContextPageUIState::Show { episode_table } => {
                    MutableWindowState::Table(episode_table)
                }
            }),
            Self::Browse { state } => match state {
                BrowsePageUIState::CategoryList { state } => Some(MutableWindowState::List(state)),
                BrowsePageUIState::CategoryPlaylistList { state, .. } => {
                    Some(MutableWindowState::List(state))
                }
            },
            Self::Lyrics { .. } => None,
            Self::CommandHelp { scroll_offset } | Self::Queue { scroll_offset } => {
                Some(MutableWindowState::Scroll(scroll_offset))
            }
        }
    }
}

impl LibraryPageUIState {
    pub fn new() -> Self {
        Self {
            playlist_list: ListState::default(),
            saved_album_list: ListState::default(),
            followed_artist_list: ListState::default(),
            focus: LibraryFocusState::Playlists,
            playlist_folder_id: 0,
        }
    }
}

impl SearchPageUIState {
    pub fn new() -> Self {
        Self {
            track_list: ListState::default(),
            album_list: ListState::default(),
            artist_list: ListState::default(),
            playlist_list: ListState::default(),
            show_list: ListState::default(),
            episode_list: ListState::default(),
            focus: SearchFocusState::Input,
        }
    }
}

impl ContextPageType {
    pub fn title(&self) -> String {
        match self {
            ContextPageType::CurrentPlaying => String::from("Current Playing"),
            ContextPageType::Browsing(id) => match id {
                ContextId::Playlist(_) => String::from("Playlist"),
                ContextId::Album(_) => String::from("Album"),
                ContextId::Artist(_) => String::from("Artist"),
                ContextId::Tracks(id) => id.kind.clone(),
                ContextId::Show(_) => String::from("Show"),
            },
        }
    }
}

impl ContextPageUIState {
    pub fn new_playlist() -> Self {
        Self::Playlist {
            track_table: TableState::default(),
        }
    }

    pub fn new_album() -> Self {
        Self::Album {
            track_table: TableState::default(),
        }
    }

    pub fn new_artist() -> Self {
        Self::Artist {
            top_track_table: TableState::default(),
            album_table: TableState::default(),
            related_artist_list: ListState::default(),
            focus: ArtistFocusState::TopTracks,
        }
    }

    pub fn new_tracks() -> Self {
        Self::Tracks {
            track_table: TableState::default(),
        }
    }

    pub fn new_show() -> Self {
        Self::Show {
            episode_table: TableState::default(),
        }
    }
}

impl MutableWindowState<'_> {
    pub fn select(&mut self, id: usize) {
        match self {
            Self::List(state) => state.select(Some(id)),
            Self::Table(state) => state.select(Some(id)),
            Self::Scroll(scroll_offset) => {
                **scroll_offset = id;
            }
        }
    }

    pub fn selected(&self) -> Option<usize> {
        match self {
            Self::List(state) => state.selected(),
            Self::Table(state) => state.selected(),
            Self::Scroll(scroll_offset) => Some(**scroll_offset),
        }
    }
}

pub trait Focusable {
    fn next(&mut self);
    fn previous(&mut self);
}

impl Focusable for PageState {
    fn next(&mut self) {
        match self {
            Self::Search {
                state: SearchPageUIState { focus, .. },
                ..
            } => focus.next(),
            Self::Library {
                state: LibraryPageUIState { focus, .. },
                ..
            } => focus.next(),
            Self::Context {
                state: Some(ContextPageUIState::Artist { focus, .. }),
                ..
            } => focus.next(),
            _ => {}
        }

        // reset the list/table state of the focus window
        if let Some(mut state) = self.focus_window_state_mut() {
            state.select(0);
        }
    }

    fn previous(&mut self) {
        match self {
            Self::Search {
                state: SearchPageUIState { focus, .. },
                ..
            } => focus.previous(),
            Self::Library {
                state: LibraryPageUIState { focus, .. },
                ..
            } => focus.previous(),
            Self::Context {
                state: Some(ContextPageUIState::Artist { focus, .. }),
                ..
            } => focus.previous(),
            _ => {}
        }

        // reset the list/table state of the focus window
        if let Some(mut state) = self.focus_window_state_mut() {
            state.select(0);
        }
    }
}

macro_rules! impl_focusable {
	($struct:ty, $([$field:ident, $next_field:ident]),+) => {
		impl Focusable for $struct {
            fn next(&mut self) {
                *self = match self {
                    $(
                        Self::$field => Self::$next_field,
                    )+
                };
            }

            fn previous(&mut self) {
                *self = match self {
                    $(
                        Self::$next_field => Self::$field,
                    )+
                };
            }
        }
	};
}

impl_focusable!(
    LibraryFocusState,
    [Playlists, SavedAlbums],
    [SavedAlbums, FollowedArtists],
    [FollowedArtists, Playlists]
);

impl_focusable!(
    ArtistFocusState,
    [TopTracks, Albums],
    [Albums, RelatedArtists],
    [RelatedArtists, TopTracks]
);

impl_focusable!(
    SearchFocusState,
    [Input, Tracks],
    [Tracks, Albums],
    [Albums, Artists],
    [Artists, Playlists],
    [Playlists, Shows],
    [Shows, Episodes],
    [Episodes, Input]
);
