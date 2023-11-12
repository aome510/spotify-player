use rspotify::model::PlayableItem;
use tracing::Instrument;

use crate::{event::ClientRequest, state::*};

#[cfg(feature = "lyric-finder")]
use crate::utils::map_join;

/// starts the client's request handler
pub async fn start_client_handler(
    state: SharedState,
    client: super::Client,
    client_sub: flume::Receiver<ClientRequest>,
) {
    while let Ok(request) = client_sub.recv_async().await {
        if client.spotify.session().await.is_invalid() {
            tracing::info!("Spotify client's session is invalid, re-creating a new session...");
            if let Err(err) = client.new_session(&state).await {
                tracing::error!("Failed to create a new session: {err:#}");
                continue;
            }
        }

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

async fn handle_track_end_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
) -> anyhow::Result<()> {
    let mut needs_update = false;
    {
        let player = state.player.read();
        if let (Some(playback), Some(track)) =
            (player.playback.as_ref(), player.current_playing_track())
        {
            if let Some(progress) = player.playback_progress() {
                needs_update = progress >= track.duration && playback.is_playing;
            }
        }
    }
    if needs_update {
        client_pub
            .send_async(ClientRequest::GetCurrentPlayback)
            .await?;
    }

    Ok(())
}

async fn handle_queue_change_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
) -> anyhow::Result<()> {
    let mut needs_update = false;
    {
        let player = state.player.read();
        if let (Some(track), Some(queue)) = (player.current_playing_track(), player.queue.as_ref())
        {
            if let Some(PlayableItem::Track(queue_track)) = queue.currently_playing.as_ref() {
                // if the currently playing track in queue is different from the actual
                // currently playing track stored inside player's playback, update the queue
                needs_update = queue_track.id != track.id;
            }
        }
    }
    if needs_update {
        // In addition to `GetCurrentUserQueue` request, also update the current playback
        // as there can be a mismatch between the current playback and the current queue.
        client_pub
            .send_async(ClientRequest::GetCurrentPlayback)
            .await?;
        client_pub
            .send_async(ClientRequest::GetCurrentUserQueue)
            .await?;
    }

    Ok(())
}

/// Starts multiple event watchers listening to events and
/// notifying the client to make update requests if needed
pub async fn start_player_event_watchers(
    state: SharedState,
    client_pub: flume::Sender<ClientRequest>,
) {
    // Start a watcher task that updates the playback every `playback_refresh_duration_in_ms` ms.
    // A positive value of `playback_refresh_duration_in_ms` is required to start the watcher.
    if state.configs.app_config.playback_refresh_duration_in_ms > 0 {
        tokio::task::spawn({
            let client_pub = client_pub.clone();
            let playback_refresh_duration = std::time::Duration::from_millis(
                state.configs.app_config.playback_refresh_duration_in_ms,
            );
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

    // Start the first task for handling "low-frequency" events.
    // An event can be categorized as "low-frequency" when
    // - we don't need to handle it "immediately"
    // - we want to avoid handling it more than once within a period of time
    tokio::task::spawn({
        let client_pub = client_pub.clone();
        let state = state.clone();
        async move {
            let refresh_duration = std::time::Duration::from_millis(1000); // frequency = 1Hz
            loop {
                tokio::time::sleep(refresh_duration).await;

                if let Err(err) = handle_track_end_event(&state, &client_pub).await {
                    tracing::error!("Encountered error when handling track end event: {err:#}");
                }
                if let Err(err) = handle_queue_change_event(&state, &client_pub).await {
                    tracing::error!("Encountered error when handling queue change event: {err:#}");
                }
            }
        }
    });

    // Start the second task (main blocking task) for handling "high-frequency" events.
    // An event is categorized as "high-frequency" when
    // - we want to handle it "immediately" to prevent users from experiencing a noticeable delay
    let refresh_duration = std::time::Duration::from_millis(200); // frequency = 5Hz

    // This timer is used to avoid making multiple `GetContext` requests in a short period of time.
    let mut last_request_timer = std::time::Instant::now();

    loop {
        tokio::time::sleep(refresh_duration).await;

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

                // update the context state and request new data when moving to a new context page
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
                }

                // request new context's data if not found in memory
                if let Some(id) = id {
                    if !state.data.read().caches.context.contains_key(&id.uri())
                        && last_request_timer.elapsed() >= std::time::Duration::from_secs(1)
                    {
                        last_request_timer = std::time::Instant::now();
                        client_pub
                            .send(ClientRequest::GetContext(id.clone()))
                            .unwrap_or_default();
                    }
                }
            }

            #[cfg(feature = "lyric-finder")]
            PageState::Lyric {
                track,
                artists,
                scroll_offset,
            } => {
                if let Some(current_track) = state.player.read().current_playing_track() {
                    if current_track.name != *track {
                        tracing::info!("Current playing track \"{}\" is different from the track \"{track}\" shown up in the lyric page. Updating the track and fetching its lyric...", current_track.name);
                        *track = current_track.name.clone();
                        *artists = map_join(&current_track.artists, |a| &a.name, ", ");
                        *scroll_offset = 0;

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
