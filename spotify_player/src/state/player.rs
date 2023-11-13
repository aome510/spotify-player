use super::model::*;

/// Player state
#[derive(Default, Debug)]
pub struct PlayerState {
    pub devices: Vec<Device>,

    pub playback: Option<rspotify_model::CurrentPlaybackContext>,
    pub playback_last_updated_time: Option<std::time::Instant>,
    /// a buffered state to speedup the feedback of playback metadata update to user
    // Related issue: https://github.com/aome510/spotify-player/issues/109
    pub buffered_playback: Option<SimplifiedPlayback>,

    pub queue: Option<rspotify_model::CurrentUserQueue>,
}

impl PlayerState {
    /// gets the current playback
    ///
    /// # Note
    /// Because playback data stored inside the player state is buffered and cached,
    /// the returned playback is estimated based on the available data.
    pub fn current_playback(&self) -> Option<rspotify_model::CurrentPlaybackContext> {
        let mut playback = self.playback.clone()?;

        // update the playback's progress based on the `playback_last_updated_time`
        playback.progress = playback.progress.map(|d| {
            d + if playback.is_playing {
                chrono::Duration::from_std(self.playback_last_updated_time.unwrap().elapsed())
                    .unwrap()
            } else {
                chrono::Duration::zero()
            }
        });

        // update the playback's metadata based on the `buffered_playback` metadata
        if let Some(ref p) = self.buffered_playback {
            playback.device.name = p.device_name.clone();
            playback.device.id = p.device_id.clone();
            playback.is_playing = p.is_playing;
            playback.device.volume_percent = p.volume;
            playback.repeat_state = p.repeat_state;
            playback.shuffle_state = p.shuffle_state;
        }

        Some(playback)
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
    pub fn playback_progress(&self) -> Option<chrono::Duration> {
        match self.playback {
            None => None,
            Some(ref playback) => {
                let progress = playback.progress.unwrap()
                    + if playback.is_playing {
                        chrono::Duration::from_std(
                            self.playback_last_updated_time.unwrap().elapsed(),
                        )
                        .ok()?
                    } else {
                        chrono::Duration::zero()
                    };
                Some(progress)
            }
        }
    }

    /// gets the current playing context's ID
    pub fn playing_context_id(&self) -> Option<ContextId> {
        match self.playback {
            Some(ref playback) => match playback.context {
                Some(ref context) => {
                    let uri = crate::utils::parse_uri(&context.uri);
                    match context._type {
                        rspotify_model::Type::Playlist => Some(ContextId::Playlist(
                            PlaylistId::from_uri(&uri).ok()?.into_static(),
                        )),
                        rspotify_model::Type::Album => Some(ContextId::Album(
                            AlbumId::from_uri(&uri).ok()?.into_static(),
                        )),
                        rspotify_model::Type::Artist => Some(ContextId::Artist(
                            ArtistId::from_uri(&uri).ok()?.into_static(),
                        )),
                        _ => None,
                    }
                }
                None => None,
            },
            None => None,
        }
    }
}
