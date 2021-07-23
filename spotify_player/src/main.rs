mod client;
mod config;
mod event;
pub mod prelude;
mod state;
mod ui;
pub mod utils;

use prelude::*;

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

#[tokio::main]
async fn start_client_watcher(
    state: state::SharedState,
    client: client::Client,
    recv: mpsc::Receiver<event::Event>,
) {
    if let Err(err) = client::start_watcher(state, client, recv).await {
        log::error!("client watcher error: {:#?}", err);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let config_folder = config::get_config_folder_path()?;

    let (send, recv) = mpsc::channel::<event::Event>();
    let state = state::State::new();

    // start application's threads
    thread::spawn({
        let client_config = config::ClientConfig::from_config_file(config_folder)?;

        let oauth = SpotifyOAuth::default()
            .client_id(&client_config.client_id)
            .client_secret(&client_config.client_secret)
            .redirect_uri("http://localhost:8888/callback")
            .cache_path(config::get_token_cache_file_path()?)
            .scope(&SCOPES.join(" "))
            .build();

        let client = client::Client::new(oauth);
        let cloned_state = state.clone();
        move || {
            start_client_watcher(cloned_state, client, recv);
        }
    });
    thread::spawn({
        let cloned_sender = send.clone();
        let cloned_state = state.clone();
        move || {
            event::start_event_stream(cloned_sender, cloned_state);
        }
    });
    ui::start_ui(state, send)
}
