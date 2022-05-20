use crate::{state::model::*, utils};
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
        input: String,
        current_query: String,
        state: SearchPageUIState,
    },
    Tracks {
        id: String,
        title: String,
        desc: String,
        state: TableState,
    },
    #[cfg(feature = "lyric-finder")]
    Lyric {
        track: String,
        artists: String,
        scroll_offset: usize,
    },
}

pub enum PageType {
    Library,
    Context,
    Search,
    Tracks,
    #[cfg(feature = "lyric-finder")]
    Lyric,
}

#[derive(Clone, Debug)]
pub struct LibraryPageUIState {
    pub playlist_list: ListState,
    pub saved_album_list: ListState,
    pub followed_artist_list: ListState,
    pub focus: LibraryFocusState,
}

#[derive(Clone, Debug)]
pub struct SearchPageUIState {
    pub track_list: ListState,
    pub album_list: ListState,
    pub artist_list: ListState,
    pub playlist_list: ListState,
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
        album_list: ListState,
        related_artist_list: ListState,
        focus: ArtistFocusState,
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
}

pub enum MutableWindowState<'a> {
    Table(&'a mut TableState),
    List(&'a mut ListState),
}

impl PageState {
    /// The type of the page.
    pub fn page_type(&self) -> PageType {
        match self {
            PageState::Library { .. } => PageType::Library,
            PageState::Context { .. } => PageType::Context,
            PageState::Search { .. } => PageType::Search,
            PageState::Tracks { .. } => PageType::Tracks,
            #[cfg(feature = "lyric-finder")]
            PageState::Lyric { .. } => PageType::Lyric,
        }
    }

    /// Select a `id`-th item in the currently focused window of the page.
    pub fn select(&mut self, id: usize) {
        if let Some(mut state) = self.focus_window_state_mut() {
            state.select(id)
        }
    }

    /// The selected item's position in the currently focused window of the page.
    pub fn selected(&mut self) -> Option<usize> {
        self.focus_window_state_mut()
            .map(|state| state.selected())?
    }

    /// The currently focused window state of the page.
    fn focus_window_state_mut(&mut self) -> Option<MutableWindowState> {
        match self {
            Self::Library {
                state:
                    LibraryPageUIState {
                        playlist_list,
                        saved_album_list,
                        followed_artist_list,
                        focus,
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
                        focus,
                    },
                ..
            } => match focus {
                SearchFocusState::Input => None,
                SearchFocusState::Tracks => Some(MutableWindowState::List(track_list)),
                SearchFocusState::Albums => Some(MutableWindowState::List(album_list)),
                SearchFocusState::Artists => Some(MutableWindowState::List(artist_list)),
                SearchFocusState::Playlists => Some(MutableWindowState::List(playlist_list)),
            },
            Self::Context { state, .. } => state.as_mut().map(|state| match state {
                ContextPageUIState::Playlist { track_table } => {
                    MutableWindowState::Table(track_table)
                }
                ContextPageUIState::Album { track_table } => MutableWindowState::Table(track_table),
                ContextPageUIState::Artist {
                    top_track_table,
                    album_list,
                    related_artist_list,
                    focus,
                } => match focus {
                    ArtistFocusState::TopTracks => MutableWindowState::Table(top_track_table),
                    ArtistFocusState::Albums => MutableWindowState::List(album_list),
                    ArtistFocusState::RelatedArtists => {
                        MutableWindowState::List(related_artist_list)
                    }
                },
            }),
            Self::Tracks { state, .. } => Some(MutableWindowState::Table(state)),
            #[cfg(feature = "lyric-finder")]
            Self::Lyric { .. } => None,
        }
    }
}

impl LibraryPageUIState {
    pub fn new() -> Self {
        Self {
            playlist_list: utils::new_list_state(),
            saved_album_list: utils::new_list_state(),
            followed_artist_list: utils::new_list_state(),
            focus: LibraryFocusState::Playlists,
        }
    }
}

impl SearchPageUIState {
    pub fn new() -> Self {
        Self {
            track_list: utils::new_list_state(),
            album_list: utils::new_list_state(),
            artist_list: utils::new_list_state(),
            playlist_list: utils::new_list_state(),
            focus: SearchFocusState::Input,
        }
    }
}

impl ContextPageUIState {
    pub fn new_playlist() -> Self {
        Self::Playlist {
            track_table: utils::new_table_state(),
        }
    }

    pub fn new_album() -> Self {
        Self::Album {
            track_table: utils::new_table_state(),
        }
    }

    pub fn new_artist() -> Self {
        Self::Artist {
            top_track_table: utils::new_table_state(),
            album_list: utils::new_list_state(),
            related_artist_list: utils::new_list_state(),
            focus: ArtistFocusState::TopTracks,
        }
    }
}

impl<'a> MutableWindowState<'a> {
    pub fn select(&mut self, id: usize) {
        match self {
            Self::List(state) => state.select(Some(id)),
            Self::Table(state) => state.select(Some(id)),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        match self {
            Self::List(state) => state.selected(),
            Self::Table(state) => state.selected(),
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
            state.select(0)
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
            state.select(0)
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
    [Playlists, Input]
);
