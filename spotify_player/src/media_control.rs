use souvlaki::{MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};

/// Start the application's media control event watcher
pub fn start_event_watcher() -> Result<(), souvlaki::Error> {
    tracing::info!("Initializing application's media control event watcher...");

    let hwnd = None;
    let config = PlatformConfig {
        display_name: "spotify_player",
        dbus_name: "Spotify Player",
        hwnd,
    };
    let mut controls = MediaControls::new(config)?;

    controls.attach(|e| {
        tracing::info!("Got media event: {e:?}");
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
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}
