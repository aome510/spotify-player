use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::SharedState,
};

/// Start the application's media control event watcher
pub fn start_event_watcher(
    state: SharedState,
    client_pub: tokio::sync::mpsc::Sender<ClientRequest>,
) -> Result<(), souvlaki::Error> {
    tracing::info!("Initializing application's media control event watcher...");

    let hwnd = None;
    let config = PlatformConfig {
        display_name: "spotify_player",
        dbus_name: "Spotify Player",
        hwnd,
    };
    let mut controls = MediaControls::new(config)?;

    let (tx, rx) = std::sync::mpsc::sync_channel(16);

    controls.attach(move |e| {
        tx.send(e).unwrap_or_default();
    })?;
    controls.set_playback(MediaPlayback::Playing { progress: None })?;
    controls
        .set_metadata(MediaMetadata {
            title: Some("When The Sun Hits"),
            album: Some("Souvlaki"),
            artist: Some("Slowdive"),
            duration: Some(std::time::Duration::from_secs_f64(4.0 * 60.0 + 50.0)),
            cover_url: Some("https://c.pxhere.com/photos/34/c1/souvlaki_authentic_greek_greek_food_mezes-497780.jpg!d"),
        })?;

    loop {
        if let Ok(event) = rx.recv() {
            tracing::info!("got a media control event: {event:?}");
            match event {
                MediaControlEvent::Play | MediaControlEvent::Pause | MediaControlEvent::Toggle => {
                    client_pub
                        .blocking_send(ClientRequest::Player(PlayerRequest::ResumePause))
                        .unwrap_or_default();
                }
                MediaControlEvent::Next => {
                    client_pub
                        .blocking_send(ClientRequest::Player(PlayerRequest::NextTrack))
                        .unwrap_or_default();
                }
                MediaControlEvent::Previous => {
                    client_pub
                        .blocking_send(ClientRequest::Player(PlayerRequest::PreviousTrack))
                        .unwrap_or_default();
                }
                _ => {}
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}
