use super::model::*;

/// Player state
#[derive(Debug)]
pub struct PlayerState {
    pub context_id: Option<ContextId>,

    pub playback: Option<rspotify_model::CurrentPlaybackContext>,
    pub playback_last_updated: Option<std::time::Instant>,
}

impl PlayerState {
    /// gets a simplified playback
    pub fn simplified_playback(&self) -> Option<SimplifiedPlayback> {
        self.playback.as_ref().map(|p| SimplifiedPlayback {
            device_id: p.device.id.clone(),
            is_playing: p.is_playing,
            repeat_state: p.repeat_state,
            shuffle_state: p.shuffle_state,
        })
    }

    /// gets the current playing track
    pub fn current_playing_track(&self) -> Option<&rspotify_model::FullTrack> {
        match self.playback {
            None => None,
            Some(ref playback) => match playback.item {
                Some(rspotify::model::PlayableItem::Track(ref track)) => Some(track),
                _ => None,
            },
        }
    }

    /// gets the current playback progress
    pub fn playback_progress(&self) -> Option<std::time::Duration> {
        match self.playback {
            None => None,
            Some(ref playback) => {
                let progress_ms = playback.progress.unwrap()
                    + if playback.is_playing {
                        std::time::Instant::now()
                            .saturating_duration_since(self.playback_last_updated.unwrap())
                    } else {
                        time::Duration::default()
                    };
                Some(progress_ms)
            }
        }
    }

    /// gets the current context
    pub fn context(&self) -> Option<&Context> {
        match self.context_id {
            Some(ref id) => self.context_cache.peek(&id.uri()),
            None => None,
        }
    }

    /// gets the current context (mutable)
    pub fn context_mut(&mut self) -> Option<&mut Context> {
        match self.context_id {
            Some(ref id) => self.context_cache.peek_mut(&id.uri()),
            None => None,
        }
    }
}
impl Default for PlayerState {
    fn default() -> Self {
        Self {
            context_id: None,
            playback: None,
            playback_last_updated: None,
        }
    }
}
