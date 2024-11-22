mod auth;
mod cli;
mod client;
mod command;
mod config;
mod event;
mod key;
#[cfg(feature = "media-control")]
mod media_control;
mod playlist_folders;
mod state;
#[cfg(feature = "streaming")]
mod streaming;
mod token;
mod ui;
mod utils;

use anyhow::{Context, Result};
use std::io::Write;

async fn init_spotify(
    client_pub: &flume::Sender<client::ClientRequest>,
    client: &client::Client,
    state: &state::SharedState,
) -> Result<()> {
    client.initialize_playback(state);

    // request user data
    client_pub.send(client::ClientRequest::GetCurrentUser)?;
    client_pub.send(client::ClientRequest::GetUserPlaylists)?;
    client_pub.send(client::ClientRequest::GetUserFollowedArtists)?;
    client_pub.send(client::ClientRequest::GetUserSavedAlbums)?;
    client_pub.send(client::ClientRequest::GetUserSavedTracks)?;

    Ok(())
}

fn init_logging(cache_folder: &std::path::Path) -> Result<()> {
    let log_prefix = format!(
        "spotify-player-{}",
        chrono::Local::now().format("%y-%m-%d-%H-%M")
    );

    // initialize the application's logging
    if std::env::var("RUST_LOG").is_err() {
        // default to log the current crate and librespot crates
        std::env::set_var("RUST_LOG", "spotify_player=info,librespot=info");
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
async fn start_app(state: &state::SharedState) -> Result<()> {
    let configs = config::get_config();

    if !state.is_daemon {
        #[cfg(feature = "image")]
        {
            // initialize `viuer` supports for kitty, iterm2, and sixel
            viuer::get_kitty_support();
            viuer::is_iterm_supported();
            #[cfg(feature = "sixel")]
            viuer::is_sixel_supported();
        }
    }

    // client channels
    let (client_pub, client_sub) = flume::unbounded::<client::ClientRequest>();

    #[cfg(feature = "pulseaudio-backend")]
    {
        // set environment variables for PulseAudio
        if std::env::var("PULSE_PROP_application.name").is_err() {
            std::env::set_var("PULSE_PROP_application.name", "spotify-player");
        }
        if std::env::var("PULSE_PROP_application.icon_name").is_err() {
            std::env::set_var("PULSE_PROP_application.icon_name", "spotify");
        }
        if std::env::var("PULSE_PROP_stream.description").is_err() {
            std::env::set_var(
                "PULSE_PROP_stream.description",
                format!(
                    "Spotify Connect endpoint ({})",
                    configs.app_config.device.name
                ),
            );
        }
        if std::env::var("PULSE_PROP_media.software").is_err() {
            std::env::set_var("PULSE_PROP_media.software", "Spotify");
        }
        if std::env::var("PULSE_PROP_media.role").is_err() {
            std::env::set_var("PULSE_PROP_media.role", "music");
        }
    }

    // create a Spotify API client
    let auth_config = auth::AuthConfig::new(configs)?;
    let client = client::Client::new(auth_config);
    client
        .new_session(Some(state), true)
        .await
        .context("initialize new Spotify session")?;

    // initialize Spotify-related stuff
    init_spotify(&client_pub, &client, state)
        .await
        .context("Failed to initialize the Spotify data")?;

    // Spawn application's tasks
    let mut tasks = Vec::new();

    // client socket task (for handling CLI commands)
    tasks.push(tokio::task::spawn({
        let client = client.clone();
        let state = state.clone();
        async move {
            let port = configs.app_config.client_port;
            tracing::info!("Starting a client socket at 127.0.0.1:{port}");
            match tokio::net::UdpSocket::bind(("127.0.0.1", port)).await {
                Ok(socket) => cli::start_socket(client, socket, Some(state)).await,
                Err(err) => {
                    tracing::warn!(
                        "Failed to create a client socket for handling CLI commands: {err:#}"
                    )
                }
            }
        }
    }));

    // client event handler task
    tasks.push(tokio::task::spawn({
        let state = state.clone();
        async move {
            client::start_client_handler(state, client, client_sub).await;
        }
    }));

    // player event watcher task
    tasks.push(tokio::task::spawn({
        let state = state.clone();
        let client_pub = client_pub.clone();
        async move {
            client::start_player_event_watchers(state, client_pub).await;
        }
    }));

    if !state.is_daemon {
        // spawn tasks needed for running the application UI

        // terminal event handler task
        tokio::task::spawn_blocking({
            let client_pub = client_pub.clone();
            let state = state.clone();
            move || {
                event::start_event_handler(state, client_pub);
            }
        });

        // application UI task
        tokio::task::spawn_blocking({
            let state = state.clone();
            move || ui::run(state)
        });
    }

    #[cfg(feature = "media-control")]
    if configs.app_config.enable_media_control {
        // media control task
        tokio::task::spawn_blocking({
            let state = state.clone();
            move || {
                if let Err(err) = media_control::start_event_watcher(state, client_pub) {
                    tracing::error!(
                        "Failed to start the application's media control event watcher: err={err:#?}"
                    );
                }
            }
        });

        // the winit's event loop must be run in the main thread
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            // Start an event loop that listens to OS window events.
            //
            // MacOS and Windows require an open window to be able to listen to media
            // control events. The below code will create an invisible window on startup
            // to listen to such events.
            let event_loop = winit::event_loop::EventLoop::new()?;
            #[allow(deprecated)]
            event_loop.run(move |_, _| {})?;
        }
    }

    for task in tasks {
        task.await?;
    }

    Ok(())
}

fn main() -> Result<()> {
    // librespot depends on hyper-rustls which requires a crypto provider to be set up.
    // TODO: see if this can be fixed upstream
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    // parse command line arguments
    let args = cli::init_cli()?.get_matches();

    // initialize the application's cache and config folders
    let config_folder: std::path::PathBuf = args
        .get_one::<String>("config-folder")
        .expect("config-folder should have default value")
        .into();
    if !config_folder.exists() {
        std::fs::create_dir_all(&config_folder)?;
    }

    let cache_folder: std::path::PathBuf = args
        .get_one::<String>("cache-folder")
        .expect("cache-folder should have a default value")
        .into();
    let cache_audio_folder = cache_folder.join("audio");
    if !cache_audio_folder.exists() {
        std::fs::create_dir_all(&cache_audio_folder)?;
    }
    let cache_image_folder = cache_folder.join("image");
    if !cache_image_folder.exists() {
        std::fs::create_dir_all(&cache_image_folder)?;
    }

    // initialize the application configs
    {
        let mut configs = config::Configs::new(&config_folder, &cache_folder)?;
        if let Some(theme) = args.get_one::<String>("theme") {
            // override the theme config if user specifies a `theme` cli argument
            theme.clone_into(&mut configs.app_config.theme);
        }
        config::set_config(configs);
    }

    match args.subcommand() {
        None => {
            // initialize the application's log
            init_logging(&cache_folder).context("failed to initialize application's logging")?;

            // log the application's configurations
            tracing::info!("Configurations: {:?}", config::get_config());

            let is_daemon;

            #[cfg(feature = "daemon")]
            {
                is_daemon = args.get_flag("daemon");
                if is_daemon {
                    if cfg!(any(target_os = "macos", target_os = "windows"))
                        && cfg!(feature = "media-control")
                    {
                        eprintln!("Running the application as a daemon on windows/macos with `media-control` feature enabled is not supported!");
                        std::process::exit(1);
                    }

                    tracing::info!("Starting the application as a daemon...");
                    let daemonize = daemonize::Daemonize::new();
                    daemonize.start()?;
                }
            }

            #[cfg(not(feature = "daemon"))]
            {
                is_daemon = false;
            }

            let state = std::sync::Arc::new(state::State::new(is_daemon));
            start_app(&state)
        }
        Some((cmd, args)) => cli::handle_cli_subcommand(cmd, args),
    }
}
