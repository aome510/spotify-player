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

    pub frame: FrameState,
    pub frame_history: Vec<FrameState>,
    pub popup_state: PopupState,
    pub context: ContextState,

    pub progress_bar_rect: tui::layout::Rect,

    // TODO: should wrap the below popup list states inside the popup_state
    pub playlists_list_ui_state: ListState,
    pub artists_list_ui_state: ListState,
    pub themes_list_ui_state: ListState,
    pub devices_list_ui_state: ListState,
    // TODO: find out if this is needed
    pub shortcuts_help_ui_state: bool,
}

/// Context state
#[derive(Debug)]
pub enum ContextState {
    Unknown,
    // tracks
    Playlist(TableState),
    // tracks
    Album(TableState),
    // top tracks, albums, related artists
    Artist(TableState, ListState, ListState, ArtistFocusState),
}

/// Frame state
#[derive(Clone, Debug)]
pub enum FrameState {
    Default,
    Browse(String),
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

/// Popup state
#[derive(Debug)]
pub enum PopupState {
    None,
    CommandHelp,
    ContextSearch(String),
    PlaylistList,
    DeviceList,
    ArtistList(Vec<Artist>),
    ThemeList(Vec<config::Theme>),
}

impl UIState {
    /// gets all tracks inside the current playing context.
    /// If in the context search mode, returns tracks filtered by the search query.
    pub fn get_context_tracks<'a>(
        &'a self,
        player: &'a std::sync::RwLockReadGuard<'a, PlayerState>,
    ) -> Vec<&'a Track> {
        match self.popup_state {
            PopupState::ContextSearch(ref query) => player
                .context
                .get_tracks()
                .map(|tracks| {
                    tracks
                        .iter()
                        .filter(|t| t.get_basic_info().to_lowercase().contains(query))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            _ => player
                .context
                .get_tracks()
                .map(|tracks| tracks.iter().collect::<Vec<_>>())
                .unwrap_or_default(),
        }
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_running: true,
            theme: config::Theme::default(),
            input_key_sequence: key::KeySequence { keys: vec![] },

            frame: FrameState::Default,
            frame_history: vec![FrameState::Default],
            popup_state: PopupState::None,
            context: ContextState::Unknown,

            progress_bar_rect: tui::layout::Rect::default(),

            playlists_list_ui_state: ListState::default(),
            artists_list_ui_state: ListState::default(),
            themes_list_ui_state: ListState::default(),
            devices_list_ui_state: ListState::default(),
            shortcuts_help_ui_state: false,
        }
    }
}

impl ContextState {
    pub fn get_track_table_state(&mut self) -> Option<&mut TableState> {
        match self {
            Self::Unknown => None,
            Self::Playlist(ref mut state) => Some(state),
            Self::Album(ref mut state) => Some(state),
            Self::Artist(ref mut top_tracks, _, _, _) => Some(top_tracks),
        }
    }

    pub fn select(&mut self, id: Option<usize>) {
        match self {
            Self::Unknown => {}
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

    pub fn selected(&self) -> Option<usize> {
        match self {
            Self::Unknown => None,
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

impl Focusable for ContextState {
    fn next(&mut self) {
        if let Self::Artist(_, _, _, artist) = self {
            artist.next()
        };
    }

    fn previous(&mut self) {
        if let Self::Artist(_, _, _, artist) = self {
            artist.previous()
        };
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
