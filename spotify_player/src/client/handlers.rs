use std::time::{Duration, Instant};

use anyhow::Context;
use rspotify::model::Id;
use tracing::Instrument;

use crate::{
    config,
    state::{ContextId, ContextPageType, ContextPageUIState, PageState, PlayableId, SharedState},
};

use crate::utils::map_join;

use super::ClientRequest;

struct PlayerEventHandlerState {
    ended_playable_uri: Option<String>,
    last_get_context: Instant,
    last_playback_refresh: Instant,
    last_queue_refresh: Option<(String, Instant)>,
}

/// Check if the error returned from Spotify API is a terminal authentication/token error
fn is_auth_error(err: &anyhow::Error) -> bool {
    let err_str = format!("{err:#}");
    err_str.contains("Token is not valid")
        || err_str.contains("invalid_grant")
        || err_str.contains("status code 401")
        || err_str.contains("status code 403")
}

/// starts the client's request handler
pub async fn start_client_handler(
    state: &SharedState,
    client: &super::AppClient,
    client_sub: &flume::Receiver<ClientRequest>,
) {
    while let Ok(request) = client_sub.recv_async().await {
        let state = state.clone();
        let client = client.clone();
        let span = tracing::info_span!("client_request", request = ?request);

        tokio::task::spawn(
            async move {
                if let Err(err) = client.handle_request(&state, request).await {
                    tracing::error!("Failed to handle client request: {err:#}");

                    if is_auth_error(&err) {
                        tracing::warn!("Authentication error detected, clearing token cache to force re-authentication");
                        let cache_path = crate::config::get_config().cache_folder.join("user_client_token.json");
                        if cache_path.exists() {
                            if let Err(e) = std::fs::remove_file(&cache_path) {
                                tracing::error!("Failed to remove token cache file {}: {e:#}", cache_path.display());
                            }
                        }
                        if let Ok(mut token_guard) = client.get_token().lock().await {
                            *token_guard = None;
                        }
                    }
                }
            }
            .instrument(span),
        );
    }
}

/// Interval between background session-validity checks.
const SESSION_CHECK_INTERVAL: Duration = Duration::from_secs(1);
const CONTEXT_REFRESH_THROTTLE: Duration = Duration::from_secs(5);
const QUEUE_REFRESH_THROTTLE: Duration = Duration::from_secs(5);

pub async fn start_session_watcher(state: SharedState, client: super::AppClient) {
    let mut interval = tokio::time::interval(SESSION_CHECK_INTERVAL);
    // If a check ever runs long (e.g. a slow reconnect), skip missed ticks
    // rather than firing them back-to-back.
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        interval.tick().await;
        if let Err(err) = client.check_valid_session(&state).await {
            tracing::error!("Failed to check/reconnect the client's session: {err:#}");
        }
    }
}

fn handle_playback_change_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
    handler_state: &mut PlayerEventHandlerState,
) -> anyhow::Result<()> {
    let player = state.player.read();
    let (playback, id, duration) = match (
        player.buffered_playback.as_ref(),
        player.currently_playing(),
    ) {
        (Some(playback), Some(rspotify::model::PlayableItem::Track(track))) => (
            playback,
            PlayableId::Track(track.id.clone().expect("null track_id")),
            track.duration,
        ),
        (Some(playback), Some(rspotify::model::PlayableItem::Episode(episode))) => (
            playback,
            PlayableId::Episode(episode.id.clone()),
            episode.duration,
        ),
        _ => return Ok(()),
    };
    let playable_uri = id.uri();

    let playback_ended = player
        .playback_progress()
        .is_some_and(|progress| progress >= duration && playback.is_playing);
    if playback_ended && handler_state.ended_playable_uri.as_deref() != Some(&playable_uri) {
        client_pub.send(ClientRequest::GetCurrentPlayback)?;
        handler_state.ended_playable_uri = Some(playable_uri.clone());
    } else if !playback_ended {
        handler_state.ended_playable_uri = None;
    }

    let queue_needs_refresh = player.queue.as_ref().is_none_or(|queue| {
        queue
            .currently_playing
            .as_ref()
            .is_none_or(|queue_item| queue_item.id().expect("null track_id") != id)
    });
    if queue_needs_refresh {
        let should_refresh =
            handler_state
                .last_queue_refresh
                .as_ref()
                .is_none_or(|(last_uri, timer)| {
                    last_uri != &playable_uri || timer.elapsed() >= QUEUE_REFRESH_THROTTLE
                });
        if should_refresh {
            handler_state.last_queue_refresh = Some((playable_uri, Instant::now()));
            client_pub.send(ClientRequest::GetCurrentUserQueue)?;
        }
    } else if !queue_needs_refresh {
        handler_state.last_queue_refresh = None;
    }

    Ok(())
}

