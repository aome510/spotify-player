use super::model::*;
use crate::{config, key, utils};

use tui::widgets::*;

pub type UIStateGuard<'a> = parking_lot::MutexGuard<'a, UIState>;

/// Application's UI state which consists of multiple smaller, separate states
/// (window, page, popup, current input, etc).
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,

    pub history: Vec<PageState>,
    pub popup: Option<PopupState>,
    pub window: WindowState,

    // the rectangle representing the player's progress bar position,
    // which is mainly used to handle mouse click events (for track seeking)
    pub progress_bar_rect: tui::layout::Rect,
}

/// A state representation of a UI page.
#[derive(Clone, Debug)]
pub enum PageState {
    Library,
    Context(Option<ContextId>, ContextPageType),
    Searching {
        input: String,
        current_query: String,
    },
    Recommendations(SeedItem),
}

#[derive(Clone, Debug)]
pub enum ContextPageType {
    CurrentPlaying,
    Browsing(ContextId),
}

/// A state representation of a UI window.
///
/// A window is a component of a page which can consist of multiple sub-windows.
/// A window with more than one sub-window will need to have a variable representing
/// the focusing state/the currently focused sub-window of that window.
#[derive(Debug)]
pub enum WindowState {
    Unknown,
    Library {
        playlist_list: ListState,
        saved_album_list: ListState,
        followed_artist_list: ListState,
        focus: LibraryFocusState,
    },
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
    Search {
        track_list: ListState,
        album_list: ListState,
        artist_list: ListState,
        playlist_list: ListState,
        focus: SearchFocusState,
    },
    Recommendations {
        track_table: TableState,
    },
}

/// A state representation of a UI popup.
///
/// A popup is often used to represent a temporary interaction
/// with the application to make a request or get additional information.
#[derive(Debug)]
pub enum PopupState {
    CommandHelp { offset: usize },
    Search { query: String },
    UserPlaylistList(PlaylistPopupAction, ListState),
    UserFollowedArtistList(ListState),
    UserSavedAlbumList(ListState),
    DeviceList(ListState),
    ArtistList(Vec<Artist>, ListState),
    ThemeList(Vec<config::Theme>, ListState),
    ActionList(Item, ListState),
}

/// An action on a playlist popup list
#[derive(Debug)]
pub enum PlaylistPopupAction {
    Browse,
    AddTrack(TrackId),
}

/// A trait representing a focusable component
pub trait Focusable {
    fn next(&mut self);
    fn previous(&mut self);
}

/// Library page focus state
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LibraryFocusState {
    Playlists,
    SavedAlbums,
    FollowedArtists,
}

/// Artist page focus state
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArtistFocusState {
    TopTracks,
    Albums,
    RelatedArtists,
}

/// Search page focus state
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SearchFocusState {
    Input,
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl UIState {
    pub fn current_page(&self) -> &PageState {
        self.history.last().expect("History must not be empty")
    }

    pub fn current_page_mut(&mut self) -> &mut PageState {
        self.history.last_mut().expect("History must not be empty")
    }

    pub fn create_new_page(&mut self, page: PageState) {
        self.history.push(page);
        self.popup = None;
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: config::Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },

            history: vec![PageState::Library],
            popup: None,
            window: WindowState::Unknown,

            progress_bar_rect: tui::layout::Rect::default(),
        }
    }
}

impl PageState {
    /// The context URI of the current page.
    /// Returns `None` if the current page is not a context page.
    pub fn context_uri(&self) -> Option<String> {
        match self {
            Self::Context(context_id, _) => context_id.as_ref().map(|id| id.uri()),
            _ => None,
        }
    }
}

impl WindowState {
    /// creates a new window search state
    pub fn new_search_state() -> Self {
        Self::Search {
            track_list: utils::new_list_state(),
            album_list: utils::new_list_state(),
            artist_list: utils::new_list_state(),
            playlist_list: utils::new_list_state(),
            focus: SearchFocusState::Input,
        }
    }
}

impl PopupState {
    /// gets the (immutable) list state of a (list) popup
    pub fn list_state(&self) -> Option<&ListState> {
        match self {
            Self::DeviceList(list_state) => Some(list_state),
            Self::UserPlaylistList(.., list_state) => Some(list_state),
            Self::UserFollowedArtistList(list_state) => Some(list_state),
            Self::UserSavedAlbumList(list_state) => Some(list_state),
            Self::ArtistList(.., list_state) => Some(list_state),
            Self::ThemeList(.., list_state) => Some(list_state),
            Self::ActionList(.., list_state) => Some(list_state),
            Self::CommandHelp { .. } | Self::Search { .. } => None,
        }
    }

