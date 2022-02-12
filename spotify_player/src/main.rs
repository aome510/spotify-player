mod auth;
mod client;
mod command;
mod config;
mod event;
mod key;
#[cfg(feature = "streaming")]
mod spirc;
mod state;
mod token;
mod ui;
mod utils;

fn init_app_cli_arguments() -> clap::ArgMatches {
    clap::App::new("spotify-player")
        .version("0.5.2")
        .about("A command driven spotify player")
        .author("Thang Pham <phamducthang1234@gmail>")
        .arg(
            clap::Arg::new("theme")
                .short('t')
                .long("theme")
                .value_name("THEME")
                .help("Application theme (default: dracula)")
        )
        .arg(
            clap::Arg::new("config-folder")
                .short('c')
                .long("config-folder")
                .value_name("FOLDER")
                .help("Path to the application's config folder (default: $HOME/.config/spotify-player)")
                .next_line_help(true)
        )
        .arg(
            clap::Arg::new("cache-folder")
                .short('C')
                .long("cache-folder")
                .value_name("FOLDER")
                .help("Path to the application's cache folder (default: $HOME/.cache/spotify-player)")
                .next_line_help(true)
        )
        .get_matches()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // parse command line arguments
    let args = init_app_cli_arguments();

    // initialize the application's cache folders and config folder
    let config_folder = match args.value_of("config-folder") {
        Some(path) => path.into(),
        None => config::get_config_folder_path()?,
    };
    let cache_folder = match args.value_of("cache-folder") {
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

    // initialize the application's logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    let log_file = std::fs::File::create(cache_folder.join("spotify-player.log"))?;
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    // initialize the application state
    let mut state = state::State::default();
    // parse config options from the config files into application's state
    state.parse_config_files(&config_folder, args.value_of("theme"))?;
    let state = std::sync::Arc::new(state);

    // initialize a librespot session
    let session = auth::new_session(&cache_folder, state.app_config.device.audio_cache).await?;

    // start application's threads
    let (send, recv) = std::sync::mpsc::channel::<event::ClientRequest>();

    // get some prior information
    send.send(event::ClientRequest::NewConnection)?;
    send.send(event::ClientRequest::GetCurrentUser)?;
    send.send(event::ClientRequest::GetCurrentPlayback)?;

    let (spirc_pub, _) = tokio::sync::broadcast::channel::<()>(16);

    // client event handler thread
    std::thread::spawn({
        let state = state.clone();
        let client = client::Client::new(
            session.clone(),
            state.app_config.device.clone(),
            state.app_config.client_id.clone(),
        );
        let send = send.clone();
        client.init_token().await?;
        move || {
            client::start_client_handler(state, client, send, recv, spirc_pub);
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
    std::thread::spawn({
        let send = send.clone();
        let state = state.clone();
        move || {
            client::start_player_event_watchers(state, send);
        }
    });

    // application's UI as the main thread
    ui::start_ui(state, send)
}
