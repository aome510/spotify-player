use std::io::Write;

use anyhow::{anyhow, Result};
use librespot_core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
};

fn read_user_auth_details(user: Option<String>) -> Result<(String, String)> {
    let mut username = String::new();
    let mut stdout = std::io::stdout();
    write!(
        stdout,
        "Username{}: ",
        if user.is_none() {
            "".to_string()
        } else {
            format!(" (default {})", user.clone().unwrap())
        }
    )?;
    stdout.flush()?;
    std::io::stdin().read_line(&mut username)?;
    username = username.trim_end().to_string();
    if username.is_empty() {
        username = user.unwrap_or_default();
    }
    let password = rpassword::prompt_password_stdout(&format!("Password for {}: ", username))?;
    Ok((username, password))
}

async fn new_session_with_new_creds(cache: &Cache) -> Result<Session> {
    log::info!("creating a new session with new authentication credentials");

    println!("Authentication token not found or expired, please reauthenticate.");

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
                log::warn!("failed to authenticate: {:?}", err)
            }
        }
    }

    Err(anyhow!("authentication failed!"))
}

pub async fn new_session(cache_folder: &std::path::Path) -> Result<Session> {
    let cache = Cache::new(Some(cache_folder), Some(&cache_folder.join("audio")), None)?;

    // create a new session if either
    // - there is no cached credentials or
    // - the cached credentials are expired or invalid
    match cache.credentials() {
        None => new_session_with_new_creds(&cache).await,
        Some(creds) => {
            match Session::connect(SessionConfig::default(), creds, Some(cache.clone())).await {
                Ok(session) => {
                    log::info!("use the cached credentials");
                    Ok(session)
                }
                Err(_) => new_session_with_new_creds(&cache).await,
            }
        }
    }
}
