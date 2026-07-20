use std::{
    future::Future,
    time::{Duration, Instant},
};

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
    get_context_timer: Instant,
    last_playback_refresh_timer: Instant,
}

async fn dispatch_requests<T, Handler, HandlerFuture, HandlerOutput, RequiresOrderedExecution>(
    request_sub: flume::Receiver<T>,
    requires_ordered_execution: RequiresOrderedExecution,
    handler: Handler,
) where
    T: Send + 'static,
    Handler: Fn(T) -> HandlerFuture + Clone + Send + 'static,
    HandlerFuture: Future<Output = HandlerOutput> + Send + 'static,
    HandlerOutput: Send + 'static,
    RequiresOrderedExecution: Fn(&T) -> bool,
{
    let (ordered_pub, ordered_sub) = flume::unbounded();
    let ordered_handler = handler.clone();
    let ordered_task = tokio::task::spawn(async move {
        while let Ok(request) = ordered_sub.recv_async().await {
            if let Err(err) = tokio::task::spawn(ordered_handler(request)).await {
                tracing::error!("Ordered client request task failed: {err:#}");
            }
        }
    });
    let mut concurrent_tasks = tokio::task::JoinSet::new();

    loop {
        while let Some(result) = concurrent_tasks.try_join_next() {
            if let Err(err) = result {
                tracing::error!("Concurrent client request task failed: {err:#}");
            }
        }

        tokio::select! {
            request = request_sub.recv_async() => {
                let Ok(request) = request else {
                    break;
                };
                if requires_ordered_execution(&request) {
                    if ordered_pub.send(request).is_err() {
                        break;
                    }
                } else {
                    let handler = handler.clone();
                    concurrent_tasks.spawn(async move {
                        handler(request).await
                    });
                }
            }
            result = concurrent_tasks.join_next(), if !concurrent_tasks.is_empty() => {
                if let Some(Err(err)) = result {
                    tracing::error!("Concurrent client request task failed: {err:#}");
                }
            }
        }
    }

    drop(ordered_pub);
    if let Err(err) = ordered_task.await {
        tracing::error!("Ordered client request task failed: {err:#}");
    }
    while let Some(result) = concurrent_tasks.join_next().await {
        if let Err(err) = result {
            tracing::error!("Concurrent client request task failed: {err:#}");
        }
    }
}

/// starts the client's request handler
pub async fn start_client_handler(
    state: &SharedState,
    client: &super::AppClient,
    client_sub: &flume::Receiver<ClientRequest>,
) {
    let state = state.clone();
    let client = client.clone();
    dispatch_requests(
        client_sub.clone(),
        ClientRequest::requires_ordered_execution,
        move |request| {
            let state = state.clone();
            let client = client.clone();
            let span = tracing::info_span!("client_request", request = ?request);
            async move {
                if let Err(err) = client.handle_request(&state, request).await {
                    tracing::error!("Failed to handle client request: {err:#}");
                }
            }
            .instrument(span)
        },
    )
    .await;
}

/// Interval between background session-validity checks.
const SESSION_CHECK_INTERVAL: Duration = Duration::from_secs(1);

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

#[cfg(test)]
mod tests {
    use super::dispatch_requests;
    use std::time::Duration;

    struct DropSignal(flume::Sender<()>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    struct TestRequest {
        id: u8,
        ordered: bool,
        wait_for: Option<flume::Receiver<()>>,
    }

    fn test_dispatcher() -> (
        flume::Sender<TestRequest>,
        flume::Receiver<u8>,
        tokio::task::JoinHandle<()>,
    ) {
        let (request_pub, request_sub) = flume::unbounded();
        let (completed_pub, completed_sub) = flume::unbounded();

        let task = tokio::task::spawn(dispatch_requests(
            request_sub,
            |request: &TestRequest| request.ordered,
            move |request: TestRequest| {
                let completed_pub = completed_pub.clone();
                async move {
                    if let Some(wait_for) = request.wait_for {
                        wait_for.recv_async().await.unwrap();
                    }
                    completed_pub.send_async(request.id).await.unwrap();
                }
            },
        ));

        (request_pub, completed_sub, task)
    }

    #[tokio::test]
    async fn ordered_requests_finish_in_receive_order() {
        let (gate_pub, gate_sub) = flume::bounded(1);
        let (request_pub, completed_sub, task) = test_dispatcher();
        request_pub
            .send(TestRequest {
                id: 1,
                ordered: true,
                wait_for: Some(gate_sub),
            })
            .unwrap();
        request_pub
            .send(TestRequest {
                id: 2,
                ordered: true,
                wait_for: None,
            })
            .unwrap();
        drop(request_pub);

        assert!(
            tokio::time::timeout(Duration::from_millis(50), completed_sub.recv_async())
                .await
                .is_err()
        );
        gate_pub.send(()).unwrap();
        task.await.unwrap();

        assert_eq!(completed_sub.try_iter().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[tokio::test]
    async fn concurrent_requests_are_not_blocked_by_ordered_requests() {
        let (gate_pub, gate_sub) = flume::bounded(1);
        let (request_pub, completed_sub, task) = test_dispatcher();
        request_pub
            .send(TestRequest {
                id: 1,
                ordered: true,
                wait_for: Some(gate_sub),
            })
            .unwrap();
        request_pub
            .send(TestRequest {
                id: 2,
                ordered: false,
                wait_for: None,
            })
            .unwrap();
        drop(request_pub);

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), completed_sub.recv_async())
                .await
                .unwrap()
                .unwrap(),
            2
        );
        gate_pub.send(()).unwrap();
        task.await.unwrap();

        assert_eq!(completed_sub.try_iter().collect::<Vec<_>>(), vec![1]);
    }

    #[tokio::test]
    async fn completed_concurrent_tasks_are_reaped_while_request_stream_is_idle() {
        let (request_pub, request_sub) = flume::unbounded();
        let (gate_pub, gate_sub) = flume::bounded(1);
        let (started_pub, started_sub) = flume::bounded(1);
        let (reaped_pub, reaped_sub) = flume::bounded(1);
        let task = tokio::task::spawn(dispatch_requests(
            request_sub,
            |_: &()| false,
            move |()| {
                let gate_sub = gate_sub.clone();
                let started_pub = started_pub.clone();
                let reaped_pub = reaped_pub.clone();
                async move {
                    started_pub.send_async(()).await.unwrap();
                    gate_sub.recv_async().await.unwrap();
                    DropSignal(reaped_pub)
                }
            },
        ));

        request_pub.send(()).unwrap();
        started_sub.recv_async().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        gate_pub.send(()).unwrap();

        tokio::time::timeout(Duration::from_secs(1), reaped_sub.recv_async())
            .await
            .unwrap()
            .unwrap();

        drop(request_pub);
        task.await.unwrap();
    }
}
