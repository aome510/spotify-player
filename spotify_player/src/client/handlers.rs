use anyhow::Context;
use rspotify::model::PlayableItem;
use tracing::Instrument;

use crate::{config, state::*};

#[cfg(feature = "lyric-finder")]
use crate::utils::map_join;

use super::ClientRequest;

struct PlayerEventHandlerState {
    add_track_to_queue_req_timer: std::time::Instant,
}

/// starts the client's request handler
pub async fn start_client_handler(
    state: SharedState,
    client: super::Client,
    client_sub: flume::Receiver<ClientRequest>,
) {
    while let Ok(request) = client_sub.recv_async().await {
        if let Err(err) = client.check_valid_session(&state).await {
            tracing::error!("{err:#}");
            continue;
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

fn handle_playback_change_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
    handler_state: &mut PlayerEventHandlerState,
) -> anyhow::Result<()> {
    let player = state.player.read();
    let (playback, track) = match (
        player.buffered_playback.as_ref(),
        player.current_playing_track(),
    ) {
        (Some(playback), Some(track)) => (playback, track),
        _ => return Ok(()),
    };

    if let Some(progress) = player.playback_progress() {
        // update the playback when the current track ends
        if progress >= track.duration && playback.is_playing {
            client_pub.send(ClientRequest::GetCurrentPlayback)?;
        }
    }

    if let Some(queue) = player.queue.as_ref() {
        // queue needs to be updated if its playing track is different from actual playback's playing track
        if let Some(PlayableItem::Track(queue_track)) = queue.currently_playing.as_ref() {
            if queue_track.id != track.id {
                client_pub.send(ClientRequest::GetCurrentUserQueue)?;
            }
        }
    } else {
        client_pub.send(ClientRequest::GetCurrentUserQueue)?;
    }

    // handle fake track repeat mode
    if playback.fake_track_repeat_state {
        if let Some(progress) = player.playback_progress() {
            // re-queue the current track if it's about to end while
            // ensuring that only one `AddTrackToQueue` request is made
            if progress + chrono::TimeDelta::seconds(5) >= track.duration
                && playback.is_playing
                && handler_state.add_track_to_queue_req_timer.elapsed()
                    > std::time::Duration::from_secs(10)
            {
                tracing::info!(
                    "fake track repeat mode is enabled, add the current track ({}) to queue",
                    track.name
                );
                client_pub.send(ClientRequest::AddTrackToQueue(track.id.clone().unwrap()))?;
                handler_state.add_track_to_queue_req_timer = std::time::Instant::now();
            }
        }
    }

    Ok(())
}

fn handle_page_change_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
) -> anyhow::Result<()> {
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
                if !matches!(id, ContextId::Tracks(_))
                    && !state.data.read().caches.context.contains_key(&id.uri())
                {
                    client_pub.send(ClientRequest::GetContext(id.clone()))?;
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
                    track.clone_from(&current_track.name);
                    *artists = map_join(&current_track.artists, |a| &a.name, ", ");
                    *scroll_offset = 0;

                    client_pub.send(ClientRequest::GetLyric {
                        track: track.clone(),
                        artists: artists.clone(),
                    })?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn handle_player_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
    handler_state: &mut PlayerEventHandlerState,
) -> anyhow::Result<()> {
    handle_page_change_event(state, client_pub).context("handle page change event")?;
    handle_playback_change_event(state, client_pub, handler_state)
        .context("handle playback change event")?;

    Ok(())
}

/// Starts multiple event watchers listening to events and
/// notifying the client to make update requests if needed
pub async fn start_player_event_watchers(
    state: SharedState,
    client_pub: flume::Sender<ClientRequest>,
) {
    let configs = config::get_config();

    // Start a watcher task that updates the playback every `playback_refresh_duration_in_ms` ms.
    // A positive value of `playback_refresh_duration_in_ms` is required to start the watcher.
    if configs.app_config.playback_refresh_duration_in_ms > 0 {
        tokio::task::spawn({
            let client_pub = client_pub.clone();
            let playback_refresh_duration = std::time::Duration::from_millis(
                configs.app_config.playback_refresh_duration_in_ms,
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

    let refresh_duration = std::time::Duration::from_secs(1);
    let mut handler_state = PlayerEventHandlerState {
        add_track_to_queue_req_timer: std::time::Instant::now(),
    };

    loop {
        tokio::time::sleep(refresh_duration).await;
        if let Err(err) = handle_player_event(&state, &client_pub, &mut handler_state) {
            tracing::error!("Encounter error when handling player event: {err:#}");
        }
    }
}
