mod client;
mod command;
mod config;
mod event;
mod key;
mod state;
mod ui;
mod utils;

use anyhow::{anyhow, Result};
use rspotify::model::PlayingItem;

// spotify authentication token's scopes for permissions
const SCOPES: [&str; 10] = [
    "user-read-recently-played",
    "user-top-read",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "streaming",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
];

async fn init_state(client: &mut client::Client, state: &state::SharedState) -> Result<()> {
    state.player.write().unwrap().auth_token_expires_at = client.refresh_token().await?;

    let devices = client.get_devices().await?;
    if devices.is_empty() {
        return Err(anyhow!(
            "no active device available. Please connect to one and try again."
        ));
    }
    state.player.write().unwrap().devices = devices;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // disable logging by default
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    // parse command line arguments
    let matches = clap::App::new("spotify-player")
        .version("0.1.0")
        .author("Thang Pham <phamducthang1234@gmail>")
        .arg(
            clap::Arg::with_name("config-folder")
                .short("c")
                .long("config-folder")
                .value_name("FOLDER")
                .help("Path to the application's config folder (default: $HOME/.config/spotify-player)")
                .next_line_help(true),
        )
        .get_matches();

    // parsing config files
    let mut state = state::State::default();
    let config_folder = match matches.value_of("config-folder") {
        Some(path) => path.into(),
        None => config::get_config_folder_path()?,
    };
    {
        state.app_config.parse_config_file(&config_folder)?;
        log::info!("app configuartions: {:#?}", state.app_config);

        state.theme_config.parse_config_file(&config_folder)?;
        if let Some(theme) = state.theme_config.find_theme(&state.app_config.theme) {
            state.ui.lock().unwrap().theme = theme;
        }
        log::info!("theme configuartions: {:#?}", state.theme_config);

        state.keymap_config.parse_config_file(&config_folder)?;
        log::info!("keymap configuartions: {:#?}", state.keymap_config);
    }

    // init the shared state
    let state = std::sync::Arc::new(state);

    // start application's threads
    let (send, recv) = std::sync::mpsc::channel::<event::Event>();

    // client event watcher/handler thread
    std::thread::spawn({
        let client_config = config::ClientConfig::from_config_file(&config_folder)?;

        let oauth = rspotify::oauth2::SpotifyOAuth::default()
            .client_id(&client_config.client_id)
            .client_secret(&client_config.client_secret)
            .redirect_uri("http://localhost:8888/callback")
            .cache_path(config::get_token_cache_file_path(&config_folder))
            .scope(&SCOPES.join(" "))
            .build();

        let mut client = client::Client::new(oauth);
        // init the application's state
        init_state(&mut client, &state).await?;

        let state = state.clone();
        move || {
            client::start_watcher(state, client, recv);
        }
    });
    // terminal event streaming thread
    std::thread::spawn({
        let send = send.clone();
        let state = state.clone();
        move || {
            event::start_event_stream(send, state);
        }
    });
    if state.app_config.playback_refresh_duration_in_ms > 0 {
        // playback pooling (every `playback_refresh_duration_in_ms` ms) thread
        std::thread::spawn({
            let send = send.clone();
            let playback_refresh_duration =
                std::time::Duration::from_millis(state.app_config.playback_refresh_duration_in_ms);
            move || loop {
                send.send(event::Event::GetCurrentPlayback)
                    .unwrap_or_else(|err| {
                        log::warn!("failed to send GetCurrentPlayback event: {:#?}", err);
                    });
                std::thread::sleep(playback_refresh_duration);
            }
        });
    }
    std::thread::spawn({
        // update the current playback if the currently playing track ends
        let state = state.clone();
        let send = send.clone();
        let delay = std::time::Duration::from_millis(state.app_config.playback_update_delay_in_ms);
        move || loop {
            let progress_ms = state.player.read().unwrap().get_playback_progress();
            let duration_ms = match state.player.read().unwrap().playback {
                Some(ref playback) => match playback.item {
                    Some(PlayingItem::Track(ref track)) => Some(track.duration_ms),
                    _ => None,
                },
                None => None,
            };
            if let Some(progress_ms) = progress_ms {
                if progress_ms == duration_ms.unwrap() {
                    send.send(event::Event::GetCurrentPlayback)
                        .unwrap_or_else(|err| {
                            log::warn!("failed to send GetCurrentPlayback event: {:#?}", err);
                        });
                }
            }
            std::thread::sleep(delay);
        }
    });

    // application's UI rendering as the main thread
    send.send(event::Event::GetCurrentPlayback)?;
    ui::start_ui(state, send)
}
