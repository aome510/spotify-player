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

pub use parking_lot::{Mutex, RwLock};

/// Application's shared state (wrapped inside an `std::sync::Arc`)
pub type SharedState = std::sync::Arc<State>;

/// Application's state
pub struct State {
    pub ui: Mutex<UIState>,
    pub player: RwLock<PlayerState>,
    pub data: RwLock<AppData>,

    pub is_daemon: bool,
}

impl State {
    pub fn new(is_daemon: bool) -> Self {
        let mut ui = UIState::default();
        let configs = config::get_config();

        if let Some(theme) = configs.theme_config.find_theme(&configs.app_config.theme) {
            // update the UI's theme based on the `theme` config option
            ui.theme = theme;
        }

        let app_data = AppData::new(&configs.cache_folder);

        Self {
            ui: Mutex::new(ui),
            player: RwLock::new(PlayerState::default()),
            data: RwLock::new(app_data),
            is_daemon,
        }
    }

    #[cfg(feature = "streaming")]
    pub fn is_streaming_enabled(&self) -> bool {
        let configs = config::get_config();
        configs.app_config.enable_streaming == config::StreamingType::Always
            || (configs.app_config.enable_streaming == config::StreamingType::DaemonOnly
                && self.is_daemon)
    }
}
