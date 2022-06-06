mod keymap;
mod theme;

const DEFAULT_CONFIG_FOLDER: &str = ".config/spotify-player";
const DEFAULT_CACHE_FOLDER: &str = ".cache/spotify-player";
const APP_CONFIG_FILE: &str = "app.toml";
const THEME_CONFIG_FILE: &str = "theme.toml";
const KEYMAP_CONFIG_FILE: &str = "keymap.toml";

use anyhow::{anyhow, Result};
use config_parser2::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use keymap::*;
pub use theme::*;

#[derive(Debug, Deserialize, ConfigParse)]
/// Application configurations
pub struct AppConfig {
    pub theme: String,
    pub client_id: String,
    pub app_refresh_duration_in_ms: u64,
    pub playback_refresh_duration_in_ms: u64,
    pub track_table_item_max_len: usize,
    #[cfg(feature = "media-control")]
    pub enable_media_control: bool,

    pub default_device: String,

    pub device: DeviceConfig,
}

#[derive(Debug, Deserialize, ConfigParse, Clone)]
/// Application device configurations
pub struct DeviceConfig {
    pub name: String,
    pub device_type: String,
    pub volume: u8,
    pub bitrate: u16,
    pub audio_cache: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dracula".to_owned(),
            // official spotify web app's client id
            client_id: "65b708073fc0480ea92a077233ca87bd".to_string(),
            app_refresh_duration_in_ms: 32,
            playback_refresh_duration_in_ms: 0,
            track_table_item_max_len: 32,

            // Because of the "creating new window and stealing focus" behaviour
            // when running the media control event loop on startup,
            // media control support is disabled by default for Windows and MacOS.
            // Users will need to explicitly enable this option in their configuration files.
            #[cfg(feature = "media-control")]
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            enable_media_control: false,
            #[cfg(feature = "media-control")]
            #[cfg(target_os = "linux")]
            enable_media_control: true,

            default_device: "spotify-player".to_string(),

            device: DeviceConfig::default(),
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            name: "spotify-player".to_string(),
            device_type: "speaker".to_string(),
            volume: 50,
            bitrate: 160,
            audio_cache: false,
        }
    }
}

impl AppConfig {
    // parses configurations from an application config file in `path` folder,
    // then updates the current configurations accordingly.
    pub fn parse_config_file(&mut self, path: &Path) -> Result<()> {
        let file_path = path.join(APP_CONFIG_FILE);
        match std::fs::read_to_string(&file_path) {
            Err(err) => {
                tracing::warn!(
                    "Failed to open the application config file (path={file_path:?}): {err:#}. Use the default configurations instead",
                );
            }
            Ok(content) => {
                self.parse(toml::from_str::<toml::Value>(&content)?)?;
            }
        }
        Ok(())
    }
}

/// gets the application's configuration folder path
pub fn get_config_folder_path() -> Result<PathBuf> {
    match dirs_next::home_dir() {
        Some(home) => Ok(home.join(DEFAULT_CONFIG_FOLDER)),
        None => Err(anyhow!("cannot find the $HOME folder")),
    }
}

/// gets the application's cache folder path
pub fn get_cache_folder_path() -> Result<PathBuf> {
    match dirs_next::home_dir() {
        Some(home) => Ok(home.join(DEFAULT_CACHE_FOLDER)),
        None => Err(anyhow!("cannot find the $HOME folder")),
    }
}
