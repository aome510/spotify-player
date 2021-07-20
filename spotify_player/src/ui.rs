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

        let text = if let Some(context) = state.current_playback_context.clone() {
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
                let playlist_tracks_info = match state.current_playlist_tracks.as_ref() {
                    None => "loading playlist track...".to_owned(),
                    Some(tracks) => {
                        format!("there are {} track(s) in the playlist", tracks.len())
                    }
                };

                format!(
                    "currently playing {} at {}/{} (repeat: {}, shuffle: {})\n{}\n{}",
                    track.name,
                    progress_in_sec,
                    track.duration_ms / 1000,
                    context.repeat_state.as_str(),
                    context.shuffle_state,
                    playlist_info,
                    playlist_tracks_info,
                )
            } else {
                "loading current playback...".to_owned()
            }
        } else {
            "loading current playback...".to_owned()
        };

        terminal.draw(move |f| {
            let ui = Paragraph::new(text)
                .block(
                    Block::default()
                        .title("Current playing")
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(ui, f.size());
        })?;

        if std::time::SystemTime::now() > state.auth_token_expires_at {
            send.send(event::Event::RefreshToken)?;
        }
        std::thread::sleep(config::REFRESH_DURATION);
    }
}
