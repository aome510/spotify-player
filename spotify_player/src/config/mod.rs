mod keymap;
mod theme;

const DEFAULT_CONFIG_FOLDER: &str = ".config/spotify-player";
const DEFAULT_CACHE_FOLDER: &str = ".cache/spotify-player";
const APP_CONFIG_FILE: &str = "app.toml";
const THEME_CONFIG_FILE: &str = "theme.toml";
const KEYMAP_CONFIG_FILE: &str = "keymap.toml";

use anyhow::{anyhow, Result};
use config_parser2::*;
use librespot_core::config::SessionConfig;
use reqwest::Url;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use keymap::*;
pub use theme::*;

#[derive(Debug, Deserialize, ConfigParse)]
/// Application configurations
pub struct AppConfig {
    pub theme: String,
    pub client_id: String,

    pub client_port: u16,

    pub copy_command: Command,

    pub playback_format: String,
    #[cfg(feature = "notify")]
    pub notify_format: NotifyFormat,

    // session configs
    pub proxy: Option<String>,
    pub ap_port: Option<u16>,

    // duration configs
    pub app_refresh_duration_in_ms: u64,
    pub playback_refresh_duration_in_ms: u64,
    #[cfg(feature = "image")]
    pub cover_image_refresh_duration_in_ms: u64,

    pub page_size_in_rows: usize,

    pub track_table_item_max_len: usize,

    // icon configs
    pub play_icon: String,
    pub pause_icon: String,
    pub liked_icon: String,

    // layout configs
    pub border_type: BorderType,
    pub progress_bar_type: ProgressBarType,

    pub playback_window_position: Position,

    #[cfg(feature = "image")]
    pub cover_img_length: usize,
    #[cfg(feature = "image")]
    pub cover_img_width: usize,
    #[cfg(feature = "image")]
    pub cover_img_scale: f32,

    pub playback_window_width: usize,

    #[cfg(feature = "media-control")]
    pub enable_media_control: bool,

    #[cfg(feature = "streaming")]
    pub enable_streaming: bool,

    pub enable_cover_image_cache: bool,

    pub default_device: String,

    pub device: DeviceConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Position {
    Top,
    Bottom,
}
config_parser_impl!(Position);

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum BorderType {
    Hidden,
    Plain,
    Rounded,
    Double,
    Thick,
}
config_parser_impl!(BorderType);

#[derive(Debug, Deserialize, Clone)]
pub enum ProgressBarType {
    Line,
    Rectangle,
}
config_parser_impl!(ProgressBarType);

#[derive(Debug, Deserialize, ConfigParse, Clone)]
pub struct Command {
    pub command: String,
    pub args: Vec<String>,
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

#[derive(Debug, Deserialize, ConfigParse, Clone)]
#[cfg(feature = "notify")]
pub struct NotifyFormat {
    pub summary: String,
    pub body: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dracula".to_owned(),
            // official spotify web app's client id
            client_id: "65b708073fc0480ea92a077233ca87bd".to_string(),

            client_port: 8080,

            playback_format: String::from("{track} • {artists}\n{album}\n{metadata}"),
            #[cfg(feature = "notify")]
            notify_format: NotifyFormat {
                summary: String::from("{track} • {artists}"),
                body: String::from("{album}"),
            },

            #[cfg(target_os = "macos")]
            copy_command: Command {
                command: "pbcopy".to_string(),
                args: vec![],
            },
            #[cfg(all(unix, not(target_os = "macos")))]
            copy_command: Command {
                command: "xclip".to_string(),
                args: vec!["-sel".to_string(), "c".to_string()],
            },
            #[cfg(target_os = "windows")]
            copy_command: Command {
                command: "clip".to_string(),
                args: vec![],
            },

            proxy: None,
            ap_port: None,
            app_refresh_duration_in_ms: 32,
            playback_refresh_duration_in_ms: 0,

            #[cfg(feature = "image")]
            cover_image_refresh_duration_in_ms: 2000,

            page_size_in_rows: 20,

            track_table_item_max_len: 32,

            pause_icon: "▌▌".to_string(),
            play_icon: "▶".to_string(),
            liked_icon: "♥".to_string(),

            border_type: BorderType::Plain,
            progress_bar_type: ProgressBarType::Rectangle,

            playback_window_position: Position::Top,

            #[cfg(feature = "image")]
            cover_img_length: 9,
            #[cfg(feature = "image")]
            cover_img_width: 5,
            #[cfg(feature = "image")]
            cover_img_scale: 1.0,

            playback_window_width: 6,

            // Because of the "creating new window and stealing focus" behaviour
            // when running the media control event loop on startup,
            // media control support is disabled by default for Windows and MacOS.
            // Users will need to explicitly enable this option in their configuration files.
            #[cfg(feature = "media-control")]
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            enable_media_control: false,
            #[cfg(feature = "media-control")]
            #[cfg(all(unix, not(target_os = "macos")))]
            enable_media_control: true,

            #[cfg(feature = "streaming")]
            enable_streaming: true,

            enable_cover_image_cache: true,

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

    pub fn session_config(&self) -> SessionConfig {
        let proxy = self
            .proxy
            .as_ref()
            .and_then(|proxy| match Url::parse(proxy) {
                Err(err) => {
                    tracing::warn!("failed to parse proxy url {proxy}: {err}");
                    None
                }
                Ok(url) => Some(url),
            });
        SessionConfig {
            proxy,
            ap_port: self.ap_port,
            ..Default::default()
        }
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
