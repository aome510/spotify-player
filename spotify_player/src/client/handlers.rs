use tracing::Instrument;

use crate::{event::ClientRequest, state::*};

#[cfg(feature = "lyric-finder")]
use crate::utils::map_join;

/// starts the client's request handler
pub async fn start_client_handler(
    state: SharedState,
    client: super::Client,
    client_pub: flume::Sender<ClientRequest>,
    client_sub: flume::Receiver<ClientRequest>,
    streaming_pub: flume::Sender<()>,
    streaming_sub: flume::Receiver<()>,
) {
    while let Ok(request) = client_sub.recv_async().await {
        match request {
            #[cfg(feature = "streaming")]
            ClientRequest::NewStreamingConnection => {
                // send a notification to current streaming subcriber channels to shutdown all running connections
                streaming_pub.send(()).unwrap_or_default();
                match client
                    .new_streaming_connection(streaming_sub.clone(), client_pub.clone())
                    .await
                {
                    Err(err) => tracing::error!(
                        "Encountered an error during creating a new streaming connection: {err:#}",
                    ),
                    Ok(id) => {
                        // By default, when `NewStreamingConnection` is called, the app also connects to the new device
                        client_pub
                            .send(ClientRequest::ConnectDevice(Some(id)))
                            .unwrap_or_default();
                    }
                }
            }
            _ => {
                let state = state.clone();
                let client = client.clone();
                let span = tracing::info_span!("client_request", request = ?request);
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
pub async fn start_player_event_watchers(
    state: SharedState,
    client_pub: flume::Sender<ClientRequest>,
) {
    // Start a watcher task that updates the playback every `playback_refresh_duration_in_ms` ms.
    // A positive value of `playback_refresh_duration_in_ms` is required to start the watcher.
    if state.app_config.playback_refresh_duration_in_ms > 0 {
        tokio::task::spawn({
            let client_pub = client_pub.clone();
            let playback_refresh_duration =
                std::time::Duration::from_millis(state.app_config.playback_refresh_duration_in_ms);
            async move {
                loop {
                    client_pub
                        .send_async(ClientRequest::GetCurrentPlayback)
                        .await
                        .unwrap_or_default();
                    tokio::time::sleep(playback_refresh_duration).await;
                }
            }
        });
    }

    let refresh_duration = std::time::Duration::from_millis(200);

    // Main watcher task
    loop {
        tokio::time::sleep(refresh_duration).await;

        // update the playback when the current track ends
        let (progress_ms, duration_ms, is_playing) = {
            let player = state.player.read();

            (
                player.playback_progress(),
                player.current_playing_track().map(|t| t.duration),
                player
                    .playback
                    .as_ref()
                    .map(|p| p.is_playing)
                    .unwrap_or_default(),
            )
        };
        if let (Some(progress_ms), Some(duration_ms)) = (progress_ms, duration_ms) {
            if progress_ms >= duration_ms && is_playing {
                client_pub
                    .send_async(ClientRequest::GetCurrentPlayback)
                    .await
                    .unwrap_or_default();
            }
        }

        // update the context state and request new data when moving to a new context page
        match state.ui.lock().current_page_mut() {
            PageState::Context {
                id,
                context_page_type,
                state: page_state,
            } => {
                let expected_id = match context_page_type {
                    ContextPageType::Browsing(context_id) => Some(context_id.clone()),
                    ContextPageType::CurrentPlaying => state.player.read().playing_context_id(),
                };

                if *id != expected_id {
                    tracing::info!("Current context ID ({:?}) is different from the expected ID ({:?}), update the context state", id, expected_id);

                    *id = expected_id;

                    // update the UI page state based on the context's type
                    match id {
                        Some(id) => {
                            *page_state = Some(match id {
                                ContextId::Album(_) => ContextPageUIState::new_album(),
                                ContextId::Artist(_) => ContextPageUIState::new_artist(),
                                ContextId::Playlist(_) => ContextPageUIState::new_playlist(),
                                ContextId::Tracks(_) => ContextPageUIState::new_tracks(),
                            });
                        }
                        None => {
                            *page_state = None;
                        }
                    }

                    // request new context's data if not found in memory
                    if let Some(id) = id {
                        if !state.data.read().caches.context.contains(&id.uri()) {
                            client_pub
                                .send(ClientRequest::GetContext(id.clone()))
                                .unwrap_or_default();
                        }
                    }
                }
            }
            #[cfg(feature = "lyric-finder")]
            PageState::Lyric { track, artists, .. } => {
                if let Some(current_track) = state.player.read().current_playing_track() {
                    if current_track.name != *track {
                        tracing::info!("Current playing track \"{}\" is different from the track \"{track}\" shown up in the lyric page. Updating the track and fetching its lyric...", current_track.name);
                        *track = current_track.name.clone();
                        *artists = map_join(&current_track.artists, |a| &a.name, ", ");

                        client_pub
                            .send(ClientRequest::GetLyric {
                                track: track.clone(),
                                artists: artists.clone(),
                            })
                            .unwrap_or_default();
                    }
                }
            }
            _ => {}
        }
    }
}
