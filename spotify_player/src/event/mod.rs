use crate::{
    command::Command,
    key::{Key, KeySequence},
    state::*,
    utils::new_list_state,
};
use anyhow::Result;
use crossterm::event::*;
use rand::Rng;
use rspotify::model;
use std::sync::mpsc;
use tokio_stream::StreamExt;

mod popup;
mod window;

#[derive(Debug)]
/// A request that modifies the player's playback
pub enum PlayerRequest {
    NextTrack,
    PreviousTrack,
    ResumePause,
    SeekTrack(u32),
    Repeat,
    Shuffle,
    Volume(u8),
    TransferPlayback(String, bool),
    StartPlayback(Playback),
}

#[derive(Debug)]
/// A request to the client
pub enum ClientRequest {
    GetCurrentUser,
    GetDevices,
    GetUserPlaylists,
    GetUserSavedAlbums,
    GetUserFollowedArtists,
    GetContext(ContextId),
    GetCurrentPlayback,
    GetRecommendations(SeedItem),
    Search(String),
    AddTrackToPlaylist(PlaylistId, TrackId),
    SaveToLibrary(Item),
    Player(PlayerRequest),
}

#[tokio::main]
/// starts a handler to handle terminal events (key pressed, mouse clicked, etc)
pub async fn start_event_handler(send: mpsc::Sender<ClientRequest>, state: SharedState) {
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                log::info!("got event: {:?}", event);
                if let Err(err) = match event {
                    Event::Mouse(event) => handle_mouse_event(event, &send, &state),
                    Event::Key(event) => handle_key_event(event, &send, &state),
                    _ => Ok(()),
                } {
                    log::warn!("failed to handle event: {:#}", err);
                }
            }
            Err(err) => {
                log::warn!("failed to get event: {:#}", err);
            }
        }
    }
}

// handles a terminal mouse event
fn handle_mouse_event(
    event: MouseEvent,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let ui = state.ui.lock();
    // a left click event
    if let MouseEventKind::Down(MouseButton::Left) = event.kind {
        if event.row == ui.progress_bar_rect.y {
            let player = state.player.read();
            let track = player.current_playing_track();
            if let Some(track) = track {
                let position_ms = (track.duration.as_millis() as u32) * (event.column as u32)
                    / (ui.progress_bar_rect.width as u32);
                send.send(ClientRequest::Player(PlayerRequest::SeekTrack(position_ms)))?;
            }
        }
    }
    Ok(())
}

// handle a terminal key pressed event
fn handle_key_event(
    event: KeyEvent,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let key: Key = event.into();

    // lock the UI state because we'll use it many times
    // when handling a terminal event
    let mut ui = state.ui.lock();

    // parse the key sequence from user's previous inputs
    let mut key_sequence = ui.input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let handled = match ui.popup {
        None => {
            // no popup
            match ui.current_page() {
                PageState::Recommendations(..) => {
                    window::handle_key_sequence_for_recommendation_window(
                        &key_sequence,
                        send,
                        state,
                        &mut ui,
                    )?
                }
                PageState::Browsing(_) | PageState::CurrentPlaying => {
                    window::handle_key_sequence_for_context_window(
                        &key_sequence,
                        send,
                        state,
                        &mut ui,
                    )?
                }
                PageState::Searching { .. } => window::handle_key_sequence_for_search_window(
                    &key_sequence,
                    send,
                    state,
                    &mut ui,
                )?,
            }
        }
        Some(_) => popup::handle_key_sequence_for_popup(&key_sequence, send, state, &mut ui)?,
    };

    // if the key sequence is not handled, let the global command handler handle it
    let handled = if !handled {
        match state
            .keymap_config
            .find_command_from_key_sequence(&key_sequence)
        {
            Some(command) => handle_global_command(command, send, state, &mut ui)?,
            None => false,
        }
    } else {
        true
    };

    // if successfully handled the key sequence, clear the key sequence.
    // else, the current key sequence is probably a prefix of a command's shortcut
    if handled {
        ui.input_key_sequence.keys = vec![];
    } else {
        ui.input_key_sequence = key_sequence;
    }
    Ok(())
}

/// handles a global command
fn handle_global_command(
    command: Command,
    send: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
    ui: &mut UIStateGuard,
) -> Result<bool> {
    match command {
        Command::Quit => {
            ui.is_running = false;
        }
        Command::NextTrack => {
            send.send(ClientRequest::Player(PlayerRequest::NextTrack))?;
        }
        Command::PreviousTrack => {
            send.send(ClientRequest::Player(PlayerRequest::PreviousTrack))?;
        }
        Command::ResumePause => {
            send.send(ClientRequest::Player(PlayerRequest::ResumePause))?;
        }
        Command::Repeat => {
            send.send(ClientRequest::Player(PlayerRequest::Repeat))?;
        }
        Command::Shuffle => {
            send.send(ClientRequest::Player(PlayerRequest::Shuffle))?;
        }
        Command::VolumeUp => {
            if let Some(ref playback) = state.player.read().playback {
                if let Some(percent) = playback.device.volume_percent {
                    let volume = std::cmp::min(percent + 5, 100_u32);
                    send.send(ClientRequest::Player(PlayerRequest::Volume(volume as u8)))?;
                }
            }
        }
        Command::VolumeDown => {
            if let Some(ref playback) = state.player.read().playback {
                if let Some(percent) = playback.device.volume_percent {
                    let volume = std::cmp::max(percent.saturating_sub(5_u32), 0_u32);
                    send.send(ClientRequest::Player(PlayerRequest::Volume(volume as u8)))?;
                }
            }
        }
        Command::OpenCommandHelp => {
            ui.popup = Some(PopupState::CommandHelp { offset: 0 });
        }
        Command::RefreshPlayback => {
            send.send(ClientRequest::GetCurrentPlayback)?;
        }
        Command::ShowActionsOnCurrentTrack => {
            if let Some(track) = state.player.read().current_playing_track() {
                let item = Item::Track(track.clone().into());
                let actions = item.actions();
                ui.popup = Some(PopupState::ActionList(item, actions, new_list_state()));
            }
        }
        Command::BrowsePlayingContext => {
            ui.create_new_page(PageState::CurrentPlaying);
        }
        Command::BrowseUserPlaylists => {
            send.send(ClientRequest::GetUserPlaylists)?;
            ui.popup = Some(PopupState::UserPlaylistList(
                PlaylistPopupAction::Browse,
                state.data.read().user_data.playlists.to_vec(),
                new_list_state(),
            ));
        }
        Command::BrowseUserFollowedArtists => {
            send.send(ClientRequest::GetUserFollowedArtists)?;
            ui.popup = Some(PopupState::UserFollowedArtistList(new_list_state()));
        }
        Command::BrowseUserSavedAlbums => {
            send.send(ClientRequest::GetUserSavedAlbums)?;
            ui.popup = Some(PopupState::UserSavedAlbumList(new_list_state()));
        }
        Command::SearchPage => {
            ui.create_new_page(PageState::Searching {
                input: "".to_owned(),
                current_query: "".to_owned(),
            });
            ui.window = WindowState::new_search_state();
        }
        Command::PreviousPage => {
            if ui.history.len() > 1 {
                ui.history.pop();
                ui.popup = None;
                ui.window = WindowState::Unknown;
            }
        }
        Command::SwitchDevice => {
            ui.popup = Some(PopupState::DeviceList(new_list_state()));
            send.send(ClientRequest::GetDevices)?;
        }
        Command::SwitchTheme => {
            ui.popup = Some(PopupState::ThemeList(state.themes(ui), new_list_state()));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
