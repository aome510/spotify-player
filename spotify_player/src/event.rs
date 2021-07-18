use std::sync::mpsc;

use crossterm::event::{self as term_event, EventStream, KeyCode, KeyModifiers};
use tokio::stream::StreamExt;

pub enum Event {
    RefreshToken,
    GetCurrentPlayingContext,
    NextSong,
    PreviousSong,
    TogglePlayingState,
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

fn handle_event(event: term_event::Event, send: &mpsc::Sender<Event>) {
    if let term_event::Event::Key(key_event) = event {
        match key_event.into() {
            KeyEvent::Ctrl(KeyCode::Char('c')) => {
                std::process::exit(0);
            }
            KeyEvent::None(KeyCode::Char('n')) => {
                send.send(Event::NextSong).unwrap();
            }
            KeyEvent::None(KeyCode::Char('p')) => {
                send.send(Event::PreviousSong).unwrap();
            }
            KeyEvent::None(KeyCode::Char(' ')) => {
                send.send(Event::TogglePlayingState).unwrap();
            }
            _ => {}
        }
    };
}

#[tokio::main]
/// actively pools events from the terminal using `crossterm::event::EventStream`
pub async fn poll_events(send: mpsc::Sender<Event>) {
    println!("start pooling events...");
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                println!("Event::{:?}", event);
                handle_event(event, &send);
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
            }
        }
    }
}
