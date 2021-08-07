use crate::{
    command::Command,
    key::{Key, KeySequence},
    state,
};
use anyhow::Result;
use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use std::sync::mpsc;
use tokio::stream::StreamExt;
use tui::widgets::ListState;

#[derive(Debug)]
pub enum Context {
    Playlist(String),
    Album(String),
    Unknown,
}

#[derive(Debug)]
/// An event to communicate with the client
pub enum Event {
    GetDevices,
    GetCurrentPlayback,
    RefreshToken,
    NextTrack,
    PreviousTrack,
    ResumePause,
    Repeat,
    Shuffle,
    SwitchContext(Context),
    PlayTrack(String, String),
    PlayContext(String),
    TransferPlayback(String),
}

impl From<term_event::KeyEvent> for Key {
    fn from(event: term_event::KeyEvent) -> Self {
        match event.modifiers {
            KeyModifiers::NONE => Key::None(event.code),
            KeyModifiers::ALT => Key::Alt(event.code),
            KeyModifiers::CONTROL => Key::Ctrl(event.code),
            KeyModifiers::SHIFT => Key::None(event.code),
            _ => unreachable!(),
        }
    }
}

#[tokio::main]
/// starts the application's event stream that pools and handles events from the terminal
pub async fn start_event_stream(send: mpsc::Sender<Event>, state: state::SharedState) {
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                log::info!("got event: {:?}", event);
                if let Err(err) = handle_terminal_event(event, &send, &state) {
                    log::warn!("failed to handle event: {:#}", err);
                }
            }
            Err(err) => {
                log::warn!("failed to get event: {:#}", err);
            }
        }
    }
}

