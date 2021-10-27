use anyhow::Result;
use std::sync::mpsc;

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::*,
};

use super::Client;

/// starts the client's request handler
#[tokio::main]
pub async fn start_client_handler(
    state: SharedState,
    client: Client,
    recv: mpsc::Receiver<ClientRequest>,
) {
    while let Ok(request) = recv.recv() {
        let state = state.clone();
        let client = client.clone();
        tokio::spawn(async move {
            if let Err(err) = client.handle_request(&state, request).await {
                log::warn!("failed to handle client request: {:?}", err);
            }
        });
    }
}

/// starts multiple event watchers listening
/// to player events and notifying the client
/// to make additional update requests if needed
#[tokio::main]
pub async fn start_player_event_watchers(state: SharedState, send: mpsc::Sender<ClientRequest>) {
    // start a thread that updates the current playback every `playback_refresh_duration_in_ms` ms.
    // A positive value of `playback_refresh_duration_in_ms` is required to start the watcher.
    if state.app_config.playback_refresh_duration_in_ms > 0 {
        std::thread::spawn({
            let send = send.clone();
            let playback_refresh_duration =
                std::time::Duration::from_millis(state.app_config.playback_refresh_duration_in_ms);
            move || -> Result<()> {
                loop {
                    send.send(ClientRequest::GetCurrentPlayback).unwrap();
                    std::thread::sleep(playback_refresh_duration);
                }
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
                send.send(ClientRequest::GetCurrentPlayback).unwrap();
            }
        }

        // try to reconnect if there is no playback
        if player.playback.is_none() {
            send.send(ClientRequest::Player(PlayerRequest::Reconnect))
                .unwrap();
        }
    }
}
