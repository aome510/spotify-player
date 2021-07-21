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
                state.write().unwrap().context_search_state.query = None;
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

fn handle_event(
    event: term_event::Event,
    send: &mpsc::Sender<Event>,
    state: &state::SharedState,
) -> Result<()> {
    if state.read().unwrap().context_search_state.query.is_some() {
        handle_search_mode_event(event, send, state)
    } else {
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
                KeyEvent::None(KeyCode::Char('r')) => {
                    send.send(Event::Repeat)?;
                }
                KeyEvent::None(KeyCode::Char('s')) => {
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
                    state.context_search_state = state::ContextSearchState {
                        query: Some("/".to_owned()),
                        tracks: state
                            .get_context_filtered_tracks()
                            .into_iter()
                            .cloned()
                            .collect(),
                    };
                }
                _ => {}
            }
        };

        Ok(())
    }
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
                    log::error!("failed to handle event: {:#}", err);
                }
            }
            Err(err) => {
                log::error!("failed to get event: {:#}", err);
            }
        }
    }
}
