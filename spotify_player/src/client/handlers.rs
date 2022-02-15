use std::sync::mpsc;

use tokio::sync::broadcast;

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::*,
};

use super::Client;

/// starts the client's request handler
pub fn start_client_handler(
    state: SharedState,
    client: Client,
    client_pub: mpsc::Sender<ClientRequest>,
    client_sub: mpsc::Receiver<ClientRequest>,
    spirc_pub: broadcast::Sender<()>,
) {
    while let Ok(request) = client_sub.recv() {
        match request {
            #[cfg(feature = "streaming")]
            ClientRequest::NewSpircConnection => {
                // send a notification to current spirc subcribers to shutdown all running spirc connections
                spirc_pub.send(()).unwrap_or_default();
                client.new_spirc_connection(spirc_pub.subscribe(), client_pub.clone());
            }
            _ => {
                let state = state.clone();
                let client = client.clone();
                tokio::spawn(async move {
                    if let Err(err) = client.handle_request(&state, request).await {
                        tracing::warn!("failed to handle client request: {:?}", err);
                    }
                });
            }
        }
    }
}

/// starts multiple event watchers listening
/// to player events and notifying the client
/// to make additional update requests if needed
pub fn start_player_event_watchers(state: SharedState, client_pub: mpsc::Sender<ClientRequest>) {
    // start a thread that updates the current playback every `playback_refresh_duration_in_ms` ms.
    // A positive value of `playback_refresh_duration_in_ms` is required to start the watcher.
    if state.app_config.playback_refresh_duration_in_ms > 0 {
        tokio::task::spawn_blocking({
            let client_pub = client_pub.clone();
            let playback_refresh_duration =
                std::time::Duration::from_millis(state.app_config.playback_refresh_duration_in_ms);
            move || loop {
                client_pub
                    .send(ClientRequest::GetCurrentPlayback)
                    .unwrap_or_default();
                std::thread::sleep(playback_refresh_duration);
            }
        });
    }

    // the main thread that watches new player events every `refresh_duration` ms.
    let refresh_duration = std::time::Duration::from_millis(1000);
    loop {
        std::thread::sleep(refresh_duration);

        let player = state.player.read();

        // update the playback when the current track ends
        let progress_ms = player.playback_progress();
        let duration_ms = player.current_playing_track().map(|t| t.duration);
        let is_playing = match player.playback {
            Some(ref playback) => playback.is_playing,
            None => false,
        };
        if let (Some(progress_ms), Some(duration_ms)) = (progress_ms, duration_ms) {
            if progress_ms >= duration_ms && is_playing {
                client_pub
                    .send(ClientRequest::GetCurrentPlayback)
                    .unwrap_or_default();
            }
        }

        // try to reconnect if there is no playback
        if player.playback.is_none() {
            client_pub
                .send(ClientRequest::Player(PlayerRequest::Reconnect))
                .unwrap_or_default();
        }
    }
}
