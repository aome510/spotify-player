use std::time::{Duration, Instant};

use anyhow::Context;
use rspotify::model::Id;
use tracing::Instrument;

use crate::{
    config,
    state::{
        ContextCursor, ContextId, ContextPageType, ContextPageUIState, PageState, PlayableId,
        SharedState,
    },
};

use crate::utils::map_join;

use super::ClientRequest;

struct PlayerEventHandlerState {
    get_context_timer: Instant,
    last_playback_refresh_timer: Instant,
}

/// starts the client's request handler
pub async fn start_client_handler(
    state: &SharedState,
    client: &super::AppClient,
    client_sub: &flume::Receiver<ClientRequest>,
) {
    while let Ok(request) = client_sub.recv_async().await {
        if let Err(err) = client.check_valid_session(state).await {
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

    if let Some(progress) = player.playback_progress() {
        // update the playback when the current track ends
        if progress >= duration && playback.is_playing {
            client_pub.send(ClientRequest::GetCurrentPlayback)?;
        }
    }

    if let Some(queue) = player.queue.as_ref() {
        // queue needs to be updated if its playing track is different from actual playback's playing track
        if let Some(queue_track) = queue.currently_playing.as_ref() {
            if queue_track.id().expect("null track_id") != id {
                client_pub.send(ClientRequest::GetCurrentUserQueue)?;
            }
        }
    } else {
        client_pub.send(ClientRequest::GetCurrentUserQueue)?;
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
                ContextPageType::CurrentPlaying { tracks_id } => {
                    // If we have a stored tracks_id, use it; otherwise get from player
                    if let Some(tracks_id) = tracks_id {
                        Some(ContextId::Tracks(tracks_id.clone()))
                    } else {
                        state.player.read().playing_context_id()
                    }
                }
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
                        || handler_state.get_context_timer.elapsed() > Duration::from_secs(5))
                {
                    client_pub.send(ClientRequest::GetContext(id.clone()))?;
                    handler_state.get_context_timer = Instant::now();
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
    handle_playback_change_event(state, client_pub).context("handle playback change event")?;

    Ok(())
}

/// Starts event watcher listening to events and making update requests to the client if needed
pub fn start_player_event_watcher(state: &SharedState, client_pub: &flume::Sender<ClientRequest>) {
    let configs = config::get_config();

    let refresh_duration = Duration::from_millis(100);
    let playback_refresh_duration =
        Duration::from_millis(configs.app_config.playback_refresh_duration_in_ms);
    let mut handler_state = PlayerEventHandlerState {
        get_context_timer: Instant::now(),
        last_playback_refresh_timer: Instant::now(),
    };

    loop {
        // periodically refresh the playback state (if enabled in config)
        if configs.app_config.playback_refresh_duration_in_ms > 0
            && handler_state.last_playback_refresh_timer.elapsed() >= playback_refresh_duration
        {
            client_pub
                .send(ClientRequest::GetCurrentPlayback)
                .unwrap_or_default();
            handler_state.last_playback_refresh_timer = Instant::now();
        }

        if let Err(err) = handle_player_event(state, client_pub, &mut handler_state) {
            tracing::error!("Encounter error when handling player event: {err:#}");
        }

        std::thread::sleep(refresh_duration);
    }
}

// ---------------------------------------------------------------------------
// Queue feeder
// ---------------------------------------------------------------------------

/// Builds a shuffled list of indices into a context's track list, excluding `current_index`
fn build_shuffle_ids(track_count: usize, current_index: usize) -> Vec<usize> {
    use rand::seq::SliceRandom;
    let mut order: Vec<usize> = (0..track_count).collect();
    order.remove(current_index);
    order.shuffle(&mut rand::rng());
    order
}

/// Returns all playable IDs from a context in their natural order.
fn context_playable_ids(context: &crate::state::Context) -> Vec<PlayableId<'static>> {
    use crate::state::Context;
    match context {
        Context::Playlist { tracks, .. }
        | Context::Album { tracks, .. }
        | Context::Tracks { tracks, .. } => tracks
            .iter()
            .map(|t| PlayableId::Track(t.id.clone()))
            .collect(),
        Context::Artist { top_tracks, .. } => top_tracks
            .iter()
            .map(|t| PlayableId::Track(t.id.clone()))
            .collect(),
        Context::Show { episodes, .. } => episodes
            .iter()
            .map(|e| PlayableId::Episode(e.id.clone()))
            .collect(),
    }
}

fn resolve_next_item(state: &SharedState, current_track_uri: &str) -> Option<PlayableId<'static>> {
    let mut player = state.player.write();

    // Get snapshot playback state
    let (repeat_state, shuffle_state) = player
        .buffered_playback
        .as_ref()
        .map_or((rspotify::model::RepeatState::Off, false), |p| {
            (p.repeat_state, p.shuffle_state)
        });

    let cursor = player.context_cursor.as_mut().expect("cursor available");

    let track_count = cursor.ids.len();
    let current_index = cursor
        .ids
        .iter()
        .position(|id| id.uri() == current_track_uri)
        .expect("current track not available in context");

    if shuffle_state && cursor.shuffle_ids.is_none() {
        // initialize shuffle order if shuffle is on but shuffle_ids is not built yet
        cursor.shuffle_ids = Some(build_shuffle_ids(track_count, current_index));
    } else if !shuffle_state {
        // clear shuffle order if shuffle is off
        cursor.shuffle_ids = None;
    }

    // determine next index based on shuffle state
    let next_index = if let Some(ids) = cursor.shuffle_ids.as_mut() {
        ids.pop()
            .or_else(|| {
                // Shuffled order exhausted — rebuild for the next context loop.
                *ids = build_shuffle_ids(track_count, current_index);
                ids.pop()
            })
            .unwrap_or_default()
    } else {
        (current_index + 1) % track_count
    };

    // determine next index based on repeat state
    let next_index = match repeat_state {
        rspotify::model::RepeatState::Track => {
            // track repeat is already automatically handled by Spotify queue
            return None;
        }
        rspotify::model::RepeatState::Context => next_index,
        rspotify::model::RepeatState::Off => {
            if next_index == 0 {
                // Reached the end of the context and repeat is off — no next item.
                return None;
            }
            next_index
        }
    };

    cursor.ids.get(next_index).cloned()
}

pub async fn start_queue_feeder(state: SharedState, client_pub: &flume::Sender<ClientRequest>) {
    let poll_interval = tokio::time::Duration::from_millis(500);
    loop {
        tokio::time::sleep(poll_interval).await;

        let player = state.player.read();

        let current_track_uri = match player.currently_playing() {
            Some(rspotify::model::PlayableItem::Track(t)) => {
                t.id.as_ref().map(rspotify::prelude::Id::uri)
            }
            Some(rspotify::model::PlayableItem::Episode(e)) => Some(e.id.uri()),
            Some(rspotify::model::PlayableItem::Unknown(_)) | None => None,
        };

        // Nothing has changed — keep waiting.
        if current_track_uri == player.queue_feeder_last_seen_track {
            continue;
        }

        let Some(current_track_uri) = current_track_uri else {
            continue;
        };

        let Some(context_id) = player.playing_context_id() else {
            continue;
        };

        let context_changed = {
            player
                .context_cursor
                .as_ref()
                .is_none_or(|c| c.context_id != context_id)
        };
        drop(player);

        if context_changed {
            let Some(ids) = state
                .data
                .read()
                .caches
                .context
                .get(&context_id.uri())
                .map(context_playable_ids)
            else {
                client_pub
                    .send(ClientRequest::GetContext(context_id.clone()))
                    .unwrap_or_default();
                continue;
            };

            tracing::info!("Context changed ({context_id:?}), resetting queue feeder cursor",);

            state.player.write().context_cursor = Some(ContextCursor {
                context_id,
                ids,
                shuffle_ids: None, // will be built later if shuffle is on
            });
        }

        if let Some(next_item) = resolve_next_item(&state, &current_track_uri) {
            tracing::info!(
                "Queue feeder: enqueueing next item {} after track transition",
                next_item.uri()
            );

            client_pub
                .send(ClientRequest::AddPlayableToQueue(next_item))
                .unwrap_or_default();
        }

        state.player.write().queue_feeder_last_seen_track = Some(current_track_uri);
    }
}
