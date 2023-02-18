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
        .version("0.11.1")
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
    client_pub: &flume::Sender<event::ClientRequest>,
    streaming_sub: &flume::Receiver<()>,
    client: &client::Client,
    state: &state::SharedState,
) -> Result<()> {
    client.init_token().await?;

    // if `streaming` feature is enabled, create a new streaming connection
    #[cfg(feature = "streaming")]
    client
        .new_streaming_connection(streaming_sub.clone(), client_pub.clone())
        .await
        .context("failed to create a new streaming connection")?;

    // initialize the playback state
    client.update_current_playback_state(state).await?;

    if state.player.read().playback.is_none() {
        tracing::info!("No playback found on startup, trying to connect to an available device...");
        client_pub.send(event::ClientRequest::ConnectDevice(None))?;
    }

    // request user data
    client_pub.send(event::ClientRequest::GetCurrentUserQueue)?;
    client_pub.send(event::ClientRequest::GetCurrentUser)?;
    client_pub.send(event::ClientRequest::GetUserPlaylists)?;
    client_pub.send(event::ClientRequest::GetUserFollowedArtists)?;
    client_pub.send(event::ClientRequest::GetUserSavedAlbums)?;
    client_pub.send(event::ClientRequest::GetUserSavedTracks)?;

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
        writeln!(&mut file, "Stack backtrace:\n{backtrace:?}").unwrap();
    }));

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // parse command line arguments
    let args = init_app_cli_arguments();

    // initialize the application's cache folder and config folder
    let config_folder = match args.get_one::<String>("config-folder") {
        Some(path) => path.into(),
        None => config::get_config_folder_path()?,
    };
    let cache_folder = match args.get_one::<String>("cache-folder") {
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
        state.parse_config_files(&config_folder, args.get_one::<String>("theme"))?;
        std::sync::Arc::new(state)
    };

    // create a librespot session
    let session = auth::new_session(
        &cache_folder,
        state.app_config.device.audio_cache,
        &state.app_config,
    )
    .await?;

    // create a spotify API client
    let client = client::Client::new(
        session.clone(),
        state.app_config.device.clone(),
        state.app_config.client_id.clone(),
    );

    // client channels
    let (client_pub, client_sub) = flume::unbounded::<event::ClientRequest>();
    // streaming channels, which are used to notify a shutdown to running streaming connections
    // upon creating a new connection.
    let (streaming_pub, streaming_sub) = flume::unbounded::<()>();

    // initialize Spotify-related stuff
    init_spotify(&client_pub, &streaming_sub, &client, &state)
        .await
        .context("failed to initialize the spotify client")?;

    #[cfg(feature = "image")]
    {
        // initialize viuer supports for kitty and iterm2
        viuer::get_kitty_support();
        viuer::is_iterm_supported();
        #[cfg(feature = "sixel")]
        viuer::is_sixel_supported();
    }

    // Spawn application's tasks

    // client event handler task
    tokio::task::spawn({
        let state = state.clone();
        let client_pub = client_pub.clone();
        async move {
            client::start_client_handler(
                state,
                client,
                client_pub,
                client_sub,
                streaming_pub,
                streaming_sub,
            )
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
    tokio::task::spawn({
        let state = state.clone();
        let client_pub = client_pub.clone();
        async move {
            client::start_player_event_watchers(state, client_pub).await;
        }
    });

    // application UI task
    #[allow(unused_variables)]
    let ui_task = tokio::task::spawn_blocking({
        let state = state.clone();
        move || ui::run(state)
    });

    #[cfg(feature = "media-control")]
    if state.app_config.enable_media_control {
        // media control task
        tokio::task::spawn_blocking({
            let state = state.clone();
            move || {
                if let Err(err) = media_control::start_event_watcher(state, client_pub) {
                    tracing::error!(
                        "Failed to start the application's media control event watcher: err={err:?}"
                    );
                }
            }
        });

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            // Start an event loop that listens to OS window events.
            //
            // MacOS and Windows require an open window to be able to listen to media
            // control events. The below code will create an invisible window on startup
            // to listen to such events.
            let event_loop = winit::event_loop::EventLoop::new();
            event_loop.run(move |_, _, control_flow| {
                *control_flow = winit::event_loop::ControlFlow::Wait;
            });
        }
    }
    #[allow(unreachable_code)]
    {
        ui_task.await??;
        Ok(())
    }
}
