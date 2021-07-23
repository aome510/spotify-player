use crate::{prelude::*, state};
use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use tokio::stream::StreamExt;

#[derive(Debug)]
pub enum Event {
    Quit,
    RefreshToken,
    NextTrack,
    PreviousTrack,
    ResumePause,
    Repeat,
    Shuffle,
    GetPlaylist(String),
    SelectNextTrack,
    SelectPreviousTrack,
    PlaySelectedTrack,
    SearchTrackInContext,
    SortContextTracks(state::ContextSortOrder),
}

pub enum KeyEvent {
    None(KeyCode),
    Ctrl(KeyCode),
    Alt(KeyCode),
    Unknown,
}

impl From<term_event::KeyEvent> for KeyEvent {
    fn from(event: term_event::KeyEvent) -> Self {
        match event.modifiers {
            KeyModifiers::NONE => KeyEvent::None(event.code),
            KeyModifiers::ALT => KeyEvent::Alt(event.code),
            KeyModifiers::CONTROL => KeyEvent::Ctrl(event.code),
            KeyModifiers::SHIFT => KeyEvent::None(event.code),
            _ => KeyEvent::Unknown,
        }
    }
}

fn handle_search_mode_event(
    event: term_event::Event,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<()> {
    if let term_event::Event::Key(key_event) = event {
        match key_event.into() {
            KeyEvent::None(KeyCode::Esc) => {
                let mut state = state.write().unwrap();
                state.current_event_state = state::EventState::Default;
                state.context_search_state.query = None;
            }
            KeyEvent::None(KeyCode::Char(c)) => {
                let mut state = state.write().unwrap();
                state.context_search_state.query.as_mut().unwrap().push(c);
                send.send(Event::SearchTrackInContext)?;
            }
            KeyEvent::None(KeyCode::Backspace) => {
                let mut state = state.write().unwrap();
                if let Some(query) = state.context_search_state.query.as_mut() {
                    if query.len() > 1 {
                        query.pop().unwrap();
                        send.send(Event::SearchTrackInContext)?;
                    }
                }
            }
            KeyEvent::Ctrl(KeyCode::Char('j')) => {
                send.send(Event::SelectNextTrack)?;
            }
            KeyEvent::Ctrl(KeyCode::Char('k')) => {
                send.send(Event::SelectPreviousTrack)?;
            }
            KeyEvent::None(KeyCode::Enter) => {
                send.send(Event::PlaySelectedTrack)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn handle_sort_mode_event(
    event: term_event::Event,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<()> {
    if let term_event::Event::Key(key_event) = event {
        match key_event.into() {
            KeyEvent::None(KeyCode::Char('q')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::TrackName(true),
            ))?,
            KeyEvent::None(KeyCode::Char('Q')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::TrackName(false),
            ))?,
            KeyEvent::None(KeyCode::Char('w')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::Album(true),
            ))?,
            KeyEvent::None(KeyCode::Char('W')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::Album(false),
            ))?,
            KeyEvent::None(KeyCode::Char('e')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::Artists(true),
            ))?,
            KeyEvent::None(KeyCode::Char('E')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::Artists(false),
            ))?,
            KeyEvent::None(KeyCode::Char('r')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::AddedAt(true),
            ))?,
            KeyEvent::None(KeyCode::Char('R')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::AddedAt(false),
            ))?,
            KeyEvent::None(KeyCode::Char('t')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::Duration(true),
            ))?,
            KeyEvent::None(KeyCode::Char('T')) => send.send(Event::SortContextTracks(
                state::ContextSortOrder::Duration(false),
            ))?,
            _ => {}
        }
    }
    state.write().unwrap().current_event_state = state::EventState::Default;
    Ok(())
}

fn handel_default_mode_event(
    event: term_event::Event,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<()> {
    if let term_event::Event::Key(key_event) = event {
        match key_event.into() {
            KeyEvent::None(KeyCode::Char('q')) => {
                send.send(Event::Quit)?;
            }
            KeyEvent::None(KeyCode::Char('n')) => {
                send.send(Event::NextTrack)?;
            }
            KeyEvent::None(KeyCode::Char('p')) => {
                send.send(Event::PreviousTrack)?;
            }
            KeyEvent::None(KeyCode::Char(' ')) => {
                send.send(Event::ResumePause)?;
            }
            KeyEvent::Ctrl(KeyCode::Char('r')) => {
                send.send(Event::Repeat)?;
            }
            KeyEvent::Ctrl(KeyCode::Char('s')) => {
                send.send(Event::Shuffle)?;
            }
            KeyEvent::None(KeyCode::Char('j')) => {
                send.send(Event::SelectNextTrack)?;
            }
            KeyEvent::None(KeyCode::Char('k')) => {
                send.send(Event::SelectPreviousTrack)?;
            }
            KeyEvent::None(KeyCode::Enter) => {
                send.send(Event::PlaySelectedTrack)?;
            }
            KeyEvent::None(KeyCode::Char('/')) => {
                let mut state = state.write().unwrap();
                state.current_event_state = state::EventState::ContextSearch;
                state.context_search_state = state::ContextSearchState {
                    query: Some("/".to_owned()),
                    tracks: state
                        .get_context_filtered_tracks()
                        .into_iter()
                        .cloned()
                        .collect(),
                };
            }
            KeyEvent::None(KeyCode::Char('s')) => {
                state.write().unwrap().current_event_state = state::EventState::Sort;
            }
            _ => {}
        }
    };

    Ok(())
}

fn handle_event(
    event: term_event::Event,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<()> {
    // handle global commands
    if let term_event::Event::Key(key_event) = event {
        if let KeyEvent::Ctrl(KeyCode::Char('c')) = key_event.into() {
            send.send(Event::Quit)?;
        }
    }
    let current_event_state = state.read().unwrap().current_event_state.clone();
    match current_event_state {
        state::EventState::Default => {
            handel_default_mode_event(event, send, state)?;
        }
        state::EventState::ContextSearch => {
            handle_search_mode_event(event, send, state)?;
        }
        state::EventState::Sort => {
            handle_sort_mode_event(event, send, state)?;
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
