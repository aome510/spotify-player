use crate::prelude::*;
use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use tokio::stream::StreamExt;

#[derive(Debug)]
pub enum Event {
    Quit,
    RefreshToken,
    NextSong,
    PreviousSong,
    ResumePause,
    Repeat,
    Shuffle,
    GetPlaylist(String),
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

fn handle_event(event: term_event::Event, send: &mpsc::Sender<Event>) -> Result<()> {
    if let term_event::Event::Key(key_event) = event {
        match key_event.into() {
            KeyEvent::Ctrl(KeyCode::Char('c')) => {
                send.send(Event::Quit)?;
            }
            KeyEvent::None(KeyCode::Char('n')) => {
                send.send(Event::NextSong)?;
            }
            KeyEvent::None(KeyCode::Char('p')) => {
                send.send(Event::PreviousSong)?;
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
            _ => {}
        }
    };

    Ok(())
}

#[tokio::main]
/// actively pools events from the terminal using `crossterm::event::EventStream`
pub async fn start_event_stream(send: mpsc::Sender<Event>) {
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                log::debug!("got event: {:?}", event);
                if let Err(err) = handle_event(event, &send) {
                    log::error!("failed to handle event: {:#}", err);
                }
            }
            Err(err) => {
                log::error!("failed to get event: {:#}", err);
            }
        }
    }
}
