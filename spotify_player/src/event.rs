use std::sync::mpsc;

use crate::event;
use crossterm::event::{Event as CrEvent, EventStream, KeyCode, KeyEvent, KeyModifiers};
use tokio::stream::StreamExt;

pub enum Event {
    RefreshToken,
    GetCurrentPlayingContext,
    NextSong,
    PreviousSong,
    TogglePlayingState,
}

fn handle_event(event: CrEvent, send: &mpsc::Sender<event::Event>) {
    match event {
        CrEvent::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
        }) => {
            std::process::exit(0);
        }
        CrEvent::Key(KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::NONE,
        }) => {
            send.send(Event::NextSong).unwrap();
        }
        CrEvent::Key(KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::NONE,
        }) => {
            send.send(Event::PreviousSong).unwrap();
        }
        CrEvent::Key(KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
        }) => {
            send.send(Event::TogglePlayingState).unwrap();
        }
        _ => {}
    };
}

#[tokio::main]
pub async fn poll_events(send: mpsc::Sender<event::Event>) {
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
