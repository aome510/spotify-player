use anyhow::Result;
use rspotify::model;

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::*,
    utils,
};

use super::Client;

/// starts the client's request handler
#[tokio::main]
pub async fn start_client_handler(
    state: SharedState,
    client: Client,
    recv: std::sync::mpsc::Receiver<ClientRequest>,
) {
    while let Ok(request) = recv.recv() {
        let state = state.clone();
        let client = client.clone();
        tokio::spawn(async move {
            if let Err(err) = client.handle_request(&state, request).await {
                log::warn!("{:#?}", err);
            }
        });
    }
}

// starts multiple event watchers listening
// to player events and notifying the client
// to make additional update requests if needed
#[tokio::main]
pub async fn start_player_event_watchers(
    state: SharedState,
    send: std::sync::mpsc::Sender<ClientRequest>,
) {
    // start a watcher thread that updates the current playback every `playback_refresh_duration_in_ms` ms.
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

    // start the main event watcher watching for new events every `refresh_duration` ms.
    let refresh_duration = std::time::Duration::from_millis(500);
    loop {
        watch_player_events(&state, &send)
            .await
            .unwrap_or_else(|err| {
                log::warn!(
                    "encountered an error when watching for player events: {}",
                    err
                );
            });

        std::thread::sleep(refresh_duration);
    }
}

async fn watch_player_events(
    state: &SharedState,
    send: &std::sync::mpsc::Sender<ClientRequest>,
) -> Result<()> {
    {
        let player = state.player.read().unwrap();

        // if cannot find the current playback, try to connect to the first avaiable device
        if player.playback.is_none() && !player.devices.is_empty() {
            log::info!(
                "no playback found, try to connect the first available device {}",
                player.devices[0].name
            );
            // only transfering the playback to a new device, not forcing to start the playback
            send.send(ClientRequest::Player(PlayerRequest::TransferPlayback(
                player.devices[0].id.clone(),
                false,
            )))?;
        }

        // update the playback when the current track ends
        let progress_ms = player.playback_progress();
        let duration_ms = player.current_playing_track().map(|t| t.duration);
        let is_playing = match player.playback {
            Some(ref playback) => playback.is_playing,
            None => false,
        };
        if let (Some(progress_ms), Some(duration_ms)) = (progress_ms, duration_ms) {
            if progress_ms >= duration_ms && is_playing {
                send.send(ClientRequest::GetCurrentPlayback)?;
            }
        }
    }

    // update the player's context based on the UI's page state
    match state.ui.lock().unwrap().current_page() {
        PageState::Searching(..) => {
            state.player.write().unwrap().context_id = None;
        }
        PageState::Browsing(id) => {
            let should_update = match state.player.read().unwrap().context_id {
                None => true,
                Some(ref context_id) => context_id != id,
            };
            if should_update {
                utils::update_context(state, Some(id.clone()));
            }
        }
        PageState::CurrentPlaying => {
            let player = state.player.read().unwrap();
            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = player.playback {
                match playback.context {
                    Some(ref context) => {
                        let should_update = match player.context_id {
                            None => true,
                            Some(ref context_id) => context_id.uri() != context.uri,
                        };
                        if should_update && player.context_cache.peek(&context.uri).is_none() {
                            match context._type {
                                model::Type::Playlist => {
                                    let context_id =
                                        ContextId::Playlist(PlaylistId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                model::Type::Album => {
                                    let context_id =
                                        ContextId::Album(AlbumId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                model::Type::Artist => {
                                    let context_id =
                                        ContextId::Artist(ArtistId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                _ => {
                                    log::info!(
                                        "encountered not supported context type: {:#?}",
                                        context._type
                                    )
                                }
                            };
                        }
                    }
                    None => {
                        if player.context_id.is_some() {
                            // the current playback doesn't have a playing context,
                            // update the state's `context_id` to `None`
                            utils::update_context(state, None);
                            log::info!("current playback does not have a playing context");
                        }
                    }
                }
            };
        }
    }

    Ok(())
}
