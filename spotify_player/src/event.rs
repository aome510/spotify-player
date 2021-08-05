use crate::{
    command::Command,
    key::{Key, KeySequence},
    state,
};
use anyhow::Result;
use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use std::sync::{mpsc, MutexGuard};
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

    let mut state = state.lock().unwrap();

    let mut key_sequence = state.input_key_sequence.clone();
    key_sequence.keys.push(key.clone());
    if state
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let mut handled = match state.popup_state {
        state::PopupState::None => {
            handle_key_sequence_for_none_popup(&key_sequence, send, &mut state)?
        }
        state::PopupState::ContextSearch(_) => {
            handle_key_sequence_for_context_search_popup(&key_sequence, send, &mut state)?
        }
        state::PopupState::PlaylistSwitch => {
            handle_key_sequence_for_playlist_switch_popup(&key_sequence, send, &mut state)?
        }
        state::PopupState::ThemeSwitch(_) => {
            handle_key_sequence_for_theme_switch_popup(&key_sequence, &mut state)?
        }
        state::PopupState::DeviceSwitch => {
            handle_key_sequence_for_device_switch_popup(&key_sequence, send, &mut state)?
        }
        state::PopupState::CommandHelp => {
            handle_key_sequence_for_command_help_popup(&key_sequence, &mut state)?
        }
    };
    if !handled {
        handled = handle_key_sequence(&key_sequence, send, &mut state)?;
    }

    // if no command is handled, open the shortcuts help based on the current key sequence input
    if handled {
        state.shortcuts_help_ui_state = false;
        state.input_key_sequence.keys = vec![];
    } else {
        state.shortcuts_help_ui_state = true;
        state.input_key_sequence = key_sequence;
    }
    Ok(())
}

fn handle_key_sequence_for_none_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::SearchContextTracks => {
                state.context_tracks_table_ui_state.select(Some(0));
                state.popup_state = state::PopupState::ContextSearch(state::ContextSearchState {
                    query: "/".to_owned(),
                    tracks: state.get_context_tracks().into_iter().cloned().collect(),
                });
                Ok(true)
            }
            Command::SortByTrack => {
                state.sort_context_tracks(state::ContextSortOrder::TrackName);
                Ok(true)
            }
            Command::SortByAlbum => {
                state.sort_context_tracks(state::ContextSortOrder::Album);
                Ok(true)
            }
            Command::SortByArtists => {
                state.sort_context_tracks(state::ContextSortOrder::Artists);
                Ok(true)
            }
            Command::SortByAddedDate => {
                state.sort_context_tracks(state::ContextSortOrder::AddedAt);
                Ok(true)
            }
            Command::SortByDuration => {
                state.sort_context_tracks(state::ContextSortOrder::Duration);
                Ok(true)
            }
            Command::ReverseOrder => {
                state.reverse_context_tracks();
                Ok(true)
            }
            Command::SwitchPlaylist => {
                state.popup_state = state::PopupState::PlaylistSwitch;
                state.playlists_list_ui_state = ListState::default();
                state.playlists_list_ui_state.select(Some(0));
                Ok(true)
            }
            Command::SwitchDevice => {
                state.popup_state = state::PopupState::DeviceSwitch;
                state.devices_list_ui_state = ListState::default();
                state.devices_list_ui_state.select(Some(0));
                send.send(Event::GetDevices)?;
                Ok(true)
            }
            Command::SwitchTheme => {
                let theme = state.theme_config.theme.clone();
                state.popup_state = state::PopupState::ThemeSwitch(theme);
                state.themes_list_ui_state = ListState::default();
                state.themes_list_ui_state.select(Some(0));
                Ok(true)
            }
            _ => handle_generic_command_for_context_track_table(command, send, state),
        },
        _ => Ok(false),
    }
}

