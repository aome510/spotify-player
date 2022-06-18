#[cfg(feature = "image")]
use crate::utils;

use super::model::*;

/// Player state
#[derive(Default, Debug)]
pub struct PlayerState {
    pub devices: Vec<Device>,

    pub playback: Option<rspotify_model::CurrentPlaybackContext>,
    pub playback_last_updated_time: Option<std::time::Instant>,
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

    #[cfg(feature = "image")]
    /// gets the current playing track's album cover URL
    pub fn current_playing_track_album_cover_url(&self) -> Option<&str> {
        self.current_playing_track()
            .and_then(utils::get_track_album_image_url)
    }

    /// gets the current playback progress
    pub fn playback_progress(&self) -> Option<std::time::Duration> {
        match self.playback {
            None => None,
            Some(ref playback) => {
                let progress = playback.progress.unwrap()
                    + if playback.is_playing {
                        self.playback_last_updated_time.unwrap().elapsed()
                    } else {
                        // zero duration
                        std::time::Duration::default()
                    };
                Some(progress)
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
