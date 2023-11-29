use std::io::Write;

use anyhow::{anyhow, Result};
use librespot_core::{
    authentication::Credentials,
    cache::Cache,
    config::SessionConfig,
    session::{Session, SessionError},
};

use crate::state;

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
    pub fn new(configs: &state::Configs) -> Result<AuthConfig> {
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

fn read_user_auth_details(user: Option<String>) -> Result<(String, String)> {
    let mut username = String::new();
    let mut stdout = std::io::stdout();
    match user {
        None => write!(stdout, "Username: ")?,
        Some(ref u) => write!(stdout, "Username (default: {u}): ")?,
    }
    stdout.flush()?;
    std::io::stdin().read_line(&mut username)?;
    username = username.trim_end().to_string();
    if username.is_empty() {
        username = user.unwrap_or_default();
    }
    let password = rpassword::prompt_password(format!("Password for {username}: "))?;
    Ok((username, password))
}

pub async fn new_session_with_new_creds(auth_config: &AuthConfig) -> Result<Session> {
    tracing::info!("Creating a new session with new authentication credentials");

    let mut user: Option<String> = None;

    for i in 0..3 {
        let (username, password) = read_user_auth_details(user)?;
        user = Some(username.clone());
        match Session::connect(
            auth_config.session_config.clone(),
            Credentials::with_password(username, password),
            Some(auth_config.cache.clone()),
            true,
        )
        .await
        {
            Ok((session, _)) => {
                println!("Successfully authenticated as {}", user.unwrap_or_default());
                return Ok(session);
            }
            Err(err) => {
                eprintln!("Failed to authenticate, {} tries left", 2 - i);
                tracing::warn!("Failed to authenticate: {err:#}")
            }
        }
    }

    Err(anyhow!("authentication failed!"))
}

/// Creates a new Librespot session
///
/// By default, the function will look for cached credentials in the `APP_CACHE_FOLDER` folder.
///
/// If `reauth` is true, re-authenticate by asking the user for Spotify's username and password.
/// The re-authentication process should only happen on the terminal using stdin/stdout.
pub async fn new_session(auth_config: &AuthConfig, reauth: bool) -> Result<Session> {
    match auth_config.cache.credentials() {
        None => {
            let msg = "No cached credentials found, please authenticate the application first.";
            if reauth {
                eprintln!("{msg}");
                new_session_with_new_creds(auth_config).await
            } else {
                anyhow::bail!(msg);
            }
        }
        Some(creds) => {
            match Session::connect(
                auth_config.session_config.clone(),
                creds,
                Some(auth_config.cache.clone()),
                true,
            )
            .await
            {
                Ok((session, _)) => {
                    tracing::info!(
                        "Successfully used the cached credentials to create a new session!"
                    );
                    Ok(session)
                }
                Err(err) => match err {
                    SessionError::AuthenticationError(err) => {
                        anyhow::bail!("Failed to authenticate using cached credentials: {err:#}");
                    }
                    SessionError::IoError(err) => {
                        anyhow::bail!("{err:#}\nPlease check your internet connection.");
                    }
                },
            }
        }
    }
}
