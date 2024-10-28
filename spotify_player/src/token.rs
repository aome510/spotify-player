use std::collections::HashSet;

use anyhow::Result;
use chrono::{Duration, Utc};
use librespot_core::session::Session;
use rspotify::Token;

use crate::auth::OAUTH_SCOPES;

const TIMEOUT_IN_SECS: u64 = 5;

/// gets an authentication token with pre-defined permission scopes
pub async fn get_token(session: &Session) -> Result<Token> {
    tracing::info!("Getting new authentication token...");

    let scopes = OAUTH_SCOPES.join(",");
    let fut = session.token_provider().get_token(&scopes);
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
