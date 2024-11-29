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

async fn retrieve_token(
    session: &Session,
    client_id: &str,
) -> Result<librespot_core::token::Token> {
    let query_uri = format!(
        "hm://keymaster/token/authenticated?scope={}&client_id={}&device_id={}",
        SCOPES.join(","),
        client_id,
        session.device_id(),
    );
    let request = session.mercury().get(query_uri)?;
    let response = request.await?;
    let data = response
        .payload
        .first()
        .ok_or(librespot_core::token::TokenError::Empty)?
        .clone();
    let token = librespot_core::token::Token::from_json(String::from_utf8(data)?)?;
    Ok(token)
}

pub async fn get_token(session: &Session, client_id: &str) -> Result<rspotify::Token> {
    tracing::info!("Getting a new authentication token...");

    let fut = retrieve_token(session, client_id);
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
