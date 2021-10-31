use super::model::*;

/// Player state
#[derive(Default, Debug)]
pub struct PlayerState {
    pub devices: Vec<Device>,

    pub playback: Option<rspotify_model::CurrentPlaybackContext>,
    pub playback_last_updated: Option<std::time::Instant>,
}

impl PlayerState {
    /// gets a simplified version of the current playback
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
                        std::time::Duration::default()
                    };
                Some(progress_ms)
            }
        }
    }

    /// gets the current playing context's ID
    pub fn playing_context_id(&self) -> Option<ContextId> {
        match self.playback {
            Some(ref playback) => match playback.context {
                Some(ref context) => match context._type {
                    rspotify_model::Type::Playlist => Some(ContextId::Playlist(
                        PlaylistId::from_uri(&context.uri).expect("invalid playing context URI"),
                    )),
                    rspotify_model::Type::Album => Some(ContextId::Album(
                        AlbumId::from_uri(&context.uri).expect("invalid playing context URI"),
                    )),
                    rspotify_model::Type::Artist => Some(ContextId::Artist(
                        ArtistId::from_uri(&context.uri).expect("invalid playing context URI"),
                    )),
                    _ => None,
                },
                None => None,
            },
            None => None,
        }
    }
}
