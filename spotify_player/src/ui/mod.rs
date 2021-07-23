use crate::config;
use crate::event;
use crate::prelude::*;
use crate::state;
use std::io::Stdout;
use tui::backend::CrosstermBackend;

type Terminal = tui::Terminal<CrosstermBackend<Stdout>>;
type Frame<'a> = tui::Frame<'a, CrosstermBackend<Stdout>>;

mod help;

fn render_current_playback_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let playback_info = if let Some(ref context) = state.read().unwrap().current_playback_context {
        if let Some(PlayingItem::Track(ref track)) = context.item {
            let progress_in_sec: u32 = context.progress_ms.unwrap() / 1000;
            format!(
                "Current track: {} at {}/{} (playing: {}, repeat: {}, shuffle: {})\n",
                track.name,
                progress_in_sec,
                track.duration_ms / 1000,
                context.is_playing,
                context.repeat_state.as_str(),
                context.shuffle_state,
            )
        } else {
            "".to_owned()
        }
    } else {
        "".to_owned()
    };
    let widget = Paragraph::new(playback_info)
        .block(
            Block::default()
                .title("Playback context")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, rect);
}

fn render_playlist_tracks_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let rows = state
        .read()
        .unwrap()
        .get_context_filtered_tracks()
        .into_iter()
        .map(|t| {
            let desc = state::get_track_description(t);
            Row::new(vec![
                Cell::from(state::truncate_string(
                    desc.name,
                    config::TRACK_DESC_ITEM_MAX_LEN,
                )),
                Cell::from(state::truncate_string(
                    desc.artists.join(","),
                    config::TRACK_DESC_ITEM_MAX_LEN,
                )),
                Cell::from(state::truncate_string(
                    desc.album,
                    config::TRACK_DESC_ITEM_MAX_LEN,
                )),
            ])
        })
        .collect::<Vec<_>>();
    let widget = Table::new(rows)
        .header(
            Row::new(vec![
                Cell::from("Track"),
                Cell::from("Artists"),
                Cell::from("Album"),
            ])
            .style(Style::default().fg(Color::Yellow)),
        )
        .block(
            Block::default()
                .title("Context tracks")
                .borders(Borders::ALL),
        )
        .widths(&[
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");
    frame.render_stateful_widget(
        widget,
        rect,
        &mut state.write().unwrap().ui_context_tracks_table_state,
    );
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

fn render_main_layout(f: &mut Frame, state: &state::SharedState, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(rect);
    render_current_playback_widget(f, &state, chunks[0]);
    render_playlist_tracks_widget(f, &state, chunks[1]);
}

/// start the application UI as the main thread
pub fn start_ui(state: state::SharedState, send: mpsc::Sender<event::Event>) -> Result<()> {
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;

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
            if let Some(ref playback) = state.current_playback_context {
                if let Some(ref context) = playback.context {
                    if let rspotify::senum::Type::Playlist = context._type {
                        let playlist_id = context.uri.split(':').nth(2).unwrap();
                        let current_playlist_id = match state.current_playlist {
                            Some(ref playlist) => &playlist.id,
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
                let main_layout_rect = match state.read().unwrap().context_search_state.query {
                    None => f.size(),
                    Some(ref query) => {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                            .split(f.size());
                        let search_box = Paragraph::new(query.clone())
                            .block(Block::default().borders(Borders::ALL).title("Search"));
                        f.render_widget(search_box, chunks[1]);
                        chunks[0]
                    }
                };

                render_main_layout(f, &state, main_layout_rect);
            })?;
        }

        std::thread::sleep(config::UI_REFRESH_DURATION);
    }
}
