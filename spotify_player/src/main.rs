mod client;
mod config;
mod event;
pub mod prelude;
mod state;
mod ui;

use prelude::*;
use rspotify::oauth2::SpotifyOAuth;

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
    mut client: client::Client,
    recv: mpsc::Receiver<event::Event>,
) {
    while let Ok(event) = recv.recv() {
        if let Err(err) = client.handle_event(&state, event).await {
            client.handle_error(err);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config_folder = config::get_config_folder_path()?;
    let client_config = config::ClientConfig::from_config_file(config_folder)?;

    let oauth = SpotifyOAuth::default()
        .client_id(&client_config.client_id)
        .client_secret(&client_config.client_secret)
        .redirect_uri("http://localhost:8888/callback")
        .cache_path(config::get_token_cache_file_path()?)
        .scope(&SCOPES.join(" "))
        .build();

    let (send, recv) = mpsc::channel::<event::Event>();

    let mut client = client::Client::new(oauth);
    let expires_at = client.refresh_token().await?;

    let state = state::State::new();
    state.write().unwrap().auth_token_expires_at = expires_at;

    let cloned_state = state.clone();
    thread::spawn(move || {
        start_client_watcher(cloned_state, client, recv);
    });

    let cloned_sender = send.clone();
    thread::spawn(move || {
        event::start_event_stream(cloned_sender);
    });

    ui::start_ui(state, send)
}
