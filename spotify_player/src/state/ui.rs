use super::player::*;
use crate::{config, key};

use tui::widgets::{ListState, TableState};
pub type UIStateGuard<'a> = std::sync::MutexGuard<'a, UIState>;

// TODO: improve the documentation for UI states' struct

/// UI state
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,

    pub history: Vec<PageState>,
    pub popup: PopupState,
    pub window: WindowState,

    pub progress_bar_rect: tui::layout::Rect,
}

/// Page state
#[derive(Clone, Debug)]
pub enum PageState {
    CurrentPlaying,
    Browsing(String),
    Searching(String, Box<SearchResults>),
}

/// Window state
#[derive(Debug)]
pub enum WindowState {
    Unknown,
    /// tracks
    Playlist(TableState),
    /// tracks
    Album(TableState),
    /// top tracks, albums, related artists
    Artist(TableState, ListState, ListState, ArtistFocusState),
    /// tracks, albums, artists, playlists
    Search(ListState, ListState, ListState, ListState, SearchFocusState),
}

/// Popup state
#[derive(Debug)]
pub enum PopupState {
    None,
    CommandHelp(usize),
    ContextSearch(String),
    UserPlaylistList(ListState),
    UserFollowedArtistList(ListState),
    UserSavedAlbumList(ListState),
    DeviceList(ListState),
    ArtistList(Vec<Artist>, ListState),
    ThemeList(Vec<config::Theme>, ListState),
}

/// A trait representing a focusable state
pub trait Focusable {
    fn next(&mut self);
    fn previous(&mut self);
}

/// Artist Focus state
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArtistFocusState {
    TopTracks,
    Albums,
    RelatedArtists,
}

/// Search Focus state
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SearchFocusState {
    Input,
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl UIState {
    fn query_match(s: &str, query: &str) -> bool {
        query
            .split(' ')
            .fold(true, |acc, cur| acc & s.contains(cur))
    }

    pub fn current_page(&self) -> &PageState {
        self.history.last().expect("History must not be empty")
    }

    pub fn current_page_mut(&mut self) -> &mut PageState {
        self.history.last_mut().expect("History must not be empty")
    }

    /// gets a list of items possibly filtered by a search query if currently inside a search state
    pub fn get_search_filtered_items<'a, T: std::fmt::Display>(
        &self,
        items: &'a [T],
    ) -> Vec<&'a T> {
        match self.popup {
            PopupState::ContextSearch(ref query) => items
                .iter()
                .filter(|t| Self::query_match(&t.to_string().to_lowercase(), &query.to_lowercase()))
                .collect::<Vec<_>>(),
            _ => items.iter().collect::<Vec<_>>(),
        }
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: config::Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },

            history: vec![PageState::CurrentPlaying],
            popup: PopupState::None,
            window: WindowState::Unknown,

            progress_bar_rect: tui::layout::Rect::default(),
        }
    }
}

impl PopupState {
    /// gets the state of the current list popup
    pub fn get_list_state(&self) -> Option<&ListState> {
        match self {
            Self::DeviceList(ref state) => Some(state),
            Self::UserPlaylistList(ref state) => Some(state),
            Self::UserFollowedArtistList(ref state) => Some(state),
            Self::UserSavedAlbumList(ref state) => Some(state),
            Self::ArtistList(_, ref state) => Some(state),
            Self::ThemeList(_, ref state) => Some(state),
            Self::CommandHelp(_) | Self::None | Self::ContextSearch(_) => None,
        }
    }

    /// gets the (mutable) state of the current list popup
    pub fn get_list_state_mut(&mut self) -> Option<&mut ListState> {
        match self {
            Self::DeviceList(ref mut state) => Some(state),
            Self::UserPlaylistList(ref mut state) => Some(state),
            Self::UserFollowedArtistList(ref mut state) => Some(state),
            Self::UserSavedAlbumList(ref mut state) => Some(state),
            Self::ArtistList(_, ref mut state) => Some(state),
            Self::ThemeList(_, ref mut state) => Some(state),
            Self::CommandHelp(_) | Self::None | Self::ContextSearch(_) => None,
        }
    }

    /// returns the selected position in the current list popup
    pub fn list_selected(&self) -> Option<usize> {
        match self.get_list_state() {
            None => None,
            Some(state) => state.selected(),
        }
    }

    /// selects a position in the current list popup
    pub fn list_select(&mut self, id: Option<usize>) {
        match self.get_list_state_mut() {
            None => {}
            Some(state) => state.select(id),
        }
    }
}

impl WindowState {
    /// gets the state of the context track table
    pub fn get_track_table_state(&mut self) -> Option<&mut TableState> {
        match self {
            Self::Playlist(ref mut state) => Some(state),
            Self::Album(ref mut state) => Some(state),
            Self::Artist(ref mut top_tracks, _, _, _) => Some(top_tracks),
            _ => None,
        }
    }

    /// selects a position in the context track table
    pub fn select(&mut self, id: Option<usize>) {
        match self {
            Self::Unknown => {}
            Self::Search(
                ref mut tracks,
                ref mut albums,
                ref mut artists,
                ref mut playlists,
                ref focus,
            ) => match focus {
                SearchFocusState::Input => {}
                SearchFocusState::Tracks => tracks.select(id),
                SearchFocusState::Albums => albums.select(id),
                SearchFocusState::Artists => artists.select(id),
                SearchFocusState::Playlists => playlists.select(id),
            },
            Self::Playlist(ref mut state) => state.select(id),
            Self::Album(ref mut state) => state.select(id),
            Self::Artist(
                ref mut top_tracks,
                ref mut albums,
                ref mut related_artists,
                ref focus,
            ) => match focus {
                ArtistFocusState::TopTracks => top_tracks.select(id),
                ArtistFocusState::Albums => albums.select(id),
                ArtistFocusState::RelatedArtists => related_artists.select(id),
            },
        }
    }

    /// gets the selected position in the context track table
    pub fn selected(&self) -> Option<usize> {
        match self {
            Self::Unknown => None,
            Self::Search(ref tracks, ref albums, ref artists, ref playlists, ref focus) => {
                match focus {
                    &SearchFocusState::Input => None,
                    SearchFocusState::Tracks => tracks.selected(),
                    SearchFocusState::Albums => albums.selected(),
                    SearchFocusState::Artists => artists.selected(),
                    SearchFocusState::Playlists => playlists.selected(),
                }
            }
            Self::Playlist(ref state) => state.selected(),
            Self::Album(ref state) => state.selected(),
            Self::Artist(ref top_tracks, ref albums, ref related_artists, ref focus) => match focus
            {
                ArtistFocusState::TopTracks => top_tracks.selected(),
                ArtistFocusState::Albums => albums.selected(),
                ArtistFocusState::RelatedArtists => related_artists.selected(),
            },
        }
    }
}

impl Focusable for WindowState {
    fn next(&mut self) {
        match self {
            Self::Artist(_, _, _, focus) => focus.next(),
            Self::Search(_, _, _, _, focus) => focus.next(),
            _ => {}
        }
    }

    fn previous(&mut self) {
        match self {
            Self::Artist(_, _, _, focus) => focus.previous(),
            Self::Search(_, _, _, _, focus) => focus.previous(),
            _ => {}
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
