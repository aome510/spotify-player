use std::collections::HashSet;

use anyhow::Result;
use chrono::{Duration, Utc};
use librespot_core::session::Session;

const TIMEOUT_IN_SECS: u64 = 5;

/// The application authentication token's permission scopes
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

pub async fn get_token_librespot(
    session: &Session,
    _client_id: &str,
) -> Result<librespot_core::token::Token> {
    let auth_data = session.auth_data();
    if auth_data.is_empty() {
        anyhow::bail!("Session has no stored credentials for login5 token acquisition");
    }

    let token = session.login5().auth_token().await.unwrap();
    Ok(token)
}

pub async fn get_token_rspotify(session: &Session, client_id: &str) -> Result<rspotify::Token> {
    tracing::info!("Getting a new authentication token...");

    let fut = get_token_librespot(session, client_id);
    let token =
        match tokio::time::timeout(std::time::Duration::from_secs(TIMEOUT_IN_SECS), fut).await {
            Ok(Ok(token)) => token,
            Ok(Err(err)) => anyhow::bail!("failed to get the token: {:?}", err),
            Err(_) => {
                // The timeout likely happens because of the "corrupted" session,
                // shutdown it to force re-initializing.
                if !session.is_invalid() {
                    session.shutdown();
                }
                anyhow::bail!("timeout when getting the token");
            }
        };

    // converts the token returned by librespot `get_token` function to a `rspotify::Token`

    let expires_in = Duration::from_std(token.expires_in)?;
    // let expires_in = Duration::from_std(std::time::Duration::from_secs(5))?;
    let expires_at = Utc::now() + expires_in;

    let token = rspotify::Token {
        access_token: token.access_token,
        expires_in,
        expires_at: Some(expires_at),
        scopes: HashSet::new(),
        refresh_token: None,
    };

    tracing::info!("Got new token: {token:?}");

    Ok(token)
}
