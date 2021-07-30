use crate::{
    config::{Command, Key},
    prelude::*,
    state,
};
use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use tokio::stream::StreamExt;

#[derive(Debug)]
/// Event to communicate with the client
pub enum Event {
    Quit,
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
    // SortContextTracks(state::ContextSortOrder),
}

impl From<term_event::KeyEvent> for Key {
    fn from(event: term_event::KeyEvent) -> Self {
        match event.modifiers {
            KeyModifiers::NONE => Key::None(event.code),
            KeyModifiers::ALT => Key::Alt(event.code),
            KeyModifiers::CONTROL => Key::Ctrl(event.code),
            KeyModifiers::SHIFT => Key::None(event.code),
            _ => Key::Unknown,
        }
    }
}

fn handle_search_mode_key(
    key: &Key,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<bool> {
    let command = match *key {
        Key::None(c) => match c {
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
            _ => state
                .read()
                .unwrap()
                .keymap_config
                .get_command_from_key(key),
        },
        _ => state
            .read()
            .unwrap()
            .keymap_config
            .get_command_from_key(key),
    };

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
            Command::ToDefaultMode => {
                let mut state = state.write().unwrap();
                state.context_search_state.query = None;
                state.context_tracks_table_ui_state.select(Some(0));
                state.current_event_state = state::EventState::Default;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

// fn handle_sort_mode_key(
//     key: &Key,
//     send: &mpsc::Sender<Event>,
//     state: &state::SharedState,
// ) -> Result<bool> {
//     Ok(false)

//     // TODO: handle sort

//     // if let term_event::Event::Key(key_event) = event {
//     //     match key_event.into() {
//     //         Key::None(KeyCode::Char('q')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::TrackName(true),
//     //         ))?,
//     //         Key::None(KeyCode::Char('Q')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::TrackName(false),
//     //         ))?,
//     //         Key::None(KeyCode::Char('w')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::Album(true),
//     //         ))?,
//     //         Key::None(KeyCode::Char('W')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::Album(false),
//     //         ))?,
//     //         Key::None(KeyCode::Char('e')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::Artists(true),
//     //         ))?,
//     //         Key::None(KeyCode::Char('E')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::Artists(false),
//     //         ))?,
//     //         Key::None(KeyCode::Char('r')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::AddedAt(true),
//     //         ))?,
//     //         Key::None(KeyCode::Char('R')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::AddedAt(false),
//     //         ))?,
//     //         Key::None(KeyCode::Char('t')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::Duration(true),
//     //         ))?,
//     //         Key::None(KeyCode::Char('T')) => send.send(Event::SortContextTracks(
//     //             state::ContextSortOrder::Duration(false),
//     //         ))?,
//     //         _ => {}
//     //     }
//     // }
//     // state.write().unwrap().current_event_state = state::EventState::Default;
//     // Ok(())
// }

fn handle_playlist_switch_mode_key(
    key: &Key,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<bool> {
    let command = state
        .read()
        .unwrap()
        .keymap_config
        .get_command_from_key(key);
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
            Command::ToDefaultMode => {
                state.write().unwrap().current_event_state = state::EventState::Default;
                Ok(true)
            }
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

fn handle_default_mode_key(
    key: &Key,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<bool> {
    let command = state
        .read()
        .unwrap()
        .keymap_config
        .get_command_from_key(key);
    match command {
        Some(command) => match command {
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
                state.current_event_state = state::EventState::ContextSearch;
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
            // Command::SortContextTracks => {
            //     state.write().unwrap().current_event_state = state::EventState::Sort;
            //     Ok(true)
            // }
            Command::SwitchPlaylists => {
                state.write().unwrap().current_event_state = state::EventState::PlaylistSwitch;
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
        _ => Key::Unknown,
    };

    let current_event_state = state.read().unwrap().current_event_state.clone();
    let handled = match current_event_state {
        state::EventState::Default => handle_default_mode_key(&key, send, state)?,
        state::EventState::ContextSearch => handle_search_mode_key(&key, send, state)?,
        // TODO: handle sort mode after figuring out how to
        // implement keymaps by mode
        // state::EventState::Sort => handle_sort_mode_key(&key, send, state)?,
        state::EventState::Sort => false,
        state::EventState::PlaylistSwitch => handle_playlist_switch_mode_key(&key, send, state)?,
    };

    // global command handler
    if !handled {
        let command = state
            .read()
            .unwrap()
            .keymap_config
            .get_command_from_key(&key);
        if let Some(Command::Quit) = command {
            send.send(Event::Quit)?;
        }
    }
    Ok(())
}

#[tokio::main]
/// actively pools events from the terminal using `crossterm::event::EventStream`
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
