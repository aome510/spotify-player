use crate::{client, state};
use anyhow::{anyhow, Result};
use librespot_core::{session::Session, spotify_id::SpotifyId};
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, PlayerConfig},
    player::{self, PlayerEventChannel},
};
use rspotify::model::{context, offset};

pub enum Player {
    Remote(RemotePlayer),
    Local(LocalPlayer),
}

impl Player {
    pub fn get_player(&mut self) -> &mut dyn Playable {
        match self {
            Self::Remote(ref mut player) => player,
            Self::Local(ref mut player) => player,
        }
    }
}

pub struct RemotePlayer {
    state: state::SharedState,
}

pub struct LocalPlayer {
    player: player::Player,
    state: state::SharedState,
}

impl RemotePlayer {
    pub fn new(state: state::SharedState) -> Self {
        Self { state }
    }
}

impl LocalPlayer {
    pub fn new(session: Session, state: state::SharedState) -> Self {
        let backend = audio_backend::find(None).unwrap();
        let (player, channel) =
            player::Player::new(PlayerConfig::default(), session, None, move || {
                backend(None, AudioFormat::default())
            });
        std::thread::spawn({
            let state = state.clone();
            move || handle_local_player_event(channel, state)
        });
        Self { player, state }
    }
}

pub trait Playable {
    fn next_track(&mut self, client: &client::Client) -> Result<()>;
    fn previous_track(&mut self, client: &client::Client) -> Result<()>;
    fn resume_pause(&mut self, client: &client::Client) -> Result<()>;
    fn seek_track(&mut self, client: &client::Client, position_ms: u32) -> Result<()>;
    fn shuffle(&mut self, client: &client::Client) -> Result<()>;
    fn repeat(&mut self, client: &client::Client) -> Result<()>;
    fn play_track(
        &mut self,
        client: &client::Client,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()>;
    fn play_context(&mut self, client: &client::Client, context_uri: String) -> Result<()>;
}

impl RemotePlayer {
    /// gets player's current playback
    pub fn get_current_playback(&self) -> Result<context::CurrentlyPlaybackContext> {
        match self.state.player.read().unwrap().playback {
            Some(ref playback) => Ok(playback.clone()),
            None => Err(anyhow!("failed to get the current playback context")),
        }
    }
}

impl Playable for RemotePlayer {
    fn next_track(&mut self, client: &client::Client) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.next_track(&playback)
    }

    fn previous_track(&mut self, client: &client::Client) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.previous_track(&playback)
    }

    fn resume_pause(&mut self, client: &client::Client) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.toggle_playing_state(&playback)
    }

    fn seek_track(&mut self, client: &client::Client, position_ms: u32) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.seek_track(&playback, position_ms)
    }

    fn shuffle(&mut self, client: &client::Client) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.toggle_shuffle(&playback)
    }

    fn repeat(&mut self, client: &client::Client) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.cycle_repeat(&playback)
    }

    fn play_track(
        &mut self,
        client: &client::Client,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.start_playback(&playback, context_uri, track_uris, offset)
    }

    fn play_context(&mut self, client: &client::Client, context_uri: String) -> Result<()> {
        let playback = self.get_current_playback()?;
        client.start_playback(&playback, Some(context_uri), None, None)
    }
}

impl Playable for LocalPlayer {
    fn next_track(&mut self, client: &client::Client) -> Result<()> {
        Ok(())
    }

    fn previous_track(&mut self, client: &client::Client) -> Result<()> {
        Ok(())
    }

    fn resume_pause(&mut self, client: &client::Client) -> Result<()> {
        Ok(())
    }

    fn seek_track(&mut self, client: &client::Client, position_ms: u32) -> Result<()> {
        Ok(())
    }

    fn shuffle(&mut self, client: &client::Client) -> Result<()> {
        Ok(())
    }

    fn repeat(&mut self, client: &client::Client) -> Result<()> {
        Ok(())
    }

    fn play_track(
        &mut self,
        client: &client::Client,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()> {
        if let Some(offset) = offset {
            if let Some(uri) = offset.uri {
                let uri = uri.split(':').collect::<Vec<_>>()[2];
                log::info!("play track with uri: {}", uri);
                self.player
                    .load(SpotifyId::from_base62(uri).unwrap(), true, 0);
                self.player.play();
            }
        }
        Ok(())
    }

    fn play_context(&mut self, client: &client::Client, context_uri: String) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn handle_local_player_event(mut channel: PlayerEventChannel, state: state::SharedState) {
    while let Some(event) = channel.recv().await {
        log::info!("player event: {:?}", event);
    }
}
