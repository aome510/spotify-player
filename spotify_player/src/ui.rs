use crate::config;
use crate::event;
use crate::prelude::*;
use crate::state;
use std::io::Stdout;
use tui::backend::CrosstermBackend;

type Terminal = tui::Terminal<CrosstermBackend<Stdout>>;
type Frame<'a> = tui::Frame<'a, CrosstermBackend<Stdout>>;

fn render_current_playback_widget(
    frame: &mut Frame,
    context: &context::CurrentlyPlaybackContext,
    rect: Rect,
) {
    if let Some(PlayingItem::Track(track)) = context.item.as_ref() {
        let progress_in_sec: u32 = context.progress_ms.unwrap() / 1000;
        let playback_info = format!(
            "currently playing {} at {}/{} (repeat: {}, shuffle: {})\n",
            track.name,
            progress_in_sec,
            track.duration_ms / 1000,
            context.repeat_state.as_str(),
            context.shuffle_state,
        );

        let desc_block = Paragraph::new(playback_info)
            .block(
                Block::default()
                    .title("Current playback context")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(desc_block, rect);
    }
}

fn render_playlist_tracks_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let items = match state.read().unwrap().current_playlist_tracks.as_ref() {
        Some(tracks) => tracks
            .iter()
            .filter(|t| t.track.is_some())
            .map(|t| ListItem::new(t.track.as_ref().unwrap().name.clone()))
            .collect::<Vec<_>>(),
        None => vec![],
    };
    let mut state = state.write().unwrap();
    if !items.is_empty() {
        state.ui_playlist_tracks_list_state.select(Some(0));
    }
    let tracks_block = List::new(items)
        .block(
            Block::default()
                .title("Playlist tracks")
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");
    frame.render_stateful_widget(tracks_block, rect, &mut state.ui_playlist_tracks_list_state);
    log::info!("list state: {:?}", state.ui_playlist_tracks_list_state);
}

fn quit(mut terminal: Terminal) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// start the application UI as the main thread
pub fn start_ui(state: state::SharedState, send: mpsc::Sender<event::Event>) -> Result<()> {
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref());

    terminal.draw(|f| {
        let ui = Paragraph::new("Loading the application... Please check your internet connection if this takes too long <(\").")
            .block(
                Block::default()
                    .title("Loading...")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(ui, f.size())
    })?;

    loop {
        {
            // check application's state to emit events if necessary
            let state = state.read().unwrap();
            if !state.is_running {
                // a `Quit` event is sent, clean up the application then exit
                quit(terminal)?;
                return Ok(());
            }
            if std::time::SystemTime::now() > state.auth_token_expires_at {
                send.send(event::Event::RefreshToken)?;
            }

            // check if state's current playlist matches the playlist inside the current playback,
            // if not request a new playlist.
            let current_playback_context = state.current_playback_context.as_ref();
            let current_playlist = state.current_playlist.as_ref();
            if let Some(playback) = current_playback_context {
                if let Some(context) = playback.context.as_ref() {
                    if let rspotify::senum::Type::Playlist = context._type {
                        let playlist_id = context.uri.split(':').nth(2).unwrap();
                        let current_playlist_id = match current_playlist {
                            Some(playlist) => &playlist.id,
                            None => "",
                        };
                        if current_playlist_id != playlist_id {
                            send.send(event::Event::GetPlaylist(playlist_id.to_owned()))?;
                        }
                    }
                }
            };
        }

        {
            // draw ui
            terminal.draw(|f| {
                let chunks = layout.clone().split(f.size());
                if let Some(context) = state.read().unwrap().current_playback_context.as_ref() {
                    render_current_playback_widget(f, context, chunks[0]);
                }
                render_playlist_tracks_widget(f, &state, chunks[1]);
            })?;
        }

        std::thread::sleep(config::REFRESH_DURATION);
    }
}