fn handle_terminal_event(
    event: term_event::Event,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<()> {
    let key: Key = match event {
        crossterm::event::Event::Key(event) => event.into(),
        _ => {
            return Ok(());
        }
    };

    let mut ui = state.ui.lock().unwrap();

    let mut key_sequence = ui.input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let command = state
        .keymap_config
        .find_command_from_key_sequence(&key_sequence);

    let handled = match command {
        None => {
            if let state::PopupState::ContextSearch(_) = ui.popup_state {
                handle_key_sequence_for_context_search_popup(&key_sequence, send, state, &mut ui)?
            } else {
                false
            }
        }
        Some(command) => {
            let handled = match ui.popup_state {
                state::PopupState::None => {
                    handle_command_for_none_popup(command, send, state, &mut ui)?
                }
                state::PopupState::ContextSearch(_) => {
                    handle_key_sequence_for_context_search_popup(
                        &key_sequence,
                        send,
                        state,
                        &mut ui,
                    )?
                }
                state::PopupState::PlaylistSwitch => {
                    handle_command_for_playlist_switch_popup(command, send, state, &mut ui)?
                }
                state::PopupState::ThemeSwitch(_) => {
                    handle_command_for_theme_switch_popup(command, state, &mut ui)?
                }
                state::PopupState::DeviceSwitch => {
                    handle_command_for_device_switch_popup(command, send, state, &mut ui)?
                }
                state::PopupState::CommandHelp => {
                    handle_command_for_command_help_popup(command, &mut ui)?
                }
            };
            if handled {
                true
            } else {
                handle_command(command, send, &mut ui)?
            }
        }
    };

    // if no command is handled, open the shortcuts help based on the current key sequence input
    if handled {
        ui.shortcuts_help_ui_state = false;
        ui.input_key_sequence.keys = vec![];
    } else {
        ui.shortcuts_help_ui_state = true;
        ui.input_key_sequence = key_sequence;
    }
    Ok(())
}

fn handle_command_for_none_popup(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    match command {
        Command::SearchContextTracks => {
            ui.context_tracks_table_ui_state.select(Some(0));
            ui.popup_state = state::PopupState::ContextSearch(state::ContextSearchState {
                query: "/".to_owned(),
                tracks: state
                    .player
                    .read()
                    .unwrap()
                    .get_context_tracks()
                    .into_iter()
                    .cloned()
                    .collect(),
            });
            Ok(true)
        }
        Command::SortByTrack => {
            state
                .player
                .write()
                .unwrap()
                .sort_context_tracks(state::ContextSortOrder::TrackName);
            Ok(true)
        }
        Command::SortByAlbum => {
            state
                .player
                .write()
                .unwrap()
                .sort_context_tracks(state::ContextSortOrder::Album);
            Ok(true)
        }
        Command::SortByArtists => {
            state
                .player
                .write()
                .unwrap()
                .sort_context_tracks(state::ContextSortOrder::Artists);
            Ok(true)
        }
        Command::SortByAddedDate => {
            state
                .player
                .write()
                .unwrap()
                .sort_context_tracks(state::ContextSortOrder::AddedAt);
            Ok(true)
        }
        Command::SortByDuration => {
            state
                .player
                .write()
                .unwrap()
                .sort_context_tracks(state::ContextSortOrder::Duration);
            Ok(true)
        }
        Command::ReverseOrder => {
            state.player.write().unwrap().reverse_context_tracks();
            Ok(true)
        }
        Command::SwitchPlaylist => {
            ui.popup_state = state::PopupState::PlaylistSwitch;
            ui.playlists_list_ui_state = ListState::default();
            ui.playlists_list_ui_state.select(Some(0));
            Ok(true)
        }
        Command::SwitchDevice => {
            ui.popup_state = state::PopupState::DeviceSwitch;
            ui.devices_list_ui_state = ListState::default();
            ui.devices_list_ui_state.select(Some(0));
            send.send(Event::GetDevices)?;
            Ok(true)
        }
        Command::SwitchTheme => {
            let theme = ui.theme.clone();
            ui.popup_state = state::PopupState::ThemeSwitch(theme);
            ui.themes_list_ui_state = ListState::default();
            ui.themes_list_ui_state.select(Some(0));
            Ok(true)
        }
        _ => handle_generic_command_for_context_track_table(command, send, ui, state),
    }
}

fn handle_key_sequence_for_context_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    if key_sequence.keys.len() == 1 {
        let player = state.player.read().unwrap();
        if let Key::None(c) = key_sequence.keys[0] {
            let search_state = match ui.popup_state {
                state::PopupState::ContextSearch(ref mut state) => state,
                _ => unreachable!(),
            };
            match c {
                KeyCode::Char(c) => {
                    search_state.query.push(c);
                    ui.search_context_tracks(&player);
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    if search_state.query.len() > 1 {
                        search_state.query.pop().unwrap();
                        ui.search_context_tracks(&player);
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
    }

    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::ClosePopup => {
                ui.context_tracks_table_ui_state.select(Some(0));
                ui.popup_state = state::PopupState::None;
                Ok(true)
            }
            _ => handle_generic_command_for_context_track_table(command, send, ui, state),
        },
        None => Ok(false),
    }
}

fn handle_command_for_playlist_switch_popup(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    let player = state.player.read().unwrap();

    match command {
        Command::SelectNext => {
            if let Some(id) = ui.playlists_list_ui_state.selected() {
                if id + 1 < player.user_playlists.len() {
                    ui.playlists_list_ui_state.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.playlists_list_ui_state.selected() {
                if id > 0 {
                    ui.playlists_list_ui_state.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChoseSelected => {
            if let Some(id) = ui.playlists_list_ui_state.selected() {
                send.send(Event::PlayContext(player.user_playlists[id].uri.clone()))?;
            }
            Ok(true)
        }
        Command::ClosePopup => {
            ui.popup_state = state::PopupState::None;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_theme_switch_popup(
    command: Command,
    state: &state::SharedState,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    match command {
        Command::SelectNext => {
            if let Some(id) = ui.themes_list_ui_state.selected() {
                if id + 1 < state.theme_config.themes.len() {
                    ui.theme = state.theme_config.themes[id + 1].clone();
                    ui.themes_list_ui_state.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.themes_list_ui_state.selected() {
                if id > 0 {
                    ui.theme = state.theme_config.themes[id - 1].clone();
                    ui.themes_list_ui_state.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChoseSelected => {
            ui.popup_state = state::PopupState::None;
            Ok(true)
        }
        Command::ClosePopup => {
            ui.theme = match ui.popup_state {
                state::PopupState::ThemeSwitch(ref theme) => theme.clone(),
                _ => unreachable!(),
            };
            ui.popup_state = state::PopupState::None;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_device_switch_popup(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    let player = state.player.read().unwrap();

    match command {
        Command::SelectNext => {
            if let Some(id) = ui.devices_list_ui_state.selected() {
                if id + 1 < player.devices.len() {
                    ui.devices_list_ui_state.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.devices_list_ui_state.selected() {
                if id > 0 {
                    ui.devices_list_ui_state.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChoseSelected => {
            if let Some(id) = ui.devices_list_ui_state.selected() {
                send.send(Event::TransferPlayback(player.devices[id].id.clone()))?;
            }
            Ok(true)
        }
        Command::ClosePopup => {
            ui.popup_state = state::PopupState::None;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_command_for_command_help_popup(
    command: Command,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    if let Command::ClosePopup = command {
        ui.popup_state = state::PopupState::None;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_command(
    command: Command,
    send: &mpsc::Sender<Event>,
    ui: &mut state::UIStateGuard,
) -> Result<bool> {
    match command {
        Command::Quit => {
            ui.is_running = false;
            Ok(true)
        }
        Command::NextTrack => {
            send.send(Event::NextTrack)?;
            Ok(true)
        }
        Command::PreviousTrack => {
            send.send(Event::PreviousTrack)?;
            Ok(true)
        }
        Command::ResumePause => {
            send.send(Event::ResumePause)?;
            Ok(true)
        }
        Command::Repeat => {
            send.send(Event::Repeat)?;
            Ok(true)
        }
        Command::Shuffle => {
            send.send(Event::Shuffle)?;
            Ok(true)
        }
        Command::OpenCommandHelp => {
            ui.popup_state = state::PopupState::CommandHelp;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_generic_command_for_context_track_table(
    command: Command,
    send: &mpsc::Sender<Event>,
    ui: &mut state::UIStateGuard,
    state: &state::SharedState,
) -> Result<bool> {
    let player = state.player.read().unwrap();
    let tracks = ui.get_context_tracks(&player);

    match command {
        Command::SelectNext => {
            if let Some(id) = ui.context_tracks_table_ui_state.selected() {
                if id + 1 < tracks.len() {
                    ui.context_tracks_table_ui_state.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = ui.context_tracks_table_ui_state.selected() {
                if id > 0 {
                    ui.context_tracks_table_ui_state.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChoseSelected => {
            if let (Some(id), Some(playback)) = (
                ui.context_tracks_table_ui_state.selected(),
                player.playback.as_ref(),
            ) {
                if let Some(ref context) = playback.context {
                    send.send(Event::PlayTrack(
                        tracks[id].uri.clone(),
                        context.uri.clone(),
                    ))?;
                }
            }
            Ok(true)
        }
        Command::PlaySelectedTrackAlbum => {
            if let Some(id) = ui.context_tracks_table_ui_state.selected() {
                if id < tracks.len() {
                    if let Some(uri) = tracks[id].album.uri.clone() {
                        send.send(Event::PlayContext(uri))?;
                    }
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
