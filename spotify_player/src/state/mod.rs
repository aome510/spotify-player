mod constant;
mod data;
mod model;
mod player;
mod ui;

pub use constant::*;
pub use data::*;
pub use model::*;
pub use player::*;
pub use ui::*;

use crate::config;
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
    /// creates an application's state based on files in a configuration folder and an optional pre-defined theme
    pub fn new(
        config_folder: &std::path::Path,
        cache_folder: &std::path::Path,
        theme: Option<&String>,
    ) -> Result<Self> {
        let state = Self {
            app_config: config::AppConfig::new(config_folder, theme)?,
            keymap_config: config::KeymapConfig::new(config_folder)?,
            theme_config: config::ThemeConfig::new(config_folder)?,
            cache_folder: cache_folder.to_path_buf(),
            ui: Mutex::new(UIState::default()),
            player: RwLock::new(PlayerState::default()),
            data: RwLock::new(AppData::default()),
        };

        if let Some(theme) = state.theme_config.find_theme(&state.app_config.theme) {
            // update the UI theme based on the `theme` config option
            // specified in the app's general configurations
            state.ui.lock().theme = theme;
        }

        Ok(state)
    }
}
