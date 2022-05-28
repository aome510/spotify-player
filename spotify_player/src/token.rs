use std::collections::HashSet;

use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use librespot_core::{keymaster, session::Session};
use rspotify::Token;

/// the application authentication token's permission scopes
const SCOPES: [&str; 15] = [
    "user-read-recently-played",
    "user-top-read",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "streaming",
    "playlist-read-private",
    "playlist-modify-private",
    "playlist-modify-public",
    "playlist-read-collaborative",
    "user-follow-read",
    "user-follow-modify",
    "user-library-read",
    "user-library-modify",
];

/// gets an authentication token with pre-defined permission scopes
pub async fn get_token(session: &Session, client_id: &str) -> Result<Token> {
    tracing::info!("Getting new authentication token...");

    let token = keymaster::get_token(session, client_id, &SCOPES.join(","))
        .await
        .map_err(|err| anyhow!(format!("failed to get token: {:?}", err)))?;

    // converts the token returned by librespot `get_token` function to a `rspotify::Token`

    let expires_in = Duration::from_std(std::time::Duration::from_secs(token.expires_in as u64))?;
    let expires_at = Utc::now() + expires_in;

    let token = Token {
        access_token: token.access_token,
        expires_in,
        expires_at: Some(expires_at),
        scopes: HashSet::new(),
        refresh_token: None,
    };

    tracing::info!("Got new token: {token:?}");

    Ok(token)
}
