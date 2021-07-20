use crate::config;
use crate::event;
use crate::prelude::*;
use crate::state;

pub fn start_ui(state: state::SharedState, send: mpsc::Sender<event::Event>) -> Result<()> {
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;

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
        let state = state.read().unwrap();

        if !state.is_running {
            // a `Quit` event is sent, clean up the application then exit
            crossterm::terminal::disable_raw_mode()?;
            crossterm::execute!(
                terminal.backend_mut(),
                crossterm::terminal::LeaveAlternateScreen
            )?;
            terminal.show_cursor()?;
            return Ok(());
        }

        if let Some(context) = state.current_playback_context.clone() {
            if let Some(PlayingItem::Track(track)) = context.item {
                let progress_in_sec: u32 = context.progress_ms.unwrap() / 1000;
                if let Some(playing_context) = context.context {
                    if let rspotify::senum::Type::Playlist = playing_context._type {
                        let playlist_id = playing_context.uri.split(':').nth(2).unwrap().to_owned();
                        let current_playlist_id = match state.current_playlist.as_ref() {
                            None => "".to_owned(),
                            Some(playlist) => playlist.id.clone(),
                        };
                        if current_playlist_id != playlist_id {
                            send.send(event::Event::GetPlaylist(playlist_id))?;
                        }
                    }
                }

                let playlist_info = match state.current_playlist.as_ref() {
                    None => "loading playlist...".to_owned(),
                    Some(playlist) => format!("{:?}", playlist.tracks.href),
                };
                let playback_info = format!(
                    "currently playing {} at {}/{} (repeat: {}, shuffle: {})\n{}\n",
                    track.name,
                    progress_in_sec,
                    track.duration_ms / 1000,
                    context.repeat_state.as_str(),
                    context.shuffle_state,
                    playlist_info,
                );

                let items = match state.current_playlist_tracks.as_ref() {
                    Some(tracks) => tracks
                        .iter()
                        .filter(|t| t.track.is_some())
                        .map(|t| ListItem::new(t.track.as_ref().unwrap().name.clone()))
                        .collect::<Vec<_>>(),
                    None => vec![],
                };

                terminal.draw(move |f| {
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints(
                            [Constraint::Percentage(30), Constraint::Percentage(70)].as_ref(),
                        )
                        .split(f.size());

                    let desc_block = Paragraph::new(playback_info)
                        .block(
                            Block::default()
                                .title("Current playback context")
                                .borders(Borders::ALL),
                        )
                        .wrap(Wrap { trim: true });
                    let tracks_block = List::new(items).block(
                        Block::default()
                            .title("Playlist tracks")
                            .borders(Borders::ALL),
                    );

                    f.render_widget(desc_block, chunks[0]);
                    f.render_widget(tracks_block, chunks[1]);
                })?;
            }
        }

        if std::time::SystemTime::now() > state.auth_token_expires_at {
            send.send(event::Event::RefreshToken)?;
        }
        std::thread::sleep(config::REFRESH_DURATION);
    }
}
