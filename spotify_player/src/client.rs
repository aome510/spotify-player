use std::fmt::Display;

use anyhow::{anyhow, Result};
use rspotify::{
    client::Spotify,
    model::context::CurrentlyPlayingContext,
    oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo},
    util::get_token,
};

use crate::event;
use crate::state;

/// A spotify client
pub struct Client {
    spotify: Spotify,
    oauth: SpotifyOAuth,
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
    pub async fn handle_event(
        &mut self,
        state: &state::SharedState,
        event: event::Event,
    ) -> Result<()> {
        match event {
            event::Event::RefreshToken => {
                self.refresh_token().await?;
            }
            event::Event::GetCurrentPlayingContext => {
                let context = self.get_currently_playing().await?;
                state.write().unwrap().current_playing_context = context;
            }
            event::Event::NextSong => {
                self.next_track().await?;
            }
            event::Event::PreviousSong => {
                self.previous_track().await?;
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

    fn handle_rspotify_result<T, E: Display>(result: std::result::Result<T, E>) -> Result<T> {
        match result {
            Ok(data) => Ok(data),
            Err(err) => Err(anyhow!(format!("{}", err))),
        }
    }

    /// skips to the next track
    async fn next_track(&self) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.next_track(None).await)
    }

    /// skips to the previous track
    async fn previous_track(&self) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.previous_track(None).await)
    }

    /// returns the current playing context
    async fn get_currently_playing(&self) -> Result<Option<CurrentlyPlayingContext>> {
        Self::handle_rspotify_result(self.spotify.current_playing(None, None).await)
    }
}
