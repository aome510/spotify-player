use crate::config;
use crate::event;
use crate::prelude::*;
use crate::state;
use crate::ui::help::get_sort_shortcuts;
use std::io::Stdout;
use tui::backend::CrosstermBackend;

type Terminal = tui::Terminal<CrosstermBackend<Stdout>>;
type Frame<'a> = tui::Frame<'a, CrosstermBackend<Stdout>>;

mod help;
mod utils;

fn render_current_playback_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let playback_info = if let Some(ref context) = state.read().unwrap().current_playback_context {
        if let Some(PlayingItem::Track(ref track)) = context.item {
            format!(
                "Current track: {} at {}/{} (playing: {}, repeat: {}, shuffle: {})\n",
                track.name,
                format_duration(context.progress_ms.unwrap()),
                format_duration(track.duration_ms),
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

fn render_playlist_tracks_widget(
    is_active: bool,
    frame: &mut Frame,
    state: &state::SharedState,
    rect: Rect,
) {
    let rows = state
        .read()
        .unwrap()
        .get_context_filtered_tracks()
        .into_iter()
        .map(|t| {
            Row::new(vec![
                Cell::from(state::truncate_string(
                    t.name.clone(),
                    config::TRACK_DESC_ITEM_MAX_LEN,
                )),
                Cell::from(state::truncate_string(
                    t.get_artists_info(),
                    config::TRACK_DESC_ITEM_MAX_LEN,
                )),
                Cell::from(state::truncate_string(
                    t.album.name.clone(),
                    config::TRACK_DESC_ITEM_MAX_LEN,
                )),
                Cell::from(format_duration(t.duration)),
            ])
        })
        .collect::<Vec<_>>();
    let widget = Table::new(rows)
        .header(
            Row::new(vec![
                Cell::from("Track"),
                Cell::from("Artists"),
                Cell::from("Album"),
                Cell::from("Duration"),
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
            Constraint::Percentage(30),
            Constraint::Percentage(10),
        ])
        .highlight_style(if is_active {
            Style::default().add_modifier(Modifier::ITALIC)
        } else {
            Style::default()
        })
        // mostly to create a left margin of two
        .highlight_symbol("  ");
    frame.render_stateful_widget(
        widget,
        rect,
        &mut state.write().unwrap().context_tracks_table_ui_state,
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

fn render_main_layout(is_active: bool, f: &mut Frame, state: &state::SharedState, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(rect);
    render_current_playback_widget(f, &state, chunks[0]);
    render_playlist_tracks_widget(is_active, f, &state, chunks[1]);
}

fn render_playlists_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let list = List::new(
        state
            .read()
            .unwrap()
            .current_playlists
            .iter()
            .map(|p| ListItem::new(p.name.clone()))
            .collect::<Vec<_>>(),
    )
    .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
    // mostly to create a left margin of two
    .highlight_symbol("  ")
    .block(Block::default().title("Playlists").borders(Borders::ALL));
    frame.render_stateful_widget(
        list,
        rect,
        &mut state.write().unwrap().playlists_list_ui_state,
    );
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

            if let Some(ref playback) = state.current_playback_context {
                if let Some(ref context) = playback.context {
                    match context._type {
                        Type::Playlist => {
                            let playlist_id = context.uri.split(':').nth(2).unwrap();
                            let current_playlist_id = match state.current_playlist {
                                Some(ref playlist) => &playlist.id,
                                None => "",
                            };
                            if current_playlist_id != playlist_id {
                                send.send(event::Event::GetPlaylist(playlist_id.to_owned()))?;
                            }
                        }
                        Type::Album => {
                            let album_id = context.uri.split(':').nth(2).unwrap();
                            let current_album_id = match state.current_album {
                                Some(ref album) => &album.id,
                                None => "",
                            };
                            if current_album_id != album_id {
                                send.send(event::Event::GetAlbum(album_id.to_owned()))?;
                            }
                        }
                        _ => {}
                    };
                }
            };
        }

        {
            // draw ui
            terminal.draw(|f| {
                let (main_layout_rect, is_active) = {
                    let event_state = state.read().unwrap().current_event_state.clone();
                    match event_state {
                        state::EventState::Default => (f.size(), true),
                        state::EventState::PlaylistSwitch => {
                            let chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                                .split(f.size());
                            render_playlists_widget(f, &state, chunks[1]);
                            (chunks[0], false)
                        }
                        state::EventState::ContextSearch => {
                            let chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                                .split(f.size());
                            let search_box = Paragraph::new(
                                state
                                    .read()
                                    .unwrap()
                                    .context_search_state
                                    .query
                                    .clone()
                                    .unwrap(),
                            )
                            .block(Block::default().borders(Borders::ALL).title("Search"));
                            f.render_widget(search_box, chunks[1]);
                            (chunks[0], true)
                        }
                        state::EventState::Sort => {
                            let chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
                                .split(f.size());
                            let table = help::get_shortcut_table(get_sort_shortcuts());
                            f.render_widget(table, chunks[1]);
                            (chunks[0], true)
                        }
                    }
                };

                render_main_layout(is_active, f, &state, main_layout_rect);
            })?;
        }

        std::thread::sleep(config::UI_REFRESH_DURATION);
    }
}
