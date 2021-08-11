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
    pub popup_state: PopupState,

    pub progress_bar_rect: tui::layout::Rect,

    pub context_tracks_table_ui_state: TableState,
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
            popup_state: PopupState::None,

            progress_bar_rect: tui::layout::Rect::default(),

            context_tracks_table_ui_state: TableState::default(),
            playlists_list_ui_state: ListState::default(),
            artists_list_ui_state: ListState::default(),
            themes_list_ui_state: ListState::default(),
            devices_list_ui_state: ListState::default(),
            shortcuts_help_ui_state: false,
        }
    }
}
