mod data;
mod consant;
mod model;
mod player;
mod ui;

pub use consant::*;
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
#[derive(Debug)]
pub struct State {
    pub app_config: config::AppConfig,
    pub keymap_config: config::KeymapConfig,
    pub theme_config: config::ThemeConfig,

    pub ui: Mutex<UIState>,
    pub player: RwLock<PlayerState>,
    pub data: RwLock<AppData>,
}

impl State {
    /// parses application's configurations
    pub fn parse_config_files(
        &mut self,
        config_folder: &std::path::Path,
        theme: Option<&String>,
    ) -> Result<()> {
        self.app_config.parse_config_file(config_folder)?;
        if let Some(theme) = theme {
            self.app_config.theme = theme.to_owned();
        };
        tracing::info!("General configurations: {:?}", self.app_config);

        self.theme_config.parse_config_file(config_folder)?;
        tracing::info!("Theme configurations: {:?}", self.theme_config);

        self.keymap_config.parse_config_file(config_folder)?;
        tracing::info!("Keymap configurations: {:?}", self.keymap_config);

        if let Some(theme) = self.theme_config.find_theme(&self.app_config.theme) {
            // update the UI theme based on the `theme` config option
            // specified in the app's general configurations
            self.ui.lock().theme = theme;
        }

        Ok(())
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            app_config: config::AppConfig::default(),
            theme_config: config::ThemeConfig::default(),
            keymap_config: config::KeymapConfig::default(),

            ui: Mutex::new(UIState::default()),
            player: RwLock::new(PlayerState::default()),
            data: RwLock::new(AppData::default()),
        }
    }
}
