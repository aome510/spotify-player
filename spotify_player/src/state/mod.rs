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

#[derive(Debug)]
pub struct Configs {
    pub app_config: config::AppConfig,
    pub keymap_config: config::KeymapConfig,
    pub theme_config: config::ThemeConfig,
    pub cache_folder: std::path::PathBuf,
    pub config_folder: std::path::PathBuf,
}

impl Configs {
    pub fn new(config_folder: &std::path::Path, cache_folder: &std::path::Path) -> Result<Self> {
        Ok(Self {
            app_config: config::AppConfig::new(config_folder)?,
            keymap_config: config::KeymapConfig::new(config_folder)?,
            theme_config: config::ThemeConfig::new(config_folder)?,
            cache_folder: cache_folder.to_path_buf(),
            config_folder: config_folder.to_path_buf(),
        })
    }
}

/// Application's state
pub struct State {
    pub configs: Configs,
    pub ui: Mutex<UIState>,
    pub player: RwLock<PlayerState>,
    pub data: RwLock<AppData>,
}

impl State {
    pub fn new(configs: Configs) -> Result<Self> {
        let mut ui = UIState::default();

        if let Some(theme) = configs.theme_config.find_theme(&configs.app_config.theme) {
            // update the UI's theme based on the `theme` config option
            ui.theme = theme;
        }

        let app_data = AppData::new(&configs.cache_folder)?;

        Ok(Self {
            configs,
            ui: Mutex::new(ui),
            player: RwLock::new(PlayerState::default()),
            data: RwLock::new(app_data),
        })
    }
}
