mod auth;
mod client;
mod command;
mod config;
mod event;
mod key;
#[cfg(feature = "media-control")]
mod media_control;
mod state;
#[cfg(feature = "streaming")]
mod streaming;
mod token;
mod ui;
mod utils;

use anyhow::{Context, Result};
use std::io::Write;

fn init_app_cli_arguments() -> clap::ArgMatches {
    clap::Command::new("spotify-player")
        .version("0.7.0")
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
    streaming_pub: &tokio::sync::broadcast::Sender<()>,
    client: &client::Client,
    state: &state::SharedState,
) -> Result<()> {
    client.init_token().await?;
    client.update_current_playback_state(state).await?;

    // if `streaming` feature is enabled, create a new streaming connection
    #[cfg(feature = "streaming")]
    client
        .new_streaming_connection(streaming_pub.subscribe(), client_pub.clone(), false)
        .await
        .context("failed to create a new streaming connection")?;

    if state.player.read().playback.is_none() {
        tracing::info!(
            "No playback found on startup, trying to connect to the first available device"
        );
        client.connect_to_first_available_device().await?;
    }

    // request user data
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

fn init_logging(cache_folder: &std::path::Path) -> Result<()> {
    let log_prefix = format!(
        "spotify-player-{}",
        chrono::Local::now().format("%y-%m-%d-%R")
    );

    // initialize the application's logging
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "spotify_player=info"); // default to log the current crate only
    }
    let log_file = std::fs::File::create(cache_folder.join(format!("{log_prefix}.log")))
        .context("failed to create log file")?;
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    // initialize the application's panic backtrace
    let backtrace_file =
        std::fs::File::create(cache_folder.join(format!("{log_prefix}.backtrace")))
            .context("failed to create backtrace file")?;
    let backtrace_file = std::sync::Mutex::new(backtrace_file);
    std::panic::set_hook(Box::new(move |info| {
        let mut file = backtrace_file.lock().unwrap();
        let backtrace = backtrace::Backtrace::new();
        writeln!(&mut file, "Got a panic: {info:#?}\n").unwrap();
        writeln!(&mut file, "Stack backtrace:\n{:?}", backtrace).unwrap();
    }));

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
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

    init_logging(&cache_folder).context("failed to initialize application's logging")?;

    // initialize the application state
    let state = {
        let mut state = state::State::default();
        // parse config options from the config files into application's state
        state.parse_config_files(&config_folder, args.value_of("theme"))?;
        std::sync::Arc::new(state)
    };

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
    // broadcast channels used to shutdown running streaming connections upon creating a new one
    let (streaming_pub, _) = tokio::sync::broadcast::channel::<()>(16);

    // initialize Spotify-related stuff
    init_spotify(&client_pub, &streaming_pub, &client, &state)
        .await
        .context("failed to initialize the spotify client")?;

    // Spawn application's tasks

    // client event handler task
    tokio::task::spawn({
        let state = state.clone();
        let client_pub = client_pub.clone();
        async move {
            client::start_client_handler(state, client, client_pub, client_sub, streaming_pub)
                .await;
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

    #[cfg(not(feature = "media-control"))]
    {
        // application's UI as the main task
        tokio::task::spawn_blocking(move || ui::run(state)).await??;

        Ok(())
    }

    #[cfg(feature = "media-control")]
    {
        // media control task
        tokio::task::spawn_blocking(move || {
            if let Err(err) = media_control::start_event_watcher() {
                tracing::error!(
                    "Failed to start the application's media control event watcher: err={err:?}"
                );
            }
        });

        // When `media-control` feature is enabled.
        // The OS event loop must be run in the main thread, so
        // the application's UI is run as a background task.
        tokio::task::spawn_blocking({
            let state = state.clone();
            move || ui::run(state)
        });

        // Start an event loop that listens to OS window events.
        let event_loop = winit::event_loop::EventLoop::new();
        event_loop.run(move |_, _, control_flow| {
            *control_flow = winit::event_loop::ControlFlow::Wait;
        });
    }
}
