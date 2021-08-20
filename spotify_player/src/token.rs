use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use librespot_core::keymaster;
use librespot_core::session::Session;

// spotify authentication token's scopes for permissions
const SCOPES: [&str; 11] = [
    "user-read-recently-played",
    "user-top-read",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "streaming",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-follow-read",
    "user-library-read",
];

// official spotify web app's client id
const CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";

pub async fn get_token(session: &Session) -> Result<Token> {
    Ok(keymaster::get_token(session, CLIENT_ID, &SCOPES.join(","))
        .await
        .map_err(|err| anyhow!(format!("failed to get token: {:#?}", err)))?
        .into())
}

// A spotify authentication token
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
            // `expires_at` time but earlier 5 min
            expires_at: Instant::now() + Duration::from_secs(token.expires_in as u64)
                - Duration::from_secs(5 * 60),
        }
    }
}
