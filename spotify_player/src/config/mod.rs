mod keymap;
mod theme;

const DEFAULT_CONFIG_FOLDER: &str = ".config/spotify-player";
const TOKEN_CACHE_FILE: &str = ".spotify_token_cache.json";
const CLIENT_CONFIG_FILE: &str = "client.toml";
const APP_CONFIG_FILE: &str = "app.toml";
const THEME_CONFIG_FILE: &str = "theme.toml";
const KEYMAP_CONFIG_FILE: &str = "keymap.toml";

use anyhow::{anyhow, Result};
use config_parser2::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use keymap::*;
pub use theme::*;

#[derive(Deserialize)]
/// Spotify client configurations
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize, ConfigParse)]
/// Application configurations
pub struct AppConfig {
    pub theme: String,
    pub playback_update_delay_in_ms: u64,
    pub app_refresh_duration_in_ms: u64,
    pub playback_refresh_duration_in_ms: u64,
    pub track_table_item_max_len: usize,
}

impl ClientConfig {
    // creates client configurations from a client config file in `path` folder
    pub fn from_config_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path.join(CLIENT_CONFIG_FILE))?;
        Ok(toml::from_str::<Self>(&content)?)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            theme: "dracula".to_owned(),
            playback_update_delay_in_ms: 1_000,
            app_refresh_duration_in_ms: 30,
            playback_refresh_duration_in_ms: 0,
            track_table_item_max_len: 32,
        }
    }
}

impl AppConfig {
    // parses configurations from an application config file in `path` folder,
    // then updates the current configurations accordingly.
    pub fn parse_config_file(&mut self, path: &Path) -> Result<()> {
        match std::fs::read_to_string(path.join(APP_CONFIG_FILE)) {
            Err(err) => {
                log::warn!(
                    "failed to open the application config file: {:#?}...\nUse the default configurations instead...",
                    err
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

/// gets the token (spotify authentictation token) cache file path
pub fn get_token_cache_file_path(config_folder: &Path) -> PathBuf {
    config_folder.join(TOKEN_CACHE_FILE)
}