fn handle_page_change_event(
    state: &SharedState,
    client_pub: &flume::Sender<ClientRequest>,
    handler_state: &mut PlayerEventHandlerState,
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

            let new_id = if *id == expected_id {
                false
            } else {
                // update the context state and request new data when moving to a new context page
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
                            ContextId::Show(_) => ContextPageUIState::new_show(),
                        });
                    }
                    None => {
                        *page_state = None;
                    }
                }
                true
            };

            // request new context's data if not found in memory
            // To avoid making too many requests, only request if context id is changed
            // or it's been a while since the last request.
            if let Some(id) = id {
                if !matches!(id, ContextId::Tracks(_))
                    && !state.data.read().caches.context.contains_key(&id.uri())
                    && (new_id
                        || handler_state.last_get_context.elapsed() > CONTEXT_REFRESH_THROTTLE)
                {
                    client_pub.send(ClientRequest::GetContext(id.clone()))?;
                    handler_state.last_get_context = Instant::now();
                }
            }
        }

        PageState::Lyrics {
            track_uri,
            track,
            artists,
        } => {
            if let Some(rspotify::model::PlayableItem::Track(current_track)) =
                state.player.read().currently_playing()
            {
                if current_track.name != *track {
                    if let Some(id) = &current_track.id {
                        tracing::info!("Currently playing track \"{}\" is different from the track \"{track}\" shown up in the lyrics page. Fetching new track's lyrics...", current_track.name);
                        track.clone_from(&current_track.name);
                        *artists = map_join(&current_track.artists, |a| &a.name, ", ");
                        *track_uri = id.uri();
                        client_pub.send(ClientRequest::GetLyrics {
                            track_id: id.clone_static(),
                        })?;
                    }
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
    handle_page_change_event(state, client_pub, handler_state)
        .context("handle page change event")?;
    handle_playback_change_event(state, client_pub, handler_state)
        .context("handle playback change event")?;

    Ok(())
}

/// Starts event watcher listening to events and making update requests to the client if needed
pub fn start_player_event_watcher(state: &SharedState, client_pub: &flume::Sender<ClientRequest>) {
    let configs = config::get_config();

    let refresh_duration = Duration::from_millis(100);
    let playback_refresh_duration =
        Duration::from_millis(configs.app_config.playback_refresh_duration_in_ms);
    let mut handler_state = PlayerEventHandlerState {
        last_get_context: Instant::now(),
        last_playback_refresh: Instant::now(),
        ended_playable_uri: None,
        last_queue_refresh: None,
    };

    loop {
        // periodically refresh the playback state (if enabled in config)
        if configs.app_config.playback_refresh_duration_in_ms > 0
            && handler_state.last_playback_refresh.elapsed() >= playback_refresh_duration
        {
            client_pub
                .send(ClientRequest::GetCurrentPlayback)
                .unwrap_or_default();
            handler_state.last_playback_refresh = Instant::now();
        }

        if let Err(err) = handle_player_event(state, client_pub, &mut handler_state) {
            tracing::error!("Encounter error when handling player event: {err:#}");
        }

        std::thread::sleep(refresh_duration);
    }
}
