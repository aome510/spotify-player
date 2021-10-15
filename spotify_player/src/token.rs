use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use librespot::core::{keymaster, session::Session};

// the application authorization token's permission scopes
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

/// gets an authentication using the current Librespot session
pub async fn get_token(session: &Session, client_id: &str) -> Result<Token> {
    Ok(keymaster::get_token(session, client_id, &SCOPES.join(","))
        .await
        .map_err(|err| anyhow!(format!("failed to get token: {:#?}", err)))?
        .into())
}

// A spotify authorization token
#[derive(Debug)]
pub struct Token {
    pub access_token: String,
    pub expires_at: Instant,
}

impl Token {
    pub fn new() -> Self {
        Self {
            access_token: "".to_string(),
            expires_at: Instant::now(),
        }
    }
}

impl From<keymaster::Token> for Token {
    fn from(token: keymaster::Token) -> Self {
        Self {
            access_token: token.access_token,
            // `expires_at` but earlier 5 min
            expires_at: Instant::now() + Duration::from_secs(token.expires_in as u64)
                - Duration::from_secs(5 * 60),
        }
    }
}
