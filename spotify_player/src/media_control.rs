use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::SharedState,
    utils::map_join,
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
    prev_track_info: &mut String,
    prev_is_playing: &mut Option<bool>,
) -> Result<(), souvlaki::Error> {
    let player = state.player.read();

    match player.current_playing_track() {
        None => {}
        Some(track) => {
            if let Some(ref playback) = player.playback {
                #[cfg(target_os = "linux")]
                {
                    // For linux, the `souvlaki` crate doesn't support updating the playback's current position
                    // and internally it only handles at most one DBus event every one second [1].
                    // To avoid possible event congestion, which can happen when the call frequency of
                    // `update_control_metadata` is higher than 1Hz (`refresh_duration < 1s`, see `start_event_watcher`),
                    // only update the media playback when the playback status (determined by `is_playing` variable below) is changed.
                    // [1]: https://github.com/Sinono3/souvlaki/blob/b4d47bb2797ffdd625c17192df640510466762e1/src/platform/linux/mod.rs#L450

                    if *prev_is_playing != Some(playback.is_playing) {
                        if playback.is_playing {
                            controls.set_playback(MediaPlayback::Playing { progress: None })?;
                        } else {
                            controls.set_playback(MediaPlayback::Paused { progress: None })?;
                        }
                    }
                }

                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    let progress = player.playback_progress().map(MediaPosition);
                    if playback.is_playing {
                        controls.set_playback(MediaPlayback::Playing { progress })?;
                    } else {
                        controls.set_playback(MediaPlayback::Paused { progress })?;
                    }
                }

                *prev_is_playing = Some(playback.is_playing);
            }

            // only update metadata when the track information is changed
            let track_info = format!("{}/{}", track.name, track.album.name);
            if track_info != *prev_track_info {
                controls.set_metadata(MediaMetadata {
                    title: Some(&track.name),
                    album: Some(&track.album.name),
                    artist: Some(&map_join(&track.artists, |a| &a.name, ", ")),
                    duration: Some(track.duration),
                    cover_url: get_track_album_image_url(track),
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

    let refresh_duration = std::time::Duration::from_millis(200);
    let mut track_info = String::new();
    let mut is_playing = None;
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

        update_control_metadata(&state, &mut controls, &mut track_info, &mut is_playing)?;
        std::thread::sleep(refresh_duration);
    }
}
