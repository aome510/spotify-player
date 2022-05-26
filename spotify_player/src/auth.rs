use std::io::Write;

use anyhow::{anyhow, Result};
use librespot_core::{
    authentication::Credentials,
    cache::Cache,
    config::SessionConfig,
    session::{Session, SessionError},
};

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
    let password = rpassword::prompt_password(&format!("Password for {}: ", username))?;
    Ok((username, password))
}

async fn new_session_with_new_creds(cache: &Cache) -> Result<Session> {
    tracing::info!("Creating a new session with new authentication credentials");

    println!("Authentication token not found or invalid, please reauthenticate.");

    let mut user: Option<String> = None;

    for i in 0..3 {
        let (username, password) = read_user_auth_details(user)?;
        user = Some(username.clone());
        match Session::connect(
            SessionConfig::default(),
            Credentials::with_password(username, password),
            Some(cache.clone()),
        )
        .await
        {
            Ok(session) => {
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
pub async fn new_session(cache_folder: &std::path::Path, audio_cache: bool) -> Result<Session> {
    let audio_cache_folder = cache_folder.join("audio");
    // specifying `audio_cache` to `None` to disable audio cache
    let audio_cache = if audio_cache {
        Some(audio_cache_folder.as_path())
    } else {
        None
    };

    let cache = Cache::new(Some(cache_folder), audio_cache, None)?;

    // create a new session if either
    // - there is no cached credentials or
    // - the cached credentials are expired or invalid
    match cache.credentials() {
        None => new_session_with_new_creds(&cache).await,
        Some(creds) => {
            match Session::connect(SessionConfig::default(), creds, Some(cache.clone())).await {
                Ok(session) => {
                    tracing::info!("Use the cached credentials");
                    Ok(session)
                }
                Err(err) => match err {
                    SessionError::AuthenticationError(err) => {
                        tracing::warn!("Failed to authenticate: {err:#}");
                        new_session_with_new_creds(&cache).await
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
