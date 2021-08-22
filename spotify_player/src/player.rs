use crate::{client, state};
use anyhow::{anyhow, Result};
use librespot_core::{session::Session, spotify_id::SpotifyId};
use librespot_playback::{
    audio_backend,
    config::{AudioFormat, PlayerConfig},
    player,
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

pub struct RemotePlayer {}

pub struct LocalPlayer {
    player: player::Player,
}

impl RemotePlayer {
    pub fn new() -> Self {
        Self {}
    }
}

impl LocalPlayer {
    pub fn new(session: Session) -> Self {
        let backend = audio_backend::find(None).unwrap();
        let (player, mut channel) =
            player::Player::new(PlayerConfig::default(), session, None, move || {
                backend(None, AudioFormat::default())
            });
        tokio::spawn(async move {
            while let Some(event) = channel.recv().await {
                log::info!("player event: {:?}", event);
            }
        });
        Self { player }
    }
}

pub trait Playable {
    fn next_track(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn previous_track(&mut self, client: &client::Client, state: &state::SharedState)
        -> Result<()>;
    fn resume_pause(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn seek_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        position_ms: u32,
    ) -> Result<()>;
    fn shuffle(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn repeat(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn play_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()>;
    fn play_context(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: String,
    ) -> Result<()>;
}

impl RemotePlayer {
    /// gets the current playback from the application state
    pub fn get_current_playback<'a>(
        player: &'a std::sync::RwLockReadGuard<'a, state::PlayerState>,
    ) -> Result<&'a context::CurrentlyPlaybackContext> {
        match player.playback {
            Some(ref playback) => Ok(playback),
            None => Err(anyhow!("failed to get the current playback context")),
        }
    }
}

impl Playable for RemotePlayer {
    fn next_track(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.next_track(playback)
    }

    fn previous_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
    ) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.previous_track(playback)
    }

    fn resume_pause(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.toggle_playing_state(playback)
    }

    fn seek_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        position_ms: u32,
    ) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.seek_track(playback, position_ms)
    }

    fn shuffle(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.toggle_shuffle(playback)
    }

    fn repeat(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.cycle_repeat(playback)
    }

    fn play_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.start_playback(playback, context_uri, track_uris, offset)
    }

    fn play_context(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: String,
    ) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.start_playback(playback, Some(context_uri), None, None)
    }
}

impl Playable for LocalPlayer {
    fn next_track(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }

    fn previous_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
    ) -> Result<()> {
        Ok(())
    }

    fn resume_pause(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }

    fn seek_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        position_ms: u32,
    ) -> Result<()> {
        Ok(())
    }

    fn shuffle(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }

    fn repeat(&mut self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }

    fn play_track(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
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

    fn play_context(
        &mut self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: String,
    ) -> Result<()> {
        Ok(())
    }
}
