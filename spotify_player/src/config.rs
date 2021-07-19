const TOKEN_CACHE_FILE: &str = ".spotify_token_cache.json";
const CONFIG_FOLDER: &str = "spotify-player";
const CLIENT_CONFIG_FILE: &str = "client.toml";

use crate::prelude::*;
use std::path::PathBuf;

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
pub fn get_token_cache_file_path() -> Result<PathBuf> {
    Ok(get_config_folder_path()?.join(TOKEN_CACHE_FILE))
}
