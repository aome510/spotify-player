mod auth;
mod client;
mod command;
mod config;
#[cfg(feature = "streaming")]
mod connect;
mod event;
mod key;
mod state;
mod token;
mod ui;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // disable logging by default
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    // parse command line arguments
    let matches = clap::App::new("spotify-player")
        .version("0.2.0")
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

    let session = auth::new_session(&cache_folder).await?;

    // connection thread (used to initialize the integrated Spotify client using librespot)
    #[cfg(feature = "streaming")]
    std::thread::spawn({
        let session = session.clone();
        let device = state.app_config.device.clone();
        move || {
            connect::new_connection(session, device);
        }
    });

    // client event handler thread
    std::thread::spawn({
        let state = state.clone();
        let send = send.clone();
        let client = client::Client::new(session, &state).await?;
        move || {
            client::start_client_handler(state, client, send, recv);
        }
    });

    // terminal event handler thread
    std::thread::spawn({
        let send = send.clone();
        let state = state.clone();
        move || {
            event::start_event_handler(send, state);
        }
    });

    // player event watcher thread(s)
    client::start_player_event_watchers(state.clone(), send.clone())?;

    // application's UI as the main thread
    ui::start_ui(state)
}
