mod keymap;
mod theme;

const DEFAULT_CONFIG_FOLDER: &str = ".config/spotify-player";
const DEFAULT_CACHE_FOLDER: &str = ".cache/spotify-player";
const APP_CONFIG_FILE: &str = "app.toml";
const THEME_CONFIG_FILE: &str = "theme.toml";
const KEYMAP_CONFIG_FILE: &str = "keymap.toml";

use anyhow::{anyhow, Result};
use config_parser2::{config_parser_impl, ConfigParse, ConfigParser};
use librespot_core::config::SessionConfig;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use keymap::KeymapConfig;
use theme::ThemeConfig;

pub use theme::Theme;

use crate::auth::SPOTIFY_CLIENT_ID;

static CONFIGS: OnceLock<Configs> = OnceLock::new();

#[derive(Debug)]
pub struct Configs {
    pub app_config: AppConfig,
    pub keymap_config: KeymapConfig,
    pub theme_config: ThemeConfig,
    pub cache_folder: std::path::PathBuf,
}

impl Configs {
    pub fn new(config_folder: &std::path::Path, cache_folder: &std::path::Path) -> Result<Self> {
        Ok(Self {
            app_config: AppConfig::new(config_folder)?,
            keymap_config: KeymapConfig::new(config_folder)?,
            theme_config: ThemeConfig::new(config_folder)?,
            cache_folder: cache_folder.to_path_buf(),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, ConfigParse)]
#[allow(clippy::struct_excessive_bools)]
/// Application configurations
pub struct AppConfig {
    pub theme: String,
    pub client_id: String,
    pub client_id_command: Option<Command>,

    pub client_port: u16,

    pub login_redirect_uri: String,

    pub player_event_hook_command: Option<Command>,

    pub playback_format: String,
    #[cfg(feature = "notify")]
    pub notify_format: NotifyFormat,
    #[cfg(feature = "notify")]
    pub notify_timeout_in_secs: u64,

    pub tracks_playback_limit: usize,

    // session configs
    pub proxy: Option<String>,
    pub ap_port: Option<u16>,

    // duration configs
    pub app_refresh_duration_in_ms: u64,
    pub playback_refresh_duration_in_ms: u64,

    pub page_size_in_rows: usize,

    // icon configs
    pub play_icon: String,
    pub pause_icon: String,
    pub liked_icon: String,

    // layout configs
    pub border_type: BorderType,
    pub progress_bar_type: ProgressBarType,

    pub layout: LayoutConfig,

    #[cfg(feature = "image")]
    pub cover_img_length: usize,
    #[cfg(feature = "image")]
    pub cover_img_width: usize,
    #[cfg(feature = "image")]
    pub cover_img_scale: f32,

    #[cfg(feature = "media-control")]
    pub enable_media_control: bool,

    #[cfg(feature = "streaming")]
    pub enable_streaming: StreamingType,

    #[cfg(feature = "notify")]
    pub enable_notify: bool,

    pub enable_cover_image_cache: bool,

    pub default_device: String,

    pub device: DeviceConfig,

    #[cfg(all(feature = "streaming", feature = "notify"))]
    pub notify_streaming_only: bool,

    pub seek_duration_secs: u16,

    pub sort_artist_albums_by_type: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Position {
    Top,
    Bottom,
}
config_parser_impl!(Position);

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum BorderType {
    Hidden,
    Plain,
    Rounded,
    Double,
    Thick,
}
config_parser_impl!(BorderType);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum ProgressBarType {
    Line,
    Rectangle,
}
config_parser_impl!(ProgressBarType);

#[derive(Debug, Deserialize, Serialize, ConfigParse, Clone)]
pub struct Command {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl Command {
    /// Execute a command, returning stdout if succeeded or stderr if failed
    pub fn execute(&self, extra_args: Option<Vec<String>>) -> anyhow::Result<String> {
        let mut args = self.args.clone();
        args.extend(extra_args.unwrap_or_default());

        let output = std::process::Command::new(&self.command)
            .args(&args)
            .output()?;

        if !output.status.success() {
            let stderr = std::str::from_utf8(&output.stderr)?.to_string();
            anyhow::bail!(stderr);
        }

        let stdout = std::str::from_utf8(&output.stdout)?.to_string();
        Ok(stdout)
    }
}

#[derive(Debug, Deserialize, Serialize, ConfigParse, Clone)]
/// Application device configurations
pub struct DeviceConfig {
    pub name: String,
    pub device_type: String,
    pub volume: u8,
    pub bitrate: u16,
    pub audio_cache: bool,
    pub normalization: bool,
    pub autoplay: bool,
}

#[derive(Debug, Deserialize, Serialize, ConfigParse, Clone)]
#[cfg(feature = "notify")]
pub struct NotifyFormat {
    pub summary: String,
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, ConfigParse, Clone)]
// Application layout configurations
pub struct LayoutConfig {
    pub library: LibraryLayoutConfig,
    pub playback_window_position: Position,
    pub playback_window_height: usize,
}

#[derive(Debug, Deserialize, Serialize, ConfigParse, Clone)]
pub struct LibraryLayoutConfig {
    pub playlist_percent: u16,
    pub album_percent: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(from = "StreamingTypeOrBool")]
pub enum StreamingType {
    Always,
    DaemonOnly,
    Never,
}
config_parser_impl!(StreamingType);

// For backward compatibility, to accept booleans for enable_streaming
#[derive(Deserialize)]
enum RawStreamingType {
    Always,
    DaemonOnly,
    Never,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StreamingTypeOrBool {
    Bool(bool),
    Type(RawStreamingType),
}

impl From<StreamingTypeOrBool> for StreamingType {
    fn from(v: StreamingTypeOrBool) -> Self {
        match v {
            StreamingTypeOrBool::Bool(true)
            | StreamingTypeOrBool::Type(RawStreamingType::Always) => StreamingType::Always,
            StreamingTypeOrBool::Bool(false)
            | StreamingTypeOrBool::Type(RawStreamingType::Never) => StreamingType::Never,
            StreamingTypeOrBool::Type(RawStreamingType::DaemonOnly) => StreamingType::DaemonOnly,
        }
    }
}

impl Command {
    pub fn new<C, A>(command: C, args: &[A]) -> Self
    where
        C: std::fmt::Display,
        A: std::fmt::Display,
    {
        Self {
            command: command.to_string(),
            args: args.iter().map(std::string::ToString::to_string).collect(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dracula".to_owned(),
            // official Spotify web app's client id
            client_id: "65b708073fc0480ea92a077233ca87bd".to_string(),
            client_id_command: None,

            client_port: 8080,

            login_redirect_uri: "http://127.0.0.1:8989/login".to_string(),

            tracks_playback_limit: 50,

            playback_format: String::from(
                "{status} {track} • {artists} {liked}\n{album}\n{metadata}",
            ),
            #[cfg(feature = "notify")]
            notify_format: NotifyFormat {
                summary: String::from("{track} • {artists}"),
                body: String::from("{album}"),
            },
            #[cfg(feature = "notify")]
            notify_timeout_in_secs: 0,

            player_event_hook_command: None,

            proxy: None,
            ap_port: None,
            app_refresh_duration_in_ms: 32,
            playback_refresh_duration_in_ms: 0,

            page_size_in_rows: 20,

            pause_icon: "▌▌".to_string(),
            play_icon: "▶".to_string(),
            liked_icon: "♥".to_string(),

            border_type: BorderType::Plain,
            progress_bar_type: ProgressBarType::Rectangle,

            layout: LayoutConfig::default(),

            #[cfg(feature = "image")]
            cover_img_length: 9,
            #[cfg(feature = "image")]
            cover_img_width: 5,
            #[cfg(feature = "image")]
            cover_img_scale: 1.0,

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
            enable_streaming: StreamingType::Always,

            #[cfg(feature = "notify")]
            enable_notify: true,

            enable_cover_image_cache: true,

            default_device: "spotify-player".to_string(),

            device: DeviceConfig::default(),

            #[cfg(all(feature = "streaming", feature = "notify"))]
            notify_streaming_only: false,

            seek_duration_secs: 5,

            sort_artist_albums_by_type: false,
        }
    }
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            name: "spotify-player".to_string(),
            device_type: "speaker".to_string(),
            volume: 70,
            bitrate: 320,
            audio_cache: false,
            normalization: false,
            autoplay: false,
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            library: LibraryLayoutConfig {
                playlist_percent: 40,
                album_percent: 40,
            },
            playback_window_position: Position::Top,
            playback_window_height: 6,
        }
    }
}

impl LayoutConfig {
    fn check_values(&self) -> anyhow::Result<()> {
        if self.library.album_percent + self.library.playlist_percent > 99 {
            anyhow::bail!("Invalid library layout: summation of album_percent and playlist_percent cannot be greater than 99!");
        }
        Ok(())
    }
}

impl AppConfig {
    pub fn new(path: &Path) -> Result<Self> {
        let mut config = Self::default();
        if !config.parse_config_file(path)? {
            config.write_config_file(path)?;
        }

        config.layout.check_values()?;
        Ok(config)
    }

    // parses configurations from an application config file in `path` folder,
    // then updates the current configurations accordingly.
    // returns false if no config file found and true otherwise
    fn parse_config_file(&mut self, path: &Path) -> Result<bool> {
        let file_path = path.join(APP_CONFIG_FILE);
        match std::fs::read_to_string(file_path) {
            Ok(content) => self
                .parse(toml::from_str::<toml::Value>(&content)?)
                .map(|()| true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn write_config_file(&self, path: &Path) -> Result<()> {
        toml::to_string_pretty(&self)
            .map_err(From::from)
            .and_then(|content| {
                std::fs::write(path.join(APP_CONFIG_FILE), content).map_err(From::from)
            })
    }

    pub fn session_config(&self) -> SessionConfig {
        let proxy = self
            .proxy
            .as_ref()
            .and_then(|proxy| match Url::parse(proxy) {
                Err(err) => {
                    tracing::warn!("failed to parse proxy url {proxy}: {err:#}");
                    None
                }
                Ok(url) => Some(url),
            });
        SessionConfig {
            proxy,
            ap_port: self.ap_port,
            client_id: SPOTIFY_CLIENT_ID.to_string(),
            autoplay: Some(self.device.autoplay),
            ..Default::default()
        }
    }

    /// Returns stdout of `client_id_command` if set, otherwise it returns the the value of `client_id`
    pub fn get_client_id(&self) -> Result<String> {
        match self.client_id_command {
            Some(ref cmd) => cmd.execute(None).map(|out| out.trim().into()),
            None => Ok(self.client_id.clone()),
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

pub fn get_config() -> &'static Configs {
    CONFIGS.get().expect("configs is already initialized")
}
pub fn set_config(configs: Configs) {
    CONFIGS
        .set(configs)
        .expect("configs should be initialized only once");
}
