mod consant;
mod data;
mod model;
mod player;
mod ui;

pub use consant::*;
pub use data::*;
pub use model::*;
pub use player::*;
pub use ui::*;

use crate::config::{self};
use anyhow::Result;

pub use parking_lot::{Mutex, RwLock};

/// Application's shared state (wrapped inside an std::sync::Arc)
pub type SharedState = std::sync::Arc<State>;

/// Application's state
pub struct State {
    pub app_config: config::AppConfig,
    pub keymap_config: config::KeymapConfig,
    pub theme_config: config::ThemeConfig,

    pub cache_folder: std::path::PathBuf,

    pub ui: Mutex<UIState>,
    pub player: RwLock<PlayerState>,
    pub data: RwLock<AppData>,
}

impl State {
    /// parses application's configurations
    pub fn new(config_folder: &std::path::Path, theme: Option<&String>) -> Result<Self> {
        let mut state = Self {
            app_config: config::AppConfig::new(config_folder, theme)?,
            keymap_config: config::KeymapConfig::default(),
            theme_config: config::ThemeConfig::default(),
            cache_folder: std::path::PathBuf::new(),
            ui: Mutex::new(UIState::default()),
            player: RwLock::new(PlayerState::default()),
            data: RwLock::new(AppData::default()),
        };

        state.theme_config.parse_config_file(config_folder)?;
        state.keymap_config.parse_config_file(config_folder)?;

        if let Some(theme) = state.theme_config.find_theme(&state.app_config.theme) {
            // update the UI theme based on the `theme` config option
            // specified in the app's general configurations
            state.ui.lock().theme = theme;
        }

        Ok(state)
    }
}
