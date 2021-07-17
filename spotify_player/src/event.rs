use crossterm::event::*;
use tokio::stream::StreamExt;

use crate::state::SharedState;

#[tokio::main]
pub async fn poll_events(state: SharedState) {
    println!("start pooling events...");
    let mut event_stream = EventStream::new();

    while let Some(event) = event_stream.next().await {
        match event {
            Ok(event) => {
                println!("Event::{:?}", event);
                if event
                    == Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                    })
                {
                    state.write().unwrap().is_running = false;
                    break;
                }
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
            }
        }
    }
}
