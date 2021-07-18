use std::{fmt::Display, sync::RwLockReadGuard};

use anyhow::{anyhow, Result};
use rspotify::{
    client::Spotify,
    model::*,
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
                let context = self.get_current_playing().await?;
                state.write().unwrap().current_playing_context = context;
            }
            event::Event::NextSong => {
                self.next_track().await?;
            }
            event::Event::PreviousSong => {
                self.previous_track().await?;
            }
            event::Event::TogglePlayingState => {
                let state = state.read().unwrap();
                self.toggle_playing_state(state).await?;
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

    // wrapper functions and helper functions of `rspotify` client functions

    /// converts a `rspotify` result format into `anyhow` compatible result format
    fn handle_rspotify_result<T, E: Display>(result: std::result::Result<T, E>) -> Result<T> {
        match result {
            Ok(data) => Ok(data),
            Err(err) => Err(anyhow!(format!("{}", err))),
        }
    }

    /// gets the current playing state from the application state
    fn get_current_playing_state<'a>(
        state: &'a RwLockReadGuard<'a, state::State>,
    ) -> Result<&'a context::CurrentlyPlayingContext> {
        match state.current_playing_context.as_ref() {
            Some(state) => Ok(state),
            None => Err(anyhow!("unable to get the currently playing context")),
        }
    }

    /// toggle the current playing state (pause/resume a track)
    async fn toggle_playing_state(&self, state: RwLockReadGuard<'_, state::State>) -> Result<()> {
        let state = Self::get_current_playing_state(&state)?;
        if state.is_playing {
            self.pause_track().await
        } else {
            self.resume_track().await
        }
    }

    /// resume a previously paused/played track
    async fn resume_track(&self) -> Result<()> {
        Self::handle_rspotify_result(
            self.spotify
                .start_playback(None, None, None, None, None)
                .await,
        )
    }

    /// pause currently playing track
    async fn pause_track(&self) -> Result<()> {
        Self::handle_rspotify_result(self.spotify.pause_playback(None).await)
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
    async fn get_current_playing(&self) -> Result<Option<context::CurrentlyPlayingContext>> {
        Self::handle_rspotify_result(self.spotify.current_playing(None, None).await)
    }
}
