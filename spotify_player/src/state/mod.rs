mod data;
mod model;
mod player;
mod ui;

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
        theme: Option<&str>,
    ) -> Result<()> {
        self.app_config.parse_config_file(config_folder)?;
        if let Some(theme) = theme {
            self.app_config.theme = theme.to_owned();
        };
        log::info!("general configuartions: {:#?}", self.app_config);

        self.theme_config.parse_config_file(config_folder)?;
        log::info!("theme configuartions: {:#?}", self.theme_config);

        self.keymap_config.parse_config_file(config_folder)?;
        log::info!("keymap configuartions: {:#?}", self.keymap_config);

        if let Some(theme) = self.theme_config.find_theme(&self.app_config.theme) {
            // update the UI theme based on the `theme` config option
            // specified in the app's general configurations
            self.ui.lock().theme = theme;
        }

        Ok(())
    }

    /// gets a list of items possibly filtered by a search query if exists a search popup
    pub fn filtered_items_by_search<'a, T: std::fmt::Display>(&self, items: &'a [T]) -> Vec<&'a T> {
        match self.ui.lock().popup {
            Some(PopupState::Search { ref query }) => items
                .iter()
                .filter(|t| Self::is_match(&t.to_string().to_lowercase(), &query.to_lowercase()))
                .collect::<Vec<_>>(),
            _ => items.iter().collect::<Vec<_>>(),
        }
    }

    /// checks if a string matches a given query
    fn is_match(s: &str, query: &str) -> bool {
        query
            .split(' ')
            .fold(true, |acc, cur| acc & s.contains(cur))
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
