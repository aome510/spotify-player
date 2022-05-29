use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::SharedState,
};

fn get_track_album_image_url(track: &rspotify::model::FullTrack) -> Option<&str> {
    if track.album.images.is_empty() {
        None
    } else {
        Some(&track.album.images[0].url)
    }
}

fn update_control_metadata(
    state: &SharedState,
    controls: &mut MediaControls,
) -> Result<(), souvlaki::Error> {
    let player = state.player.read();

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
                cover_url: get_track_album_image_url(track),
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
        dbus_name: "spotify_player",
        display_name: "Spotify Player",
        hwnd,
    };
    let mut controls = MediaControls::new(config)?;

    let (tx, rx) = std::sync::mpsc::sync_channel(16);

    controls.attach(move |e| {
        tx.send(e).unwrap_or_default();
    })?;
    // Somehow, on startup, media playback needs to be initialized with `Playing`
    // for the track metadata to be shown up on the MacOS media status bar.
    controls.set_playback(MediaPlayback::Playing { progress: None })?;

    // `100ms` is a "good enough" duration for the track metadata to be updated consistently.
    // Setting this to be higher (e.g, `200ms`) would result in incorrect metadata
    // shown up in the media status bar occasionally.
    let refresh_duration = std::time::Duration::from_millis(100);
    loop {
        if let Ok(event) = rx.try_recv() {
            tracing::info!("Got a media control event: {event:?}");
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
        std::thread::sleep(refresh_duration);
    }
}