fn handle_key_sequence_for_context_search_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    if key_sequence.keys.len() == 1 {
        if let Key::None(c) = key_sequence.keys[0] {
            let search_state = match state.popup_state {
                state::PopupState::ContextSearch(ref mut state) => state,
                _ => unreachable!(),
            };
            match c {
                KeyCode::Char(c) => {
                    search_state.query.push(c);
                    state.search_context_tracks();
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    if search_state.query.len() > 1 {
                        search_state.query.pop().unwrap();
                        state.search_context_tracks();
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
                state.context_tracks_table_ui_state.select(Some(0));
                state.popup_state = state::PopupState::None;
                Ok(true)
            }
            _ => handle_generic_command_for_context_track_table(command, send, state),
        },
        None => Ok(false),
    }
}

fn handle_key_sequence_for_playlist_switch_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::SelectNext => {
                if let Some(id) = state.playlists_list_ui_state.selected() {
                    if id + 1 < state.user_playlists.len() {
                        state.playlists_list_ui_state.select(Some(id + 1));
                    }
                }
                Ok(true)
            }
            Command::SelectPrevious => {
                if let Some(id) = state.playlists_list_ui_state.selected() {
                    if id > 0 {
                        state.playlists_list_ui_state.select(Some(id - 1));
                    }
                }
                Ok(true)
            }
            Command::ChoseSelected => {
                if let Some(id) = state.playlists_list_ui_state.selected() {
                    send.send(Event::PlayContext(state.user_playlists[id].uri.clone()))?;
                }
                Ok(true)
            }
            Command::ClosePopup => {
                state.popup_state = state::PopupState::None;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_key_sequence_for_theme_switch_popup(
    key_sequence: &KeySequence,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::SelectNext => {
                if let Some(id) = state.themes_list_ui_state.selected() {
                    if id + 1 < state.theme_config.themes.len() {
                        state.theme_config.theme = state.theme_config.themes[id + 1].clone();
                        state.themes_list_ui_state.select(Some(id + 1));
                    }
                }
                Ok(true)
            }
            Command::SelectPrevious => {
                if let Some(id) = state.themes_list_ui_state.selected() {
                    if id > 0 {
                        state.theme_config.theme = state.theme_config.themes[id - 1].clone();
                        state.themes_list_ui_state.select(Some(id - 1));
                    }
                }
                Ok(true)
            }
            Command::ChoseSelected => {
                if let Some(id) = state.themes_list_ui_state.selected() {
                    // update the application's theme to the chosen theme, then
                    // move the chosen theme to the beginning of the theme list
                    let theme = state.theme_config.themes.remove(id);
                    state.theme_config.theme = theme.clone();
                    state.theme_config.themes.insert(0, theme);
                }
                state.popup_state = state::PopupState::None;
                Ok(true)
            }
            Command::ClosePopup => {
                state.theme_config.theme = match state.popup_state {
                    state::PopupState::ThemeSwitch(ref theme) => theme.clone(),
                    _ => unreachable!(),
                };
                state.popup_state = state::PopupState::None;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_key_sequence_for_device_switch_popup(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::SelectNext => {
                if let Some(id) = state.devices_list_ui_state.selected() {
                    if id + 1 < state.devices.len() {
                        state.devices_list_ui_state.select(Some(id + 1));
                    }
                }
                Ok(true)
            }
            Command::SelectPrevious => {
                if let Some(id) = state.devices_list_ui_state.selected() {
                    if id > 0 {
                        state.devices_list_ui_state.select(Some(id - 1));
                    }
                }
                Ok(true)
            }
            Command::ChoseSelected => {
                if let Some(id) = state.devices_list_ui_state.selected() {
                    send.send(Event::TransferPlayback(state.devices[id].id.clone()))?;
                }
                Ok(true)
            }
            Command::ClosePopup => {
                state.popup_state = state::PopupState::None;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_key_sequence_for_command_help_popup(
    key_sequence: &KeySequence,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    if let Some(Command::ClosePopup) = command {
        state.popup_state = state::PopupState::None;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_key_sequence(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    let command = state
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::Quit => {
                state.is_running = false;
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
                state.popup_state = state::PopupState::CommandHelp;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_generic_command_for_context_track_table(
    command: Command,
    send: &mpsc::Sender<Event>,
    state: &mut MutexGuard<state::State>,
) -> Result<bool> {
    match command {
        Command::SelectNext => {
            if let Some(id) = state.context_tracks_table_ui_state.selected() {
                if id + 1 < state.get_context_tracks().len() {
                    state.context_tracks_table_ui_state.select(Some(id + 1));
                }
            }
            Ok(true)
        }
        Command::SelectPrevious => {
            if let Some(id) = state.context_tracks_table_ui_state.selected() {
                if id > 0 {
                    state.context_tracks_table_ui_state.select(Some(id - 1));
                }
            }
            Ok(true)
        }
        Command::ChoseSelected => {
            if let (Some(id), Some(playback)) = (
                state.context_tracks_table_ui_state.selected(),
                state.playback.as_ref(),
            ) {
                if let Some(ref context) = playback.context {
                    let tracks = state.get_context_tracks();
                    send.send(Event::PlayTrack(
                        tracks[id].uri.clone(),
                        context.uri.clone(),
                    ))?;
                }
            }
            Ok(true)
        }
        Command::PlaySelectedTrackAlbum => {
            if let Some(id) = state.context_tracks_table_ui_state.selected() {
                let tracks = state.get_context_tracks();
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
