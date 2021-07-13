use anyhow::{anyhow, Result};
use rspotify::{
    client::Spotify,
    model::context::CurrentlyPlayingContext,
    oauth2::{SpotifyClientCredentials, SpotifyOAuth, TokenInfo},
    util::get_token,
};

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

    pub async fn new(oauth: SpotifyOAuth) -> Result<Self> {
        let mut client = Self {
            spotify: Spotify::default(),
            oauth,
        };
        client.refresh_token().await?;
        Ok(client)
    }

    pub fn handle_error(&self, err: anyhow::Error) {
        log::error!("client error: {:#}", err);
    }

    pub async fn refresh_token(&mut self) -> Result<()> {
        let token = match get_token(&mut self.oauth).await {
            Some(token) => token,
            None => return Err(anyhow!("auth failed")),
        };

        self.spotify = Self::get_spotify_client(token);
        Ok(())
    }

    pub async fn get_currently_playing(&self) -> Option<CurrentlyPlayingContext> {
        let result = self.spotify.current_playing(None, None).await;
        match result {
            Ok(context) => context,
            Err(err) => {
                self.handle_error(anyhow!(err));
                None
            }
        }
    }
}
