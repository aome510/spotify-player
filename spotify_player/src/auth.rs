use anyhow::{anyhow, Result};
use librespot_core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, error::ErrorKind,
    session::Session, Error,
};
use librespot_oauth::{get_access_token, OAuthError};

use crate::config;

// copied from https://github.com/hrkfdn/ncspot/pull/1244/commits/81f13e2f9458a378d70d864716c2d915ad8cdaa4
// TODO: adjust, this is just for debugging
pub const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
pub const CLIENT_REDIRECT_URI: &str = "http://127.0.0.1:8989/login";
static OAUTH_SCOPES: &[&str] = &[
    "playlist-modify",
    "playlist-modify-private",
    "playlist-modify-public",
    "playlist-read",
    "playlist-read-collaborative",
    "playlist-read-private",
    "streaming",
    "user-follow-modify",
    "user-follow-read",
    "user-library-modify",
    "user-library-read",
    "user-modify",
    "user-modify-playback-state",
    "user-modify-private",
    "user-personalized",
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
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            cache: Cache::new(None::<String>, None, None, None).unwrap(),
            session_config: SessionConfig::default(),
        }
    }
}

impl AuthConfig {
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
        })
    }
}

fn get_credentials() -> Result<Credentials, OAuthError> {
    get_access_token(
        SPOTIFY_CLIENT_ID,
        CLIENT_REDIRECT_URI,
        OAUTH_SCOPES.to_vec(),
    )
    .map(|t| Credentials::with_access_token(t.access_token))
}

async fn create_creds() -> Result<Credentials> {
    tracing::info!("Creating new authentication credentials");

    for i in 0..3 {
        match get_credentials() {
            Ok(c) => {
                println!("Successfully authenticated");
                return Ok(c);
            }
            Err(err) => {
                eprintln!("Failed to authenticate, {} tries left", 2 - i);
                tracing::warn!("Failed to authenticate: {err:#}")
            }
        }
    }

    Err(anyhow!("authentication failed!"))
}

/// Creates a new Librespot session and connects it
///
/// By default, the function will look for cached credentials in the `APP_CACHE_FOLDER` folder.
///
/// If `reauth` is true, re-authenticate by generating new credentials
pub async fn new_session(auth_config: &AuthConfig, reauth: bool) -> Result<Session> {
    // obtain credentials
    let creds = match auth_config.cache.credentials() {
        None => {
            let msg = "No cached credentials found, please authenticate the application first.";
            if reauth {
                eprintln!("{msg}");
                create_creds().await?
            } else {
                anyhow::bail!(msg);
            }
        }
        Some(creds) => {
            tracing::info!("using cached credentials");
            creds
        }
    };
    let session = Session::new(
        auth_config.session_config.clone(),
        Some(auth_config.cache.clone()),
    );
    // attempt to connect the session
    match session.connect(creds, true).await {
        Ok(()) => {
            tracing::info!("Successfully created a new session!");
            Ok(session)
        }
        Err(Error { kind, error }) => match kind {
            ErrorKind::Unauthenticated => {
                anyhow::bail!("Failed to authenticate using cached credentials: {error:#}");
            }
            ErrorKind::Unavailable => {
                anyhow::bail!("{error:#}\nPlease check your internet connection.");
            }
            _ => anyhow::bail!("{error:#}"),
        },
    }
}
