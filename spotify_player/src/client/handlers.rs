use std::sync::Arc;

use anyhow::Result;
use librespot::core::session::Session;
use rspotify::model::Type;

use crate::{
    event::{ClientRequest, PlayerRequest},
    state::*,
    utils,
};

use super::Client;

/// starts the client's request handler
#[tokio::main]
pub(crate) async fn start_client_handler(
    state: SharedState,
    mut client: Client,
    recv: std::sync::mpsc::Receiver<ClientRequest>,
) {
    while let Ok(request) = recv.recv() {
        // This is not a nice way to handle a request without blocking :/.
        // Kinda a workaround atm.
        // TODO: find a better approach to handle a request without blocking and,
        // at the same time, minimizing the number of clones
        {
            // handle the client request while trying not to block the current thread
            let client = Arc::new(&client);
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(err) = client.handle_request(&state, request).await {
                    log::warn!("{:#?}", err);
                }
            });
        }
    }
}

// starts multiple event watchers listening
// to player events and notifying the client
// to make additional update requests if needed
#[tokio::main]
pub(crate) async fn start_player_event_watchers(
    state: SharedState,
    send: std::sync::mpsc::Sender<ClientRequest>,
    session: Session,
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
        watch_player_events(&state, &send, &session)
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
    session: &Session,
) -> Result<()> {
    {
        let player = state.player.read().unwrap();

        // if cannot find the current playback, try
        // to connect the first avaiable device
        if player.playback.is_none() && !player.devices.is_empty() {
            log::info!(
                "no playback found, try to connect the first available device {}",
                player.devices[0].name
            );
            // only trying to connect, not transfer the current playback
            if let Some(ref id) = player.devices[0].id {
                send.send(ClientRequest::Player(PlayerRequest::TransferPlayback(
                    id.clone(),
                    false,
                )))?;
            }
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
            // manually empty the `context_uri` to trigger
            // context updating when moving from the search page
            // to a previous context page and vice versa
            state.player.write().unwrap().context_uri = "".to_owned();
        }
        PageState::Browsing(uri) => {
            if state.player.read().unwrap().context_uri != *uri {
                utils::update_context(state, uri.clone());
            }
        }
        PageState::CurrentPlaying => {
            let player = state.player.read().unwrap();
            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = player.playback {
                match playback.context {
                    Some(ref context) => {
                        let uri = context.uri.clone();

                        if uri != player.context_uri {
                            utils::update_context(state, uri.clone());
                            if player.context_cache.peek(&uri).is_none() {
                                match context._type {
                                    Type::Playlist => send.send(ClientRequest::GetContext(
                                        ContextURI::Playlist(uri),
                                    ))?,
                                    Type::Album => send
                                        .send(ClientRequest::GetContext(ContextURI::Album(uri)))?,
                                    Type::Artist => send
                                        .send(ClientRequest::GetContext(ContextURI::Artist(uri)))?,
                                    _ => {
                                        send.send(ClientRequest::GetContext(ContextURI::Unknown(
                                            uri,
                                        )))?;
                                        log::info!(
                                            "encountered not supported context type: {:#?}",
                                            context._type
                                        )
                                    }
                                };
                            }
                        }
                    }
                    None => {
                        if !player.context_uri.is_empty() {
                            utils::update_context(state, "".to_string());
                            send.send(ClientRequest::GetContext(ContextURI::Unknown(
                                "".to_string(),
                            )))?;
                            log::info!("current playback does not have a playing context");
                        }
                    }
                }
            };
        }
    }

    Ok(())
}
