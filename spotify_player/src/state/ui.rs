use super::model::*;
use crate::{config, key, utils};

use tui::widgets::*;

pub type UIStateGuard<'a> = parking_lot::MutexGuard<'a, UIState>;

/// Application's UI state
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,

    pub history: Vec<PageState>,
    pub popup: Option<PopupState>,

    // the rectangle representing the player's progress bar position,
    // which is mainly used to handle mouse click events (for track seeking)
    pub progress_bar_rect: tui::layout::Rect,
}

/// A state representation of a UI page.
#[derive(Clone, Debug)]
pub enum PageState {
    Library {
        state: LibraryPageUIState,
    },
    Context {
        id: Option<ContextId>,
        context_page_type: ContextPageType,
        state: ContextPageUIState,
    },
    Search {
        input: String,
        current_query: String,
        state: SearchPageUIState,
    },
    Tracks {
        title: String,
        desc: String,
        state: ListState,
    },
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
    Browsing,
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

pub enum MutableWindowState<'a> {
    Table(&'a mut TableState),
    List(&'a mut ListState),
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

            history: vec![PageState::new_library()],
            popup: None,

            progress_bar_rect: tui::layout::Rect::default(),
        }
    }
}

impl PageState {
    /// The context URI of the current page.
    /// Returns `None` if the current page is not a context page.
    pub fn context_uri(&self) -> Option<String> {
        match self {
            Self::Context { id, .. } => id.as_ref().map(|id| id.uri()),
            _ => None,
        }
    }

    pub fn new_library() -> PageState {
        PageState::Library {
            playlist_list: utils::new_list_state(),
            saved_album_list: utils::new_list_state(),
            followed_artist_list: utils::new_list_state(),
            focus: LibraryFocusState::Playlists,
        }
    }

    pub fn new_search() -> PageState {
        PageState::Search {
            input: String::new(),
            current_query: String::new(),
            track_list: utils::new_list_state(),
            album_list: utils::new_list_state(),
            artist_list: utils::new_list_state(),
            playlist_list: utils::new_list_state(),
            focus: SearchFocusState::Input,
        }
    }

    /// focus a window in the current page
    pub fn focus_window_state_mut<'a>(&'a mut self) -> Option<MutableWindowState<'a>> {
        match self {
            Self::Library {
                playlist_list,
                saved_album_list,
                followed_artist_list,
                focus,
            } => match focus {
                LibraryFocusState::Playlists => Some(MutableWindowState::List(saved_album_list)),
                LibraryFocusState::SavedAlbums => Some(MutableWindowState::List(saved_album_list)),
                LibraryFocusState::FollowedArtists => {
                    Some(MutableWindowState::List(followed_artist_list))
                }
            },
            Self::Search {
                track_list,
                album_list,
                artist_list,
                playlist_list,
                focus,
                ..
            } => match focus {
                SearchFocusState::Input => None,
                SearchFocusState::Tracks => Some(MutableWindowState::List(track_list)),
                SearchFocusState::Albums => Some(MutableWindowState::List(album_list)),
                SearchFocusState::Artists => Some(MutableWindowState::List(artist_list)),
                SearchFocusState::Playlists => Some(MutableWindowState::List(playlist_list)),
            },
            // Self::Playlist { track_table } => track_table.select(id),
            // Self::Album { track_table } => track_table.select(id),
            // Self::Artist {
            //     top_track_table,
            //     album_list,
            //     related_artist_list,
            //     focus,
            // } => match focus {
            //     ArtistFocusState::TopTracks => top_track_table.select(id),
            //     ArtistFocusState::Albums => album_list.select(id),
            //     ArtistFocusState::RelatedArtists => related_artist_list.select(id),
            // },
            // TODO: handle this!
            _ => unreachable!(),
        }
    }
}

impl<'a> MutableWindowState<'a> {
    pub fn select(&self, id: usize) {
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

pub trait Focusable {
    fn next(&mut self);
    fn previous(&mut self);
}

impl Focusable for PageState {
    fn next(&mut self) {
        match self {
            Self::Search { focus, .. } => focus.next(),
            Self::Library { focus, .. } => focus.next(),
            // TODO: handle this!
            _ => {}
        }

        // reset the list/table state of the focus window
        self.focus_window_state_mut().map(|state| state.select(0));
    }

    fn previous(&mut self) {
        match self {
            Self::Search { focus, .. } => focus.previous(),
            Self::Library { focus, .. } => focus.previous(),
            // TODO: handle this!
            _ => {}
        }

        // reset the list/table state of the focus window
        self.focus_window_state_mut().map(|state| state.select(0));
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
