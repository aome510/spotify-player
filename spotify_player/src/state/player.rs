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

    pub queue: Option<rspotify::model::CurrentUserQueue>,

    /// The currently playing Tracks context (for contexts not tracked by Spotify's playback, e.g. liked/top tracks)
    pub currently_playing_tracks_id: Option<TracksId>,

    /// App-managed custom queue for full playlist/album playback.
    /// Active when the integrated librespot player is streaming and the user
    /// started playback from a track-table context.
    pub custom_queue: Option<CustomQueue>,

    /// Rate-limiting state for volume requests, see [`VolumeSync`].
    pub volume_sync: VolumeSync,
}

/// Tracks in-flight volume changes so that rapid keypresses stay responsive.
///
/// A volume command used to read the volume out of `buffered_playback`, which is only refreshed
/// *after* the Spotify request completes. Mashing a volume key therefore made every press compute
/// its target from the same stale value, so a burst of presses collapsed into a single step.
///
/// Instead the key handler now updates `buffered_playback` optimistically (so the UI and the next
/// keypress both see the new level immediately) and records the desired level here. Requests are
/// dispatched at most once per [`VOLUME_SEND_INTERVAL`], with the trailing value flushed by the
/// player event watcher.
#[derive(Default, Debug)]
pub struct VolumeSync {
    /// A volume the user has dialed in that has not been sent to Spotify yet.
    pub pending: Option<u8>,
    /// When a volume request was last dispatched.
    pub last_sent: Option<std::time::Instant>,
}

/// Minimum spacing between volume requests sent to Spotify.
pub const VOLUME_SEND_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

impl VolumeSync {
    /// Whether a request may be dispatched right now.
    pub fn may_send(&self) -> bool {
        match self.last_sent {
            Some(t) => t.elapsed() >= VOLUME_SEND_INTERVAL,
            None => true,
        }
    }

    /// Marks a request for `volume` as dispatched.
    pub fn mark_sent(&mut self) {
        self.pending = None;
        self.last_sent = Some(std::time::Instant::now());
    }
}

impl PlayerState {
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

    /// The volume a further adjustment should be applied to.
    ///
    /// While muted, `volume` holds the pre-mute level, so adjusting from it means unmuting lands
    /// on a level relative to what the user last heard.
    pub fn effective_volume(&self) -> Option<u8> {
        let playback = self.buffered_playback.as_ref()?;
        let volume = playback.mute_state.or(playback.volume)?;
        Some(u8::try_from(volume.min(100)).unwrap_or(100))
    }

    /// Applies `volume` to the buffered playback immediately, without waiting for Spotify to
    /// confirm it, and queues it for dispatch. Returns the volume to send now, if any.
    ///
    /// Applying locally first is what makes a burst of keypresses accumulate correctly: each press
    /// reads the level the previous one just set instead of the last server-confirmed value.
    pub fn apply_volume(&mut self, volume: u8) -> Option<u8> {
        let volume = volume.min(100);
        let playback = self.buffered_playback.as_mut()?;
        playback.volume = Some(u32::from(volume));
        // Changing the volume takes the playback out of the muted state.
        playback.mute_state = None;

        if self.volume_sync.may_send() {
            self.volume_sync.mark_sent();
            Some(volume)
        } else {
            self.volume_sync.pending = Some(volume);
            None
        }
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
