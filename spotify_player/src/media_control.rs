use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::SharedState,
};

fn update_control_metadata(
    state: &SharedState,
    controls: &mut MediaControls,
) -> Result<(), souvlaki::Error> {
    let player = state.player.read();

    tracing::info!("update media control metadata...",);

    match player.current_playing_track() {
        None => {}
        Some(track) => {
            if let Some(ref playback) = player.playback {
                let progress = player.playback_progress().map(MediaPosition);
                if playback.is_playing {
                    controls.set_playback(MediaPlayback::Playing { progress })?;
                } else {
                    controls.set_playback(MediaPlayback::Paused { progress })?;
                }
            }

            controls.set_metadata(MediaMetadata {
                title: Some(&track.name),
                album: Some(&track.album.name),
                artist: Some(
                    &track
                        .artists
                        .iter()
                        .map(|a| &a.name)
                        .fold(String::new(), |x, y| {
                            if x.is_empty() {
                                x + y
                            } else {
                                x + ", " + y
                            }
                        }),
                ),
                duration: Some(track.duration),
                cover_url: None,
            })?;
        }
    }

    Ok(())
}

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

    loop {
        if let Ok(event) = rx.try_recv() {
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

        update_control_metadata(&state, &mut controls)?;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
