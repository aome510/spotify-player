use crate::{
    command::Command,
    key::{Key, KeySequence},
    state::*,
    utils::new_list_state,
};
use anyhow::Result;
use rand::Rng;
use tokio::sync::mpsc;

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
    Reconnect,
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
    #[cfg(feature = "streaming")]
    NewSpircConnection,
}

/// starts a terminal event handler (key pressed, mouse clicked, etc)
pub fn start_event_handler(state: SharedState, client_pub: mpsc::Sender<ClientRequest>) {
    while let Ok(event) = crossterm::event::read() {
        tracing::info!("got a terminal event: {:?}", event);

        if let Err(err) = match event {
            crossterm::event::Event::Mouse(event) => handle_mouse_event(event, &client_pub, &state),
            crossterm::event::Event::Key(event) => handle_key_event(event, &client_pub, &state),
            _ => Ok(()),
        } {
            tracing::warn!("failed to handle event: {}", err);
        }
    }
}

// handles a terminal mouse event
fn handle_mouse_event(
    event: crossterm::event::MouseEvent,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let ui = state.ui.lock();
    // a left click event
    if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = event.kind
    {
        if event.row == ui.progress_bar_rect.y {
            // calculate the seek position (in ms) based on the clicked position,
            // the progress bar's width and the track's duration (in ms)

            let player = state.player.read();
            let track = player.current_playing_track();
            if let Some(track) = track {
                let position_ms = (track.duration.as_millis() as u32) * (event.column as u32)
                    / (ui.progress_bar_rect.width as u32);
                client_pub
                    .blocking_send(ClientRequest::Player(PlayerRequest::SeekTrack(position_ms)))?;
            }
        }
    }
    Ok(())
}

// handle a terminal key pressed event
fn handle_key_event(
    event: crossterm::event::KeyEvent,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<()> {
    let key: Key = event.into();

    // parse the key sequence from user's previous inputs
    let mut key_sequence = state.ui.lock().input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let ui = state.ui.lock();
    let handled = match ui.popup {
        None => {
            // no popup
            match ui.current_page() {
                PageState::Library => {
                    drop(ui);
                    window::handle_key_sequence_for_library_window(&key_sequence, state)?
                }
                PageState::Recommendations(..) => {
                    drop(ui);
                    window::handle_key_sequence_for_recommendation_window(
                        &key_sequence,
                        client_pub,
                        state,
                    )?
                }
                PageState::Context(..) => {
                    drop(ui);
                    window::handle_key_sequence_for_context_window(
                        &key_sequence,
                        client_pub,
                        state,
                    )?
                }
                PageState::Searching { .. } => {
                    drop(ui);
                    window::handle_key_sequence_for_search_window(&key_sequence, client_pub, state)?
                }
            }
        }
        Some(_) => {
            drop(ui);
            popup::handle_key_sequence_for_popup(&key_sequence, client_pub, state)?
        }
    };

    // if the key sequence is not handled, let the global command handler handle it
    let handled = if !handled {
        match state
            .keymap_config
            .find_command_from_key_sequence(&key_sequence)
        {
            Some(command) => handle_global_command(command, client_pub, state)?,
            None => false,
        }
    } else {
        true
    };

    // if successfully handled the key sequence, clear the key sequence.
    // else, the current key sequence is probably a prefix of a command's shortcut
    if handled {
        state.ui.lock().input_key_sequence.keys = vec![];
    } else {
        state.ui.lock().input_key_sequence = key_sequence;
    }
    Ok(())
}

/// handles a global command
fn handle_global_command(
    command: Command,
    client_pub: &mpsc::Sender<ClientRequest>,
    state: &SharedState,
) -> Result<bool> {
    let mut ui = state.ui.lock();

    match command {
        Command::Quit => {
            ui.is_running = false;
        }
        Command::NextTrack => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::NextTrack))?;
        }
        Command::PreviousTrack => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::PreviousTrack))?;
        }
        Command::ResumePause => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::ResumePause))?;
        }
        Command::Repeat => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Repeat))?;
        }
        Command::Shuffle => {
            client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Shuffle))?;
        }
        Command::VolumeUp => {
            if let Some(ref playback) = state.player.read().playback {
                if let Some(percent) = playback.device.volume_percent {
                    let volume = std::cmp::min(percent + 5, 100_u32);
                    client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Volume(
                        volume as u8,
                    )))?;
                }
            }
        }
        Command::VolumeDown => {
            if let Some(ref playback) = state.player.read().playback {
                if let Some(percent) = playback.device.volume_percent {
                    let volume = std::cmp::max(percent.saturating_sub(5_u32), 0_u32);
                    client_pub.blocking_send(ClientRequest::Player(PlayerRequest::Volume(
                        volume as u8,
                    )))?;
                }
            }
        }
        Command::OpenCommandHelp => {
            ui.popup = Some(PopupState::CommandHelp { offset: 0 });
        }
        Command::RefreshPlayback => {
            client_pub.blocking_send(ClientRequest::GetCurrentPlayback)?;
        }
        Command::ShowActionsOnCurrentTrack => {
            if let Some(track) = state.player.read().current_playing_track() {
                if let Some(track) = Track::try_from_full_track(track.clone()) {
                    ui.popup = Some(PopupState::ActionList(Item::Track(track), new_list_state()));
                }
            }
        }
        Command::BrowsePlayingContext => {
            ui.create_new_page(PageState::Context(None, ContextPageType::CurrentPlaying));
        }
        Command::BrowseUserPlaylists => {
            client_pub.blocking_send(ClientRequest::GetUserPlaylists)?;
            ui.popup = Some(PopupState::UserPlaylistList(
                PlaylistPopupAction::Browse,
                new_list_state(),
            ));
        }
        Command::BrowseUserFollowedArtists => {
            client_pub.blocking_send(ClientRequest::GetUserFollowedArtists)?;
            ui.popup = Some(PopupState::UserFollowedArtistList(new_list_state()));
        }
        Command::BrowseUserSavedAlbums => {
            client_pub.blocking_send(ClientRequest::GetUserSavedAlbums)?;
            ui.popup = Some(PopupState::UserSavedAlbumList(new_list_state()));
        }
        Command::LibraryPage => {
            ui.create_new_page(PageState::Library);
        }
        Command::SearchPage => {
            ui.create_new_page(PageState::Searching {
                input: "".to_owned(),
                current_query: "".to_owned(),
            });
        }
        Command::PreviousPage => {
            if ui.history.len() > 1 {
                ui.history.pop();
                ui.popup = None;
                ui.window = WindowState::Unknown;

                // empty the previous page's `context_id` to force
                // updating the context page's window state and requesting the context data
                if let PageState::Context(ref mut context_id, _) = ui.current_page_mut() {
                    *context_id = None;
                }
            }
        }
        Command::SwitchDevice => {
            ui.popup = Some(PopupState::DeviceList(new_list_state()));
            client_pub.blocking_send(ClientRequest::GetDevices)?;
        }
        Command::SwitchTheme => {
            // get the available themes with the current theme moved to the first position
            let mut themes = state.theme_config.themes.clone();
            let id = themes.iter().position(|t| t.name == ui.theme.name);
            if let Some(id) = id {
                let theme = themes.remove(id);
                themes.insert(0, theme);
            }

            ui.popup = Some(PopupState::ThemeList(themes, new_list_state()));
        }
        Command::ReconnectIntegratedClient => {
            client_pub.blocking_send(ClientRequest::NewSpircConnection)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
