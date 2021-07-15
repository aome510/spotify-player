use anyhow::{anyhow, Result};
use rspotify::{
    client::Spotify,
    model::context::CurrentlyPlayingContext,
    oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo},
    util::get_token,
};

use crate::state;

/// A spotify client
pub struct Client {
    spotify: Spotify,
    oauth: SpotifyOAuth,
}

pub enum Event {
    RefreshToken,
    GetCurrentPlayingContext,
}

impl Client {
    fn get_spotify_client(token: TokenInfo) -> Spotify {
        let client_credential = SpotifyClientCredentials::default()
            .token_info(token)
            .build();
        Spotify::default()
            .client_credentials_manager(client_credential)
            .build()
    }

    /// returns the new `Client`
    pub fn new(oauth: SpotifyOAuth) -> Self {
        Self {
            spotify: Spotify::default(),
            oauth,
        }
    }

    /// handles a client event
    pub async fn handle_event(&mut self, state: &state::SharedState, event: Event) -> Result<()> {
        match event {
            Event::RefreshToken => {
                self.refresh_token().await?;
            }
            Event::GetCurrentPlayingContext => {
                let context = self.get_currently_playing().await?;
                state.write().unwrap().current_playing_context = context;
            }
        }
        Ok(())
    }

    /// handles a client error
    pub fn handle_error(&self, err: anyhow::Error) {
        log::warn!("client error: {:#}", err);
    }

    /// refreshes the client's authentication token, returns
    /// the token's `expires_at` time.
    pub async fn refresh_token(&mut self) -> Result<std::time::SystemTime> {
        let token = match get_token(&mut self.oauth).await {
            Some(token) => token,
            None => return Err(anyhow!("auth failed")),
        };

        let expires_at = token
            .expires_at
            .expect("got `None` for token's `expires_at`");
        self.spotify = Self::get_spotify_client(token);
        Ok(
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(expires_at as u64)
                - std::time::Duration::from_secs(10),
        )
    }

    /// returns the current playing context
    async fn get_currently_playing(&self) -> Result<Option<CurrentlyPlayingContext>> {
        let result = self.spotify.current_playing(None, None).await;
        match result {
            Ok(context) => Ok(context),
            Err(err) => Err(anyhow!(format!(
                "failed to get currently playing context {:#?}",
                err
            ))),
        }
    }
}
