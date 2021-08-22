use crate::{client, state};
use anyhow::{anyhow, Result};
use rspotify::model::{context, offset};

#[derive(Debug)]
pub enum Player {
    Remote(RemotePlayer),
    Local(LocalPlayer),
}

impl Player {
    pub fn get_player(&self) -> &dyn Playable {
        match self {
            Self::Remote(ref player) => player,
            Self::Local(ref player) => player,
        }
    }
}

#[derive(Debug)]
pub struct RemotePlayer {}
#[derive(Debug)]
pub struct LocalPlayer {}

pub trait Playable {
    fn next_track(&self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn previous_track(&self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn resume_pause(&self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn seek_track(
        &self,
        client: &client::Client,
        state: &state::SharedState,
        position_ms: u32,
    ) -> Result<()>;
    fn shuffle(&self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn repeat(&self, client: &client::Client, state: &state::SharedState) -> Result<()>;
    fn play_track(
        &self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()>;
    fn play_context(
        &self,
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
    fn next_track(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.next_track(playback)
    }

    fn previous_track(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.previous_track(playback)
    }

    fn resume_pause(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.toggle_playing_state(playback)
    }

    fn seek_track(
        &self,
        client: &client::Client,
        state: &state::SharedState,
        position_ms: u32,
    ) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.seek_track(playback, position_ms)
    }

    fn shuffle(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.toggle_shuffle(playback)
    }

    fn repeat(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        let state = state.player.read().unwrap();
        let playback = RemotePlayer::get_current_playback(&state)?;
        client.cycle_repeat(playback)
    }

    fn play_track(
        &self,
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
        &self,
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
    fn next_track(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }
    fn previous_track(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }
    fn resume_pause(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }
    fn seek_track(
        &self,
        client: &client::Client,
        state: &state::SharedState,
        position_ms: u32,
    ) -> Result<()> {
        Ok(())
    }
    fn shuffle(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }
    fn repeat(&self, client: &client::Client, state: &state::SharedState) -> Result<()> {
        Ok(())
    }
    fn play_track(
        &self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: Option<String>,
        track_uris: Option<Vec<String>>,
        offset: Option<offset::Offset>,
    ) -> Result<()> {
        Ok(())
    }
    fn play_context(
        &self,
        client: &client::Client,
        state: &state::SharedState,
        context_uri: String,
    ) -> Result<()> {
        Ok(())
    }
}
