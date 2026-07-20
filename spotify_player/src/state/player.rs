use super::model::{
    AlbumId, ArtistId, ContextId, Device, PlaybackMetadata, PlaylistId, ShowId, TracksId,
};
use super::queue::CustomQueue;

/// Player state
#[derive(Default, Debug)]
pub struct PlayerState {
    pub devices: Vec<Device>,

    pub playback: Option<rspotify::model::CurrentPlaybackContext>,
    pub playback_last_updated_time: Option<std::time::Instant>,
    /// A buffered state to speedup the feedback of playback metadata update to user
    // Related issue: https://github.com/aome510/spotify-player/issues/109
    pub buffered_playback: Option<PlaybackMetadata>,

    /// Identifies the most recently started playback refresh. A refresh only applies its result
    /// while its generation remains current.
    playback_refresh_generation: u64,

    pub queue: Option<rspotify::model::CurrentUserQueue>,

    /// The currently playing Tracks context (for contexts not tracked by Spotify's playback, e.g. liked/top tracks)
    pub currently_playing_tracks_id: Option<TracksId>,

    /// App-managed custom queue for full playlist/album playback.
    /// Active when the integrated librespot player is streaming and the user
    /// started playback from a track-table context.
    pub custom_queue: Option<CustomQueue>,
}

impl PlayerState {
    pub(crate) fn begin_playback_refresh(&mut self) -> u64 {
        self.invalidate_playback_refreshes();
        self.playback_refresh_generation
    }

    pub(crate) fn invalidate_playback_refreshes(&mut self) {
        self.playback_refresh_generation = self.playback_refresh_generation.wrapping_add(1);
    }

    pub(crate) fn is_current_playback_refresh(&self, generation: u64) -> bool {
        self.playback_refresh_generation == generation
    }

    /// Get the current playback
    ///
    /// # Note
    /// Because playback metadata stored inside the player state is buffered,
    /// the returned playback is estimated based on the available data.
    pub fn current_playback(&self) -> Option<rspotify::model::CurrentPlaybackContext> {
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
            playback.device.name.clone_from(&p.device_name);
            playback.device.id.clone_from(&p.device_id);
            playback.is_playing = p.is_playing;
            playback.device.volume_percent = p.volume;
            playback.repeat_state = p.repeat_state;
            playback.shuffle_state = p.shuffle_state;
        }

        Some(playback)
    }

    pub fn currently_playing(&self) -> Option<&rspotify::model::PlayableItem> {
        self.playback.as_ref().and_then(|p| p.item.as_ref())
    }

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

    pub fn playing_context_id(&self) -> Option<ContextId> {
        match self.playback {
            Some(ref playback) => match playback.context {
                Some(ref context) => {
                    let uri = crate::utils::parse_uri(&context.uri);
                    match context._type {
                        rspotify::model::Type::Playlist => Some(ContextId::Playlist(
                            PlaylistId::from_uri(&uri).ok()?.into_static(),
                        )),
                        rspotify::model::Type::Album => Some(ContextId::Album(
                            AlbumId::from_uri(&uri).ok()?.into_static(),
                        )),
                        rspotify::model::Type::Artist => Some(ContextId::Artist(
                            ArtistId::from_uri(&uri).ok()?.into_static(),
                        )),
                        rspotify::model::Type::Show => {
                            Some(ContextId::Show(ShowId::from_uri(&uri).ok()?.into_static()))
                        }
                        _ => None,
                    }
                }
                None => self
                    .custom_queue
                    .as_ref()
                    .and_then(|q| q.source_context().cloned())
                    .or_else(|| {
                        self.currently_playing_tracks_id
                            .clone()
                            .map(ContextId::Tracks)
                    }),
            },
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlayerState;

    #[test]
    fn newer_playback_refresh_supersedes_older_refresh() {
        let mut player = PlayerState::default();
        let older = player.begin_playback_refresh();
        let newer = player.begin_playback_refresh();

        assert!(!player.is_current_playback_refresh(older));
        assert!(player.is_current_playback_refresh(newer));
    }

    #[test]
    fn player_change_invalidates_pending_playback_refresh() {
        let mut player = PlayerState::default();
        let pending = player.begin_playback_refresh();

        player.invalidate_playback_refreshes();

        assert!(!player.is_current_playback_refresh(pending));
    }
}
