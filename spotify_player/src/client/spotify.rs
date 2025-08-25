use anyhow::{anyhow, Result};
use librespot_core::session::Session;
use maybe_async::maybe_async;
use rspotify::{
    clients::{BaseClient, OAuthClient},
    http::HttpClient,
    sync::Mutex,
    ClientResult, Config, Credentials, OAuth, Token,
};
use std::{fmt, sync::Arc};

use crate::token;

#[derive(Clone, Default)]
/// A Spotify client to interact with Spotify API server
pub struct Spotify {
    creds: Credentials,
    oauth: OAuth,
    config: Config,
    token: Arc<Mutex<Option<Token>>>,
    http: HttpClient,
    pub(crate) session: Arc<tokio::sync::Mutex<Option<Session>>>,
}

#[allow(clippy::missing_fields_in_debug)] // Seems like not all fields are necessary in debug
impl fmt::Debug for Spotify {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spotify")
            .field("creds", &self.creds)
            .field("oauth", &self.oauth)
            .field("config", &self.config)
            .field("token", &self.token)
            .finish()
    }
}

impl Spotify {
    /// Create a new Spotify client
    pub fn new() -> Spotify {
        Self {
            creds: Credentials::default(),
            oauth: OAuth::default(),
            config: Config {
                token_refreshing: true,
                ..Default::default()
            },
            token: Arc::new(Mutex::new(None)),
            http: HttpClient::default(),
            session: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub async fn session(&self) -> Session {
        self.session
            .lock()
            .await
            .clone()
            .expect("non-empty Spotify session")
    }

    /// Get a Spotify access token.
    /// The function may retrieve a new token and update the current token
    /// stored inside the client if the old one is expired.
    pub async fn access_token(&self) -> Result<String> {
        let should_update = match self.token.lock().await.unwrap().as_ref() {
            Some(token) => token.is_expired(),
            None => true,
        };
        if should_update {
            self.refresh_token().await?;
        }

        match self.token.lock().await.unwrap().as_ref() {
            Some(token) => Ok(token.access_token.clone()),
            None => Err(anyhow!(
                "failed to get the authentication token stored inside the client."
            )),
        }
    }
}

// TODO: remove the below uses of `maybe_async` crate once
// async trait is fully supported in stable Rust.

#[maybe_async]
impl BaseClient for Spotify {
    fn get_http(&self) -> &HttpClient {
        &self.http
    }

    fn get_token(&self) -> Arc<Mutex<Option<Token>>> {
        Arc::clone(&self.token)
    }

    fn get_creds(&self) -> &Credentials {
        &self.creds
    }

    fn get_config(&self) -> &Config {
        &self.config
    }

    async fn refetch_token(&self) -> ClientResult<Option<Token>> {
        let session = self.session().await;
        let old_token = self.token.lock().await.unwrap().clone();

        if session.is_invalid() {
            tracing::error!("Failed to get a new token: invalid session");
            return Ok(old_token);
        }

        match token::get_token_rspotify(&session).await {
            Ok(token) => Ok(Some(token)),
            Err(err) => {
                tracing::error!("Failed to get a new token: {err:#}");
                Ok(old_token)
            }
        }
    }
}

/// Implement `OAuthClient` trait for `Spotify` struct
/// to allow calling methods that get/modify user's data such as
/// `current_user_playlists`, `playlist_add_items`, etc.
///
/// Because the `Spotify` client interacts with Spotify APIs
/// using an access token that is manually retrieved by
/// the `librespot::get_token` function, implementing
/// `OAuthClient::get_oauth` and `OAuthClient::request_token` is unnecessary
#[maybe_async]
impl OAuthClient for Spotify {
    fn get_oauth(&self) -> &OAuth {
        panic!("`OAuthClient::get_oauth` should never be called!")
    }

    async fn request_token(&self, _code: &str) -> ClientResult<()> {
        panic!("`OAuthClient::request_token` should never be called!")
    }
}
