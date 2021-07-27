const DEFAULT_CONFIG_FOLDER: &str = ".config/spotify-player";
const TOKEN_CACHE_FILE: &str = ".spotify_token_cache.json";
const CLIENT_CONFIG_FILE: &str = "client.toml";
const APP_CONFIG_FILE: &str = "app.toml";

use crate::prelude::*;
use config_parser2::*;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
/// Spotify client configurations
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize, ConfigParse)]
/// Application (general) configurations
pub struct AppConfig {
    pub ui_refresh_duration_in_ms: u64,
    pub playback_refresh_duration_in_ms: u64,
    pub track_table_item_max_len: usize,
}

impl ClientConfig {
    pub fn from_config_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path.join(CLIENT_CONFIG_FILE))?;
        Ok(toml::from_str::<Self>(&content)?)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            ui_refresh_duration_in_ms: 30,
            playback_refresh_duration_in_ms: 1000,
            track_table_item_max_len: 32,
        }
    }
}

impl AppConfig {
    pub fn parse_config_file(&mut self, path: &Path) -> Result<()> {
        match std::fs::read_to_string(path.join(APP_CONFIG_FILE)) {
            Err(err) => {
                log::warn!(
                    "failed to open application config file: {:#?}...\nUse the default configurations instead...",
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
