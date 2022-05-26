use tokio::sync::{broadcast, mpsc};
use tracing::Instrument;

use crate::{event::ClientRequest, state::*};

use super::Client;

/// starts the client's request handler
pub async fn start_client_handler(
    state: SharedState,
    client: Client,
    client_pub: mpsc::Sender<ClientRequest>,
    mut client_sub: mpsc::Receiver<ClientRequest>,
    streaming_pub: broadcast::Sender<()>,
) {
    while let Some(request) = client_sub.recv().await {
        match request {
            #[cfg(feature = "streaming")]
            ClientRequest::NewStreamingConnection => {
                // send a notification to current streaming subcriber channels to shutdown all running connections
                streaming_pub.send(()).unwrap_or_default();
                if let Err(err) = client
                    .new_streaming_connection(streaming_pub.subscribe(), client_pub.clone(), true)
                    .await
                {
                    tracing::error!(
                        "Encountered an error during creating a new streaming connection: {err:#}",
                    );
                }
            }
            _ => {
                let state = state.clone();
                let client = client.clone();
                let span = tracing::info_span!("Client_request", request = ?request);
                tokio::task::spawn(
                    async move {
                        if let Err(err) = client.handle_request(&state, request).await {
                            tracing::error!("Failed to handle client request: {err:#}");
                        }
                    }
                    .instrument(span),
                );
            }
        }
    }
}

/// Starts multiple event watchers listening to events and
/// notifying the client to make update requests if needed
pub fn start_player_event_watchers(state: SharedState, client_pub: mpsc::Sender<ClientRequest>) {
    // Start a watcher task that updates the playback every `playback_refresh_duration_in_ms` ms.
    // A positive value of `playback_refresh_duration_in_ms` is required to start the watcher.
    if state.app_config.playback_refresh_duration_in_ms > 0 {
        tokio::task::spawn_blocking({
            let client_pub = client_pub.clone();
            let playback_refresh_duration =
                std::time::Duration::from_millis(state.app_config.playback_refresh_duration_in_ms);
            move || loop {
                client_pub
                    .blocking_send(ClientRequest::GetCurrentPlayback)
                    .unwrap_or_default();
                std::thread::sleep(playback_refresh_duration);
            }
        });
    }

    // Main watcher task
    let refresh_duration = std::time::Duration::from_millis(200);
    loop {
        std::thread::sleep(refresh_duration);

        let mut ui = state.ui.lock();
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
                    .blocking_send(ClientRequest::GetCurrentPlayback)
                    .unwrap_or_default();
            }
        }

        // update the context state and request new data when moving to a new context page
        if let PageState::Context {
            id,
            context_page_type,
            state,
        } = ui.current_page_mut()
        {
            let expected_id = match context_page_type {
                ContextPageType::Browsing(context_id) => Some(context_id.clone()),
                ContextPageType::CurrentPlaying => player.playing_context_id(),
            };

            if *id != expected_id {
                tracing::info!("Current context ID ({:?}) is different from the expected ID ({:?}), update the context state", id, expected_id);

                *id = expected_id.clone();
                match expected_id {
                    Some(id) => {
                        client_pub
                            .blocking_send(ClientRequest::GetContext(id.clone()))
                            .unwrap_or_default();
                        *state = Some(match id {
                            ContextId::Album(_) => ContextPageUIState::new_album(),
                            ContextId::Artist(_) => ContextPageUIState::new_artist(),
                            ContextId::Playlist(_) => ContextPageUIState::new_playlist(),
                        });
                    }
                    None => {
                        *state = None;
                    }
                }
            }
        }
    }
}
