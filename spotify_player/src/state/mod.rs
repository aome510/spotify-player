mod constant;
mod data;
mod model;
mod player;
mod queue;
mod ui;

use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};

pub use constant::*;
pub use data::*;
pub use model::*;
pub use player::*;
#[allow(unused_imports)]
pub use queue::*;
pub use ui::*;

use crate::config;

pub use parking_lot::{Mutex, RwLock};

/// Application's shared state
pub type SharedState = Arc<State>;

/// Application's state
pub struct State {
    pub ui: Mutex<UIState>,
    pub player: RwLock<PlayerState>,
    pub data: RwLock<AppData>,

    pub is_daemon: bool,

    /// Shared FFT frequency-band data written by the audio sink and read by the UI.
    /// `Some` only when `enable_audio_visualization` is `true`; avoids allocating
    /// the mutex/state entirely when the feature is not in use.
    #[cfg(feature = "streaming")]
    pub vis_bands: Option<Arc<Mutex<crate::ui::streaming::VisBands>>>,

    pub logs: Arc<Mutex<VecDeque<String>>>,

    pub request_status: Mutex<RequestStatus>,
}

const REQUEST_ERROR_DISPLAY_DURATION: Duration = Duration::from_secs(10);

#[derive(Default, Debug)]
pub struct RequestStatus {
    pending: usize,
    last_error: Option<(String, Instant)>,
}

impl RequestStatus {
    pub fn start(&mut self) {
        self.pending += 1;
    }

    pub fn finish(&mut self, error: Option<String>) {
        self.pending = self.pending.saturating_sub(1);
        if let Some(error) = error {
            self.last_error = Some((error, Instant::now()));
        }
    }

    pub fn is_loading(&self) -> bool {
        self.pending > 0
    }

    pub fn recent_error(&self) -> Option<&str> {
        self.last_error
            .as_ref()
            .filter(|(_, time)| time.elapsed() < REQUEST_ERROR_DISPLAY_DURATION)
            .map(|(error, _)| error.as_str())
    }
}

impl State {
    pub fn new(is_daemon: bool, log_buffer: Arc<Mutex<VecDeque<String>>>) -> Self {
        let mut ui = UIState::default();
        let configs = config::get_config();

        if let Some(theme) = configs.theme_config.find_theme(&configs.app_config.theme) {
            // update the UI's theme based on the `theme` config option
            ui.theme = theme;
        }

        let app_data = AppData::new(&configs.cache_folder);

        Self {
            ui: Mutex::new(ui),
            player: RwLock::new(PlayerState::default()),
            data: RwLock::new(app_data),
            is_daemon,
            #[cfg(feature = "streaming")]
            vis_bands: if configs.app_config.enable_audio_visualization {
                Some(Arc::new(Mutex::new(
                    crate::ui::streaming::VisBands::default(),
                )))
            } else {
                None
            },

            logs: log_buffer,

            request_status: Mutex::new(RequestStatus::default()),
        }
    }

    #[cfg(feature = "streaming")]
    pub fn is_streaming_enabled(&self) -> bool {
        let configs = config::get_config();
        configs.app_config.enable_streaming == config::StreamingType::Always
            || (configs.app_config.enable_streaming == config::StreamingType::DaemonOnly
                && self.is_daemon)
    }

    /// Returns `true` when the custom queue system should be used for new playback.
    ///
    /// Requires streaming to be enabled and the `custom_queue` config option
    /// to be `true`.
    #[cfg(feature = "streaming")]
    #[allow(dead_code)]
    pub fn should_use_custom_queue(&self) -> bool {
        self.is_streaming_enabled() && config::get_config().app_config.custom_queue
    }

    /// Returns `true` when the local librespot player is actively streaming
    /// audio (i.e. a `Playing` event has been received and no `Paused` / `stop`
    /// has occurred since).  Used by the UI to decide whether to allocate and
    /// render the audio-visualization area.
    #[cfg(feature = "streaming")]
    pub fn is_local_streaming_active(&self) -> bool {
        self.vis_bands.as_ref().is_some_and(|b| b.lock().is_active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_status_tracks_pending_requests_and_errors() {
        let mut status = RequestStatus::default();
        assert!(!status.is_loading());

        status.start();
        status.start();
        status.finish(None);
        assert!(status.is_loading());
        assert_eq!(status.recent_error(), None);

        status.finish(Some("Failed to load playlists: 429".to_string()));
        assert!(!status.is_loading());
        assert_eq!(status.recent_error(), Some("Failed to load playlists: 429"));

        status.finish(None);
        assert!(!status.is_loading());
    }

    #[test]
    fn request_status_hides_expired_errors() {
        let status = RequestStatus {
            pending: 0,
            last_error: Instant::now()
                .checked_sub(REQUEST_ERROR_DISPLAY_DURATION)
                .map(|time| ("old error".to_string(), time)),
        };
        assert_eq!(status.recent_error(), None);
    }
}
