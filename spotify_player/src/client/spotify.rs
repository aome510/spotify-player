use anyhow::{anyhow, Result};
use librespot_core::session::Session;
use maybe_async::maybe_async;
use rspotify::{
    clients::{mutex::Mutex, BaseClient, OAuthClient},
    http::HttpClient,
    ClientResult, Config, Credentials, OAuth, Token,
};
use std::sync::Arc;

use crate::token;

/// A wrapper struct for `librespot::Session` that implements
/// `Debug` and `Default` traits.
/// These above traits are required to implement
/// `rspotify::BaseClient` and `rspotify::OauthClient` traits.
#[derive(Clone, Default)]
pub struct SessionWrapper {
    session: Option<Session>,
}

impl std::fmt::Debug for SessionWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{ session: ... }")
    }
}

impl SessionWrapper {
    fn new(session: Session) -> Self {
        Self {
            session: Some(session),
        }
    }

    /// gets the librespot session stored inside the wrapper struct.
    /// It returns an eror if there is no such session.
    pub fn session(&self) -> Result<&Session> {
        match self.session {
            Some(ref session) => Ok(session),
            None => Err(anyhow!("failed to get the wrapped librespot session.")),
        }
    }
}

#[derive(Clone, Debug, Default)]
/// A Spotify client to interact with Spotify API server
pub struct Spotify {
    pub creds: Credentials,
    pub oauth: OAuth,
    pub config: Config,
    pub token: Arc<Mutex<Option<Token>>>,
    pub http: HttpClient,
    pub session: SessionWrapper,
    pub client_id: String,
}

impl Spotify {
    /// creates a new Spotify client
    pub fn new(session: Session, client_id: String) -> Spotify {
        Self {
            creds: Credentials::default(),
            oauth: OAuth::default(),
            config: Config {
                token_refreshing: true,
                ..Default::default()
            },
            token: Arc::new(Mutex::new(None)),
            http: HttpClient::default(),
            session: SessionWrapper::new(session),
            client_id,
        }
    }

    /// gets a Spotify access token for authorization.
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
                "failed to get the authorization token stored inside the client."
            )),
        }
    }

    /// retrieves an authorization token
    pub async fn retrieve_token(&self) -> Result<Token> {
        Ok(token::get_token(self.session.session()?, &self.client_id).await?)
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
        match self.retrieve_token().await {
            Ok(token) => Ok(Some(token)),
            Err(err) => {
                tracing::warn!("{}", err);
                Ok(None)
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

    async fn request_token(&mut self, _: &str) -> ClientResult<()> {
        panic!("`OAuthClient::request_token` should never be called!")
    }
}
