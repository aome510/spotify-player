use anyhow::Result;
use rspotify::{model, oauth2::SpotifyOAuth};

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

mod client;
mod config;
mod event;
mod state;

use std::{sync::mpsc, thread};

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
    crossterm::terminal::enable_raw_mode()?;
    thread::spawn(move || {
        event::poll_events(cloned_sender);
    });

    loop {
        if std::time::SystemTime::now() > state.read().unwrap().auth_token_expires_at {
            send.send(event::Event::RefreshToken).unwrap();
        }
        send.send(event::Event::GetCurrentPlaybackContext).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        if let Some(context) = state.read().unwrap().current_playback_context.clone() {
            if let Some(model::PlayingItem::Track(track)) = context.item {
                let progress_in_sec: u32 = context.progress_ms.unwrap() / 1000;
                println!(
                    "currently playing {} at {}/{} (repeat: {}, shuffle: {})",
                    track.name,
                    progress_in_sec,
                    track.duration_ms / 1000,
                    context.repeat_state.as_str(),
                    context.shuffle_state,
                );
            }
        }
    }
}
