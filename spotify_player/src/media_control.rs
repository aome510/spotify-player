#![allow(unused_imports)]
use souvlaki::MediaPosition;
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};

use crate::utils;
use crate::{
    event::{ClientRequest, PlayerRequest},
    state::SharedState,
    utils::map_join,
};

fn update_control_metadata(
    state: &SharedState,
    controls: &mut MediaControls,
    prev_track_info: &mut String,
) -> Result<(), souvlaki::Error> {
    let player = state.player.read();

    match player.current_playing_track() {
        None => {}
        Some(track) => {
            if let Some(ref playback) = player.playback {
                let progress = player
                    .playback_progress()
                    .and_then(|p| Some(MediaPosition(p.to_std().ok()?)));

                if playback.is_playing {
                    controls.set_playback(MediaPlayback::Playing { progress })?;
                } else {
                    controls.set_playback(MediaPlayback::Paused { progress })?;
                }
            }

            // only update metadata when the track information is changed
            let track_info = format!("{}/{}", track.name, track.album.name);
            if track_info != *prev_track_info {
                controls.set_metadata(MediaMetadata {
                    title: Some(&track.name),
                    album: Some(&track.album.name),
                    artist: Some(&map_join(&track.artists, |a| &a.name, ", ")),
                    duration: track.duration.to_std().ok(),
                    cover_url: utils::get_track_album_image_url(track),
                })?;

                *prev_track_info = track_info;
            }
        }
    }

    Ok(())
}

/// Start the application's media control event watcher
pub fn start_event_watcher(
    state: SharedState,
    client_pub: flume::Sender<ClientRequest>,
) -> Result<(), souvlaki::Error> {
    tracing::info!("Initializing application's media control event watcher...");

    let hwnd = None;
    let config = PlatformConfig {
        dbus_name: "spotify_player",
        display_name: "Spotify Player",
        hwnd,
    };
    let mut controls = MediaControls::new(config)?;

    controls.attach(move |e| {
        tracing::info!("Got a media control event: {e:?}");
        match e {
            MediaControlEvent::Play | MediaControlEvent::Pause | MediaControlEvent::Toggle => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::ResumePause))
                    .unwrap_or_default();
            }
            MediaControlEvent::SetPosition(MediaPosition(dur)) => {
                if let Ok(dur) = chrono::Duration::from_std(dur) {
                    client_pub
                        .send(ClientRequest::Player(PlayerRequest::SeekTrack(dur)))
                        .unwrap_or_default();
                }
            }
            MediaControlEvent::Next => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::NextTrack))
                    .unwrap_or_default();
            }
            MediaControlEvent::Previous => {
                client_pub
                    .send(ClientRequest::Player(PlayerRequest::PreviousTrack))
                    .unwrap_or_default();
            }
            _ => {}
        }
    })?;
    // For some reason, on startup, media playback needs to be initialized with `Playing`
    // for the track metadata to be shown up on the MacOS media status bar.
    controls.set_playback(MediaPlayback::Playing { progress: None })?;

    // The below refresh duration should be no less than 1s to avoid **overloading** linux dbus
    // handler provided by the souvlaki library, which only handles an event every 1s.
    // [1]: https://github.com/Sinono3/souvlaki/blob/b4d47bb2797ffdd625c17192df640510466762e1/src/platform/linux/mod.rs#L450
    let refresh_duration = std::time::Duration::from_millis(1000);
    let mut track_info = String::new();
    loop {
        update_control_metadata(&state, &mut controls, &mut track_info)?;
        std::thread::sleep(refresh_duration);
    }
}
