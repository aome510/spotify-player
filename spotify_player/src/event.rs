use crate::{
    config::Command,
    key::{Key, KeySequence},
    state,
};
use anyhow::Result;
use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use std::sync::mpsc;
use tokio::stream::StreamExt;

#[derive(Debug)]
/// An event to communicate with the client
pub enum Event {
    RefreshToken,
    NextTrack,
    PreviousTrack,
    ResumePause,
    Repeat,
    Shuffle,
    GetPlaylist(String),
    GetAlbum(String),
    PlaySelectedTrack,
    PlaySelectedPlaylist,
    SearchTrackInContext,
    SortContextTracks(state::ContextSortOrder),
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

fn handle_search_mode_event(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<bool> {
    if key_sequence.keys.len() == 1 {
        if let Key::None(c) = key_sequence.keys[0] {
            match c {
                KeyCode::Char(c) => {
                    let mut state = state.write().unwrap();
                    state.context_search_state.query.as_mut().unwrap().push(c);
                    send.send(Event::SearchTrackInContext)?;
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    let mut state = state.write().unwrap();
                    if let Some(query) = state.context_search_state.query.as_mut() {
                        if query.len() > 1 {
                            query.pop().unwrap();
                            send.send(Event::SearchTrackInContext)?;
                        }
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
    }

    let command = state
        .read()
        .unwrap()
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::SelectNext => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.context_tracks_table_ui_state.selected() {
                    if id + 1 < state.get_context_filtered_tracks().len() {
                        state.context_tracks_table_ui_state.select(Some(id + 1));
                    }
                }
                Ok(true)
            }
            Command::SelectPrevious => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.context_tracks_table_ui_state.selected() {
                    if id > 0 {
                        state.context_tracks_table_ui_state.select(Some(id - 1));
                    }
                }
                Ok(true)
            }
            Command::PlaySelected => {
                send.send(Event::PlaySelectedTrack)?;
                Ok(true)
            }
            Command::ClosePopup => {
                let mut state = state.write().unwrap();
                state.context_search_state.query = None;
                state.context_tracks_table_ui_state.select(Some(0));
                state.current_event_state = state::PopupBufferState::None;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_playlist_switch_mode_event(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<bool> {
    let command = state
        .read()
        .unwrap()
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::SelectNext => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.playlists_list_ui_state.selected() {
                    if id + 1 < state.current_playlists.len() {
                        state.playlists_list_ui_state.select(Some(id + 1));
                    }
                }
                Ok(true)
            }
            Command::SelectPrevious => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.playlists_list_ui_state.selected() {
                    if id > 0 {
                        state.playlists_list_ui_state.select(Some(id - 1));
                    }
                }
                Ok(true)
            }
            Command::PlaySelected => {
                send.send(Event::PlaySelectedPlaylist)?;
                Ok(true)
            }
            Command::ClosePopup => {
                state.write().unwrap().current_event_state = state::PopupBufferState::None;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_global_mode_event(
    key_sequence: &KeySequence,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<bool> {
    let command = state
        .read()
        .unwrap()
        .keymap_config
        .find_command_from_key_sequence(key_sequence);

    match command {
        Some(command) => match command {
            Command::Quit => {
                state.write().unwrap().is_running = false;
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
            Command::SelectNext => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.context_tracks_table_ui_state.selected() {
                    if id + 1 < state.get_context_filtered_tracks().len() {
                        state.context_tracks_table_ui_state.select(Some(id + 1));
                    }
                }
                Ok(true)
            }
            Command::SelectPrevious => {
                let mut state = state.write().unwrap();
                if let Some(id) = state.context_tracks_table_ui_state.selected() {
                    if id > 0 {
                        state.context_tracks_table_ui_state.select(Some(id - 1));
                    }
                }
                Ok(true)
            }
            Command::PlaySelected => {
                send.send(Event::PlaySelectedTrack)?;
                Ok(true)
            }
            Command::SearchContextTracks => {
                let mut state = state.write().unwrap();
                state.context_tracks_table_ui_state.select(Some(0));
                state.current_event_state = state::PopupBufferState::ContextSearch;
                state.context_search_state = state::ContextSearchState {
                    query: Some("/".to_owned()),
                    tracks: state
                        .get_context_filtered_tracks()
                        .into_iter()
                        .cloned()
                        .collect(),
                };
                Ok(true)
            }
            Command::SortByTrack => {
                send.send(Event::SortContextTracks(state::ContextSortOrder::TrackName))?;
                Ok(true)
            }
            Command::SortByAlbum => {
                send.send(Event::SortContextTracks(state::ContextSortOrder::Album))?;
                Ok(true)
            }
            Command::SortByArtists => {
                send.send(Event::SortContextTracks(state::ContextSortOrder::Artists))?;
                Ok(true)
            }
            Command::SortByAddedDate => {
                send.send(Event::SortContextTracks(state::ContextSortOrder::AddedAt))?;
                Ok(true)
            }
            Command::SortByDuration => {
                send.send(Event::SortContextTracks(state::ContextSortOrder::Duration))?;
                Ok(true)
            }
            Command::ReverseOrder => {
                state.write().unwrap().current_context_tracks.reverse();
                Ok(true)
            }
            Command::SwitchPlaylists => {
                state.write().unwrap().current_event_state =
                    state::PopupBufferState::PlaylistSwitch;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_event(
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

    let mut key_sequence = state.read().unwrap().current_key_prefix.clone();
    key_sequence.keys.push(key.clone());
    if state
        .read()
        .unwrap()
        .keymap_config
        .find_matched_prefix_keymaps(&key_sequence)
        .is_empty()
    {
        key_sequence = KeySequence { keys: vec![key] };
    }

    let current_event_state = state.read().unwrap().current_event_state.clone();
    let mut handled = match current_event_state {
        state::PopupBufferState::None => false,
        state::PopupBufferState::ContextSearch => {
            handle_search_mode_event(&key_sequence, send, state)?
        }
        state::PopupBufferState::PlaylistSwitch => {
            handle_playlist_switch_mode_event(&key_sequence, send, state)?
        }
    };
    if !handled {
        handled = handle_global_mode_event(&key_sequence, send, state)?;
    }
    if handled {
        state.write().unwrap().shortcuts_help_ui_state = false;
        state.write().unwrap().current_key_prefix.keys = vec![];
    } else {
        state.write().unwrap().shortcuts_help_ui_state = true;
        state.write().unwrap().current_key_prefix = key_sequence;
    }
    Ok(())
}

#[tokio::main]
/// starts the application's event stream that actively pools events from the terminal
pub async fn start_event_stream(send: mpsc::Sender<Event>, state: state::SharedState) {
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                log::info!("got event: {:?}", event);
                if let Err(err) = handle_event(event, &send, &state) {
                    log::warn!("failed to handle event: {:#}", err);
                }
            }
            Err(err) => {
                log::warn!("failed to get event: {:#}", err);
            }
        }
    }
}
