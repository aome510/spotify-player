use anyhow::Result;
use librespot_core::{authentication::Credentials, cache::Cache, config::SessionConfig, Session};
use librespot_oauth::get_access_token;

use crate::config;

pub const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
// based on https://github.com/librespot-org/librespot/blob/f96f36c064795011f9fee912291eecb1aa46fff6/src/main.rs#L173
const OAUTH_SCOPES: &[&str] = &[
    "app-remote-control",
    "playlist-modify",
    "playlist-modify-private",
    "playlist-modify-public",
    "playlist-read",
    "playlist-read-collaborative",
    "playlist-read-private",
    "streaming",
    "ugc-image-upload",
    "user-follow-modify",
    "user-follow-read",
    "user-library-modify",
    "user-library-read",
    "user-modify",
    "user-modify-playback-state",
    "user-modify-private",
    "user-personalized",
    "user-read-birthdate",
    "user-read-currently-playing",
    "user-read-email",
    "user-read-play-history",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-read-private",
    "user-read-recently-played",
    "user-top-read",
];

#[derive(Clone)]
pub struct AuthConfig {
    pub cache: Cache,
    pub session_config: SessionConfig,
    pub login_redirect_uri: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            cache: Cache::new(None::<String>, None, None, None).unwrap(),
            session_config: SessionConfig::default(),
            login_redirect_uri: "http://127.0.0.1:8989/login".to_string(),
        }
    }
}

impl AuthConfig {
    /// Create a `librespot::Session` from authentication configs
    pub fn session(&self) -> Session {
        Session::new(self.session_config.clone(), Some(self.cache.clone()))
    }

    pub fn new(configs: &config::Configs) -> Result<AuthConfig> {
        let audio_cache_folder = if configs.app_config.device.audio_cache {
            Some(configs.cache_folder.join("audio"))
        } else {
            None
        };

        let cache = Cache::new(
            Some(configs.cache_folder.clone()),
            None,
            audio_cache_folder,
            None,
        )?;

        Ok(AuthConfig {
            cache,
            session_config: configs.app_config.session_config(),
            login_redirect_uri: configs.app_config.login_redirect_uri.clone(),
        })
    }
}

/// Get Spotify credentials to authenticate the application
///
/// # Args
/// - `auth_config`: authentication configuration
/// - `reauth`: whether to re-authenticate the application if no cached credentials are found
// - `use_cached`: whether to use cached credentials if available
pub async fn get_creds(
    auth_config: &AuthConfig,
    reauth: bool,
    use_cached: bool,
) -> Result<Credentials> {
    let creds = if use_cached {
        auth_config.cache.credentials()
    } else {
        None
    };

    Ok(match creds {
        None => {
            let msg = "No cached credentials found, please authenticate the application first.";
            if reauth {
                eprintln!("{msg}");
                get_access_token(
                    SPOTIFY_CLIENT_ID,
                    &auth_config.login_redirect_uri,
                    OAUTH_SCOPES.to_vec(),
                )
                .map(|t| Credentials::with_access_token(t.access_token))?
            } else {
                anyhow::bail!(msg);
            }
        }
        Some(creds) => {
            tracing::info!("Using cached credentials");
            creds
        }
    })
}