    /// gets the (mutable) list state of a (list) popup
    pub fn list_state_mut(&mut self) -> Option<&mut ListState> {
        match self {
            Self::DeviceList(list_state) => Some(list_state),
            Self::UserPlaylistList(.., list_state) => Some(list_state),
            Self::UserFollowedArtistList(list_state) => Some(list_state),
            Self::UserSavedAlbumList(list_state) => Some(list_state),
            Self::ArtistList(.., list_state) => Some(list_state),
            Self::ThemeList(.., list_state) => Some(list_state),
            Self::ActionList(.., list_state) => Some(list_state),
            Self::CommandHelp { .. } | Self::Search { .. } => None,
        }
    }

    /// gets the selected position of a (list) popup
    pub fn list_selected(&self) -> Option<usize> {
        match self.list_state() {
            None => None,
            Some(state) => state.selected(),
        }
    }

    /// selects a position in a (list) popup
    pub fn list_select(&mut self, id: Option<usize>) {
        match self.list_state_mut() {
            None => {}
            Some(state) => state.select(id),
        }
    }
}

impl WindowState {
    /// gets the state of the track table
    pub fn track_table_state(&mut self) -> Option<&mut TableState> {
        match self {
            Self::Playlist { track_table } => Some(track_table),
            Self::Album { track_table } => Some(track_table),
            Self::Artist {
                top_track_table, ..
            } => Some(top_track_table),
            Self::Recommendations { track_table } => Some(track_table),
            _ => None,
        }
    }

    /// selects a position in the currently focused list/table of the window
    pub fn select(&mut self, id: Option<usize>) {
        match self {
            Self::Unknown => {}
            Self::Library {
                playlist_list,
                saved_album_list,
                followed_artist_list,
                focus,
            } => match focus {
                LibraryFocusState::Playlists => playlist_list.select(id),
                LibraryFocusState::SavedAlbums => saved_album_list.select(id),
                LibraryFocusState::FollowedArtists => followed_artist_list.select(id),
            },
            Self::Search {
                track_list,
                album_list,
                artist_list,
                playlist_list,
                focus,
            } => match focus {
                SearchFocusState::Input => {}
                SearchFocusState::Tracks => track_list.select(id),
                SearchFocusState::Albums => album_list.select(id),
                SearchFocusState::Artists => artist_list.select(id),
                SearchFocusState::Playlists => playlist_list.select(id),
            },
            Self::Playlist { track_table } => track_table.select(id),
            Self::Album { track_table } => track_table.select(id),
            Self::Artist {
                top_track_table,
                album_list,
                related_artist_list,
                focus,
            } => match focus {
                ArtistFocusState::TopTracks => top_track_table.select(id),
                ArtistFocusState::Albums => album_list.select(id),
                ArtistFocusState::RelatedArtists => related_artist_list.select(id),
            },
            Self::Recommendations { track_table } => track_table.select(id),
        }
    }

    /// gets the selected position in the currently focused list/table of the window
    pub fn selected(&self) -> Option<usize> {
        match self {
            Self::Unknown => None,
            Self::Library {
                playlist_list,
                saved_album_list,
                followed_artist_list,
                focus,
            } => match focus {
                LibraryFocusState::Playlists => playlist_list.selected(),
                LibraryFocusState::SavedAlbums => saved_album_list.selected(),
                LibraryFocusState::FollowedArtists => followed_artist_list.selected(),
            },
            Self::Search {
                track_list,
                album_list,
                artist_list,
                playlist_list,
                focus,
            } => match focus {
                SearchFocusState::Input => None,
                SearchFocusState::Tracks => track_list.selected(),
                SearchFocusState::Albums => album_list.selected(),
                SearchFocusState::Artists => artist_list.selected(),
                SearchFocusState::Playlists => playlist_list.selected(),
            },
            Self::Playlist { track_table } => track_table.selected(),
            Self::Album { track_table } => track_table.selected(),
            Self::Artist {
                top_track_table,
                album_list,
                related_artist_list,
                focus,
            } => match focus {
                ArtistFocusState::TopTracks => top_track_table.selected(),
                ArtistFocusState::Albums => album_list.selected(),
                ArtistFocusState::RelatedArtists => related_artist_list.selected(),
            },
            Self::Recommendations { track_table } => track_table.selected(),
        }
    }
}

impl Focusable for WindowState {
    fn next(&mut self) {
        match self {
            Self::Artist { focus, .. } => focus.next(),
            Self::Search { focus, .. } => focus.next(),
            Self::Library { focus, .. } => focus.next(),
            _ => {}
        }
        self.select(Some(0));
    }

    fn previous(&mut self) {
        match self {
            Self::Artist { focus, .. } => focus.previous(),
            Self::Search { focus, .. } => focus.previous(),
            Self::Library { focus, .. } => focus.previous(),
            _ => {}
        }
        self.select(Some(0));
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
