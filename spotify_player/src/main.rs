mod client;
mod command;
mod config;
mod event;
mod key;
mod state;
mod token;
mod ui;
mod utils;

use anyhow::Result;
use librespot_core::{authentication::Credentials, config::SessionConfig, session::Session};
use rspotify::client::Spotify;

#[tokio::main]
async fn main() -> Result<()> {
    // disable logging by default
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    // parse command line arguments
    let matches = clap::App::new("spotify-player")
        .version("0.1.1")
        .about("A command driven spotify player")
        .author("Thang Pham <phamducthang1234@gmail>")
        .arg(
            clap::Arg::with_name("config-folder")
                .short("c")
                .long("config-folder")
                .value_name("FOLDER")
                .help("Path to the application's config folder (default: $HOME/.config/spotify-player)")
                .next_line_help(true)
        ).arg(
            clap::Arg::with_name("theme")
                .short("t")
                .long("theme")
                .value_name("THEME")
                .help("Application theme (default: dracula)")
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
        state.app_config.theme = match matches.value_of("theme") {
            Some(theme) => theme.to_owned(),
            None => state.app_config.theme,
        };
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
        let auth = config::AuthConfig::from_config_file(&config_folder)?;
        let session = Session::connect(
            SessionConfig::default(),
            Credentials::with_password(auth.username, auth.password),
            None,
        )
        .await?;
        let token = token::get_token(&session).await?;
        let spotify = Spotify::default().access_token(&token.access_token);
        state.player.write().unwrap().token = token;
        let client = client::Client::new(session, spotify);

        let state = state.clone();
        let send = send.clone();
        move || {
            client::start_watcher(state, client, send, recv);
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

    // application's UI rendering as the main thread
    send.send(event::Event::GetCurrentPlayback)?;
    ui::start_ui(state, send)
}
