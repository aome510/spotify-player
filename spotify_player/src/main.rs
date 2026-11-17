mod auth;
mod cli;
mod client;
mod command;
mod config;
mod event;
mod key;
mod log_layer;
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
use parking_lot::Mutex;
use std::{collections::VecDeque, io::Write, sync::Arc};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::config::apply_config_override;

struct UiThreadSupervisor {
    state: state::SharedState,
    thread: Option<std::thread::JoinHandle<Result<()>>>,
}

impl UiThreadSupervisor {
    fn new(state: state::SharedState, thread: std::thread::JoinHandle<Result<()>>) -> Self {
        Self {
            state,
            thread: Some(thread),
        }
    }

    fn join(mut self) -> Result<()> {
        join_ui_thread(self.thread.take().expect("UI thread is available"))
    }
}

impl Drop for UiThreadSupervisor {
    fn drop(&mut self) {
        self.state.ui.lock().is_running = false;
        if let Some(thread) = self.thread.take() {
            if let Err(err) = join_ui_thread(thread) {
                tracing::error!("Failed while shutting down the UI thread: {err:#}");
            }
        }
    }
}

fn install_panic_hook(backtrace_file: Option<std::fs::File>) {
    let backtrace_file = backtrace_file.map(std::sync::Mutex::new);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ui::handle_panic();

        if let Some(backtrace_file) = backtrace_file.as_ref() {
            if let Ok(mut file) = backtrace_file.lock() {
                let backtrace = backtrace::Backtrace::new();
                let _ = writeln!(&mut file, "Got a panic: {info:#?}\n");
                let _ = writeln!(&mut file, "Stack backtrace:\n{backtrace:?}");
            }
        }

        previous_hook(info);
    }));
}

fn init_logging(
    log_folder: &std::path::Path,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
) -> Result<()> {
    if std::env::var_os("RUST_LOG").is_some_and(|x| x == "off") {
        // Don't create log files if logging is disabled.
        install_panic_hook(None);
        return Ok(());
    }

    let log_prefix = format!(
        "spotify-player-{}",
        chrono::Local::now().format("%y-%m-%d-%H-%M")
    );

    // initialize the application's logging
    if std::env::var("RUST_LOG").is_err() {
        // default to log the current crate and librespot crates
        std::env::set_var("RUST_LOG", "spotify_player=info,librespot=info");
    }
    if !log_folder.exists() {
        std::fs::create_dir_all(log_folder)?;
    }
    let log_file = std::fs::File::create(log_folder.join(format!("{log_prefix}.log")))
        .context("failed to create log file")?;

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(log_file));

    let buffer_layer = crate::log_layer::BufferLayer::new(log_buffer, 1000);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(fmt_layer)
        .with(buffer_layer)
        .init();

    // initialize the application's panic backtrace
    let backtrace_file = std::fs::File::create(log_folder.join(format!("{log_prefix}.backtrace")))
        .context("failed to create backtrace file")?;
    install_panic_hook(Some(backtrace_file));

    Ok(())
}

