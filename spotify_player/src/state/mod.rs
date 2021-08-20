mod player;
mod ui;

pub use player::*;
pub use ui::*;

use crate::config;
use anyhow::Result;
use std::sync::{Arc, Mutex, RwLock};

pub type SharedState = Arc<State>;

/// Application's state
#[derive(Debug)]
pub struct State {
    pub app_config: config::AppConfig,
    pub keymap_config: config::KeymapConfig,
    pub theme_config: config::ThemeConfig,

    pub player: RwLock<PlayerState>,
    pub ui: Mutex<UIState>,
}

impl State {
    /// gets a list of application themes with the current theme as the first element
    pub fn get_themes(&self, ui: &std::sync::MutexGuard<UIState>) -> Vec<config::Theme> {
        let mut themes = self.theme_config.themes.clone();
        let id = themes.iter().position(|t| t.name == ui.theme.name);
        if let Some(id) = id {
            let theme = themes.remove(id);
            themes.insert(0, theme);
        }
        themes
    }

    /// parses application's configurations
    pub fn parse_config_files(
        &mut self,
        config_folder: &std::path::Path,
        theme: Option<&str>,
    ) -> Result<()> {
        self.app_config.parse_config_file(config_folder)?;
        if let Some(theme) = theme {
            self.app_config.theme = theme.to_owned();
        };
        log::info!("app configuartions: {:#?}", self.app_config);

        self.theme_config.parse_config_file(config_folder)?;
        if let Some(theme) = self.theme_config.find_theme(&self.app_config.theme) {
            self.ui.lock().unwrap().theme = theme;
        }
        log::info!("theme configuartions: {:#?}", self.theme_config);

        self.keymap_config.parse_config_file(config_folder)?;
        log::info!("keymap configuartions: {:#?}", self.keymap_config);

        Ok(())
    }
}

impl Default for State {
    fn default() -> Self {
        State {
            app_config: config::AppConfig::default(),
            theme_config: config::ThemeConfig::default(),
            keymap_config: config::KeymapConfig::default(),

            player: RwLock::new(PlayerState::default()),

            ui: Mutex::new(UIState::default()),
        }
    }
}
