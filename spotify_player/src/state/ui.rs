use super::player::*;
use crate::{config, key};

use tui::widgets::{ListState, TableState};
pub type UIStateGuard<'a> = std::sync::MutexGuard<'a, UIState>;

/// UI state
#[derive(Debug)]
pub struct UIState {
    pub is_running: bool,
    pub theme: config::Theme,
    pub input_key_sequence: key::KeySequence,

    pub frame_state: FrameState,
    pub frame_history: Vec<FrameState>,
    pub focus_state: FocusState,
    pub popup_state: PopupState,

    pub progress_bar_rect: tui::layout::Rect,

    pub context_tracks_table_ui_state: TableState,
    // TODO: should wrap this list state inside a context ui state
    pub artist_albums_list_ui_state: ListState,

    // TODO: should wrap the below popup list states inside the popup_state
    pub playlists_list_ui_state: ListState,
    pub artists_list_ui_state: ListState,
    pub themes_list_ui_state: ListState,
    pub devices_list_ui_state: ListState,
    pub shortcuts_help_ui_state: bool,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtistFocusState {
    TopTracks,
    Albums,
    RelatedArtists,
}

/// Focus state
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusState {
    Artist(ArtistFocusState),
    Default,
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

            frame_state: FrameState::Default,
            frame_history: vec![FrameState::Default],
            focus_state: FocusState::Default,
            popup_state: PopupState::None,

            progress_bar_rect: tui::layout::Rect::default(),

            context_tracks_table_ui_state: TableState::default(),
            artist_albums_list_ui_state: ListState::default(),

            playlists_list_ui_state: ListState::default(),
            artists_list_ui_state: ListState::default(),
            themes_list_ui_state: ListState::default(),
            devices_list_ui_state: ListState::default(),
            shortcuts_help_ui_state: false,
        }
    }
}

impl Focusable for FocusState {
    fn next(&mut self) {
        match self {
            Self::Default => {}
            Self::Artist(artist) => artist.next(),
        };
    }

    fn previous(&mut self) {
        match self {
            Self::Default => {}
            Self::Artist(artist) => artist.previous(),
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