#[tokio::main]
async fn start_app(state: &state::SharedState) -> Result<()> {
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
            let configs = config::get_config();
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
    let client = client::AppClient::new()
        .await
        .context("construct app client")?;
    client
        .new_session(Some(state), true)
        .await
        .context("initialize new Spotify session")?;

    // request user data
    client_pub.send(client::ClientRequest::GetCurrentUser)?;
    client_pub.send(client::ClientRequest::GetUserPlaylists)?;
    client_pub.send(client::ClientRequest::GetUserFollowedArtists)?;
    client_pub.send(client::ClientRequest::GetUserSavedAlbums)?;
    client_pub.send(client::ClientRequest::GetContext(state::ContextId::Tracks(
        state::USER_LIKED_TRACKS_ID.to_owned(),
    )))?;
    client_pub.send(client::ClientRequest::GetUserSavedShows)?;

    // client socket task (for handling CLI commands)
    tokio::task::spawn({
        let client = client.clone();
        let state = state.clone();
        async move {
            cli::start_socket(&client, Some(&state), None).await;
        }
    });

    // client event handler task
    tokio::task::spawn({
        let state = state.clone();
        let client = client.clone();
        async move {
            client::start_client_handler(&state, &client, &client_sub).await;
        }
    });

    // background task that detects an invalidated session and reconnects,
    // independent of any incoming client request
    tokio::task::spawn({
        let state = state.clone();
        let client = client.clone();
        async move {
            client::start_session_watcher(state, client).await;
        }
    });

    // player event watcher task
    std::thread::Builder::new()
        .name("player-event-watcher".to_string())
        .spawn({
            let state = state.clone();
            let client_pub = client_pub.clone();
            move || {
                client::start_player_event_watcher(&state, &client_pub);
            }
        })?;

    let ui_thread = if state.is_daemon {
        None
    } else {
        #[cfg(feature = "image")]
        ui::init_image_picker(state).context("initialize image picker")?;
        let terminal = ui::init_terminal().context("initialize terminal")?;

        // terminal event handler task
        std::thread::Builder::new()
            .name("terminal-event-handler".to_string())
            .spawn({
                let client_pub = client_pub.clone();
                let state = state.clone();
                move || {
                    event::start_event_handler(&state, &client_pub);
                }
            })?;

        // application UI task
        let ui_thread = std::thread::Builder::new().name("ui".to_string()).spawn({
            let state = state.clone();
            move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui::run(&state, terminal)
                }));
                state.ui.lock().is_running = false;

                match result {
                    Ok(result) => result,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
        })?;
        Some(UiThreadSupervisor::new(state.clone(), ui_thread))
    };

    #[cfg(feature = "media-control")]
    if config::get_config().app_config.enable_media_control {
        // media control task
        std::thread::Builder::new()
            .name("media-control".to_string())
            .spawn({
                let state = state.clone();
                move || {
                    if let Err(err) = media_control::start_event_watcher(&state, client_pub) {
                        tracing::error!(
                            "Failed to start the application's media control event watcher: err={err:#?}"
                        );
                    }
                }
            })?;

        // the winit's event loop must be run in the main thread
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            // Start an event loop that listens to OS window events.
            //
            // MacOS and Windows require an open window to be able to listen to media
            // control events. The below code will create an invisible window on startup
            // to listen to such events.
            let event_loop = winit::event_loop::EventLoop::new()?;
            let state = state.clone();
            #[allow(deprecated)]
            event_loop.run(move |_, event_loop| {
                event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                    std::time::Instant::now() + std::time::Duration::from_millis(100),
                ));
                if !state.ui.lock().is_running {
                    event_loop.exit();
                }
            })?;
        }
    }

    if let Some(ui_thread) = ui_thread {
        return ui_thread.join();
    }

    // Keep daemon mode alive while its background tasks are healthy.
    loop {
        if ui::application_panicked() {
            anyhow::bail!("an application thread panicked");
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic payload")
}

fn join_ui_thread(ui_thread: std::thread::JoinHandle<Result<()>>) -> Result<()> {
    match ui_thread.join() {
        Ok(result) => result.context("application UI failed"),
        Err(payload) => anyhow::bail!("UI thread panicked: {}", panic_payload_message(&*payload)),
    }
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
        if configs.app_config.log_folder.is_none() {
            // set the log folder to be the cache folder if it is not set
            configs.app_config.log_folder = Some(cache_folder);
        }
        if let Some(overrides) = args.get_many::<String>("config-override") {
            for override_str in overrides {
                let (key, value) = override_str.split_once('=').context(format!(
                    "Invalid override format: '{override_str}'. Expected KEY=VALUE"
                ))?;

                apply_config_override(&mut configs.app_config, key, value)?;
            }
        }
        config::set_config(configs);
    }

    match args.subcommand() {
        None => {
            // initialize the application's log
            let log_folder = config::get_config()
                .app_config
                .log_folder
                .as_deref()
                .expect("log_folder is set");

            let log_buffer: Arc<Mutex<VecDeque<String>>> =
                Arc::new(Mutex::new(VecDeque::with_capacity(1000)));

            init_logging(log_folder, log_buffer.clone())
                .context("failed to initialize application's logging")?;

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

            let state = std::sync::Arc::new(state::State::new(is_daemon, log_buffer));
            start_app(&state)
        }
        Some((cmd, args)) => cli::handle_cli_subcommand(cmd, args),
    }
}

#[cfg(test)]
mod tests {
    use super::join_ui_thread;

    #[test]
    fn ui_thread_panic_is_reported_as_an_error() {
        let ui_thread = std::thread::spawn(|| -> anyhow::Result<()> {
            panic!("injected UI failure");
        });

        let err = join_ui_thread(ui_thread).unwrap_err();

        assert!(err.to_string().contains("injected UI failure"));
    }
}
