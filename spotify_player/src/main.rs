use anyhow::Result;
use librespot_core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
};

mod client;
mod command;
mod config;
mod event;
mod key;
mod state;
mod token;
mod ui;
mod utils;

async fn new_session_with_new_creds(
    config_folder: &std::path::Path,
    cache: Cache,
) -> Result<Session> {
    log::info!("creating a new session with new credentials");

    let auth = config::AuthConfig::from_config_file(config_folder)?;
    Ok(Session::connect(
        SessionConfig::default(),
        Credentials::with_password(auth.username, auth.password),
        Some(cache),
    )
    .await?)
}

async fn new_session(
    config_folder: &std::path::Path,
    cache_folder: &std::path::Path,
) -> Result<Session> {
    let cache = Cache::new(Some(cache_folder), Some(&cache_folder.join("audio")), None)?;

    // create a new session if either
    // - there is no cached credentials or
    // - the cached credentials are expired or invalid
    match cache.credentials() {
        None => new_session_with_new_creds(config_folder, cache).await,
        Some(creds) => {
            match Session::connect(SessionConfig::default(), creds, Some(cache.clone())).await {
                Ok(session) => {
                    log::info!("use the cached credentials");
                    Ok(session)
                }
                Err(_) => new_session_with_new_creds(config_folder, cache).await,
            }
        }
    }
}

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
            clap::Arg::with_name("cache-folder")
                .short("C")
                .long("cache-folder")
                .value_name("FOLDER")
                .help("Path to the application's cache folder (default: $HOME/.cache/spotify-player)")
                .next_line_help(true)
        )
        .arg(
            clap::Arg::with_name("theme")
                .short("t")
                .long("theme")
                .value_name("THEME")
                .help("Application theme (default: dracula)")
        )
        .get_matches();

    let config_folder = match matches.value_of("config-folder") {
        Some(path) => path.into(),
        None => config::get_config_folder_path()?,
    };
    let cache_folder = match matches.value_of("cache-folder") {
        Some(path) => path.into(),
        None => config::get_cache_folder_path()?,
    };

    if !config_folder.exists() {
        std::fs::create_dir_all(&config_folder)?;
    }
    let cache_audio_folder = cache_folder.join("audio");
    if !cache_audio_folder.exists() {
        std::fs::create_dir_all(&cache_audio_folder)?;
    }

    // init the shared application state
    let mut state = state::State::default();
    state.parse_config_files(&config_folder, matches.value_of("theme"))?;
    let state = std::sync::Arc::new(state);

    // start application's threads
    let (send, recv) = std::sync::mpsc::channel::<event::Event>();

    // client event watcher/handler thread
    std::thread::spawn({
        let state = state.clone();
        let send = send.clone();
        let session = new_session(&config_folder, &cache_folder).await?;
        let client = client::Client::new(session, &state).await?;
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
                        log::warn!("failed to send GetCurrentPlayback event: {}", err);
                    });
                std::thread::sleep(playback_refresh_duration);
            }
        });
    }

    // application's UI rendering as the main thread
    send.send(event::Event::GetCurrentPlayback)?;
    ui::start_ui(state, send)
}
