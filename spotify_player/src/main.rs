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

use anyhow::Context;

fn init_app_cli_arguments() -> clap::ArgMatches {
    clap::Command::new("spotify-player")
        .version("0.6.0")
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

async fn init_spotify(
    client_pub: &tokio::sync::mpsc::Sender<event::ClientRequest>,
    spirc_pub: &tokio::sync::broadcast::Sender<()>,
    client: &client::Client,
    state: &state::SharedState,
) -> anyhow::Result<()> {
    client.init_token().await?;
    client.update_current_playback_state(state).await?;

    // if `streaming` feature is enabled, create new Spirc connection
    #[cfg(feature = "streaming")]
    client
        .new_spirc_connection(spirc_pub.subscribe(), client_pub.clone(), false)
        .await?;

    if state.player.read().playback.is_none() {
        tracing::info!(
            "no playback found on startup, trying to connect to the first available device"
        );
        client.connect_to_first_available_device().await?;
    }

    // Request user data

    client_pub
        .send(event::ClientRequest::GetCurrentUser)
        .await?;

    // request data needed to render the Library page (default page when starting the application)
    client_pub
        .send(event::ClientRequest::GetUserPlaylists)
        .await?;
    client_pub
        .send(event::ClientRequest::GetUserFollowedArtists)
        .await?;
    client_pub
        .send(event::ClientRequest::GetUserSavedAlbums)
        .await?;

    Ok(())
}

fn init_logging(cache_folder: &std::path::Path) -> anyhow::Result<()> {
    let log_file = format!(
        "spotify-player-{}.log",
        chrono::Local::now().format("%y-%m-%d-%R")
    );
    let log_file_path = cache_folder.join(log_file);
    // initialize the application's logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "spotify_player=info") // default to log the current crate only
    }
    let log_file = std::fs::File::create(log_file_path).context("failed to create log file")?;
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // parse command line arguments
    let args = init_app_cli_arguments();

    // initialize the application's cache folder and config folder
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

    init_logging(&cache_folder)?;

    // initialize the application state
    let mut state = state::State::default();
    // parse config options from the config files into application's state
    state.parse_config_files(&config_folder, args.value_of("theme"))?;
    let state = std::sync::Arc::new(state);

    // create a librespot session
    let session = auth::new_session(&cache_folder, state.app_config.device.audio_cache).await?;

    // create a spotify API client
    let client = client::Client::new(
        session.clone(),
        state.app_config.device.clone(),
        state.app_config.client_id.clone(),
    );

    // create application's channels
    let (client_pub, client_sub) = tokio::sync::mpsc::channel::<event::ClientRequest>(16);
    let (spirc_pub, _) = tokio::sync::broadcast::channel::<()>(16);

    // initialize Spotify-related stuff
    init_spotify(&client_pub, &spirc_pub, &client, &state)
        .await
        .context("failed to initialize the spotify client")?;

    // Spawn application's tasks

    // client event handler task
    tokio::task::spawn({
        let state = state.clone();
        let client_pub = client_pub.clone();
        async move {
            client::start_client_handler(state, client, client_pub, client_sub, spirc_pub).await;
        }
    });

    // terminal event handler task
    tokio::task::spawn_blocking({
        let client_pub = client_pub.clone();
        let state = state.clone();
        move || {
            event::start_event_handler(state, client_pub);
        }
    });

    // player event watcher task
    tokio::task::spawn_blocking({
        let state = state.clone();
        move || {
            client::start_player_event_watchers(state, client_pub);
        }
    });

    // application's UI as the main task
    tokio::task::spawn_blocking(move || {
        ui::start_ui(state).unwrap();
    })
    .await?;
    std::process::exit(0);
}
