const CONFIG_FOLDER: &str = "spotify-player";
const TOKEN_CACHE_FILE: &str = ".spotify_token_cache.json";
const CLIENT_CONFIG_FILE: &str = "client.toml";
pub const UI_REFRESH_DURATION: Duration = Duration::from_millis(30);
pub const PLAYBACK_REFRESH_DURACTION: Duration = Duration::from_secs(1);
pub const TRACK_DESC_ITEM_MAX_LEN: usize = 32;

use crate::prelude::*;
use std::{path::PathBuf, time::Duration};

#[derive(Deserialize)]
/// Spotify client configurations
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl ClientConfig {
    pub fn from_config_file(path: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path.join(CLIENT_CONFIG_FILE))?;
        Ok(toml::from_str::<Self>(&content)?)
    }
}

/// returns the application's configuration folder path
pub fn get_config_folder_path() -> Result<PathBuf> {
    match dirs_next::home_dir() {
        Some(home) => Ok(home.join(".config").join(CONFIG_FOLDER)),
        None => Err(anyhow!("Cannot find the $HOME folder")),
    }
}

/// return the token (spotify authentictation token) cache file path
pub fn get_token_cache_file_path(config_folder: &PathBuf) -> PathBuf {
    config_folder.join(TOKEN_CACHE_FILE)
}
