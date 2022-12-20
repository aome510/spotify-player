use std::io::Write;

use anyhow::{anyhow, Result};
use librespot_core::{
    authentication::Credentials,
    cache::Cache,
    session::{Session, SessionError},
};

use crate::config::AppConfig;

fn read_user_auth_details(user: Option<String>) -> Result<(String, String)> {
    let mut username = String::new();
    let mut stdout = std::io::stdout();
    match user {
        None => write!(stdout, "Username: ")?,
        Some(ref u) => write!(stdout, "Username (default: {}): ", u)?,
    }
    stdout.flush()?;
    std::io::stdin().read_line(&mut username)?;
    username = username.trim_end().to_string();
    if username.is_empty() {
        username = user.unwrap_or_default();
    }
    let password = rpassword::prompt_password(format!("Password for {}: ", username))?;
    Ok((username, password))
}

async fn new_session_with_new_creds(cache: &Cache, app_config: &AppConfig) -> Result<Session> {
    tracing::info!("Creating a new session with new authentication credentials");

    println!("Authentication token not found or invalid, please reauthenticate.");

    let mut user: Option<String> = None;

    for i in 0..3 {
        let (username, password) = read_user_auth_details(user)?;
        user = Some(username.clone());
        match Session::connect(
            app_config.session_config(),
            Credentials::with_password(username, password),
            Some(cache.clone()),
            true,
        )
        .await
        {
            Ok((session, _)) => {
                println!("Successfully authenticated as {}", user.unwrap_or_default());
                return Ok(session);
            }
            Err(err) => {
                println!("Failed to authenticate, {} tries left", 2 - i);
                tracing::warn!("Failed to authenticate: {err:#}")
            }
        }
    }

    Err(anyhow!("authentication failed!"))
}

/// creates new Librespot session
pub async fn new_session(
    cache_folder: &std::path::Path,
    audio_cache: bool,
    app_config: &AppConfig,
) -> Result<Session> {
    // specifying `audio_cache` to `None` to disable audio cache
    let audio_cache_folder = if audio_cache {
        Some(cache_folder.join("audio"))
    } else {
        None
    };

    let cache = Cache::new(
        Some(cache_folder),
        None,
        audio_cache_folder.as_deref(),
        None,
    )?;

    // create a new session if either
    // - there is no cached credentials or
    // - the cached credentials are expired or invalid
    match cache.credentials() {
        None => new_session_with_new_creds(&cache, app_config).await,
        Some(creds) => {
            match Session::connect(
                app_config.session_config(),
                creds,
                Some(cache.clone()),
                true,
            )
            .await
            {
                Ok((session, _)) => {
                    tracing::info!("Use the cached credentials");
                    Ok(session)
                }
                Err(err) => match err {
                    SessionError::AuthenticationError(err) => {
                        tracing::warn!("Failed to authenticate: {err:#}");
                        new_session_with_new_creds(&cache, app_config).await
                    }
                    SessionError::IoError(err) => Err(anyhow!(format!(
                        "{}\nPlease check your internet connection.",
                        err
                    ))),
                },
            }
        }
    }
}
