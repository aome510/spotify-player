use crate::{event, state::*, utils};
use anyhow::Result;
use tui::{layout::*, style::*, text::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod context;
mod help;
mod popup;

/// starts the application UI as the main thread
pub fn start_ui(state: SharedState, send: std::sync::mpsc::Sender<event::Event>) -> Result<()> {
    // terminal UI initializations
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;

    let ui_refresh_duration =
        std::time::Duration::from_millis(state.app_config.app_refresh_duration_in_ms);
    loop {
        if !state.ui.lock().unwrap().is_running {
            clean_up(terminal)?;
            return Ok(());
        }

        update_player_state(&state, &send)?;

        terminal.draw(|frame| {
            let ui = state.ui.lock().unwrap();

            let block = Block::default().style(ui.theme.app_style());
            frame.render_widget(block, frame.size());

            render_application_layout(frame, ui, &state, frame.size());
        })?;

        std::thread::sleep(ui_refresh_duration);
    }
}

fn update_player_state(
    state: &SharedState,
    send: &std::sync::mpsc::Sender<event::Event>,
) -> Result<()> {
    let player = state.player.read().unwrap();

    // updates the auth token if expired
    if std::time::Instant::now() > player.token.expires_at {
        send.send(event::Event::RefreshToken)?;
    }

    // updates the playback when the current playing song ends
    let progress_ms = player.get_playback_progress();
    let duration_ms = player.get_current_playing_track().map(|t| t.duration_ms);
    let is_playing = match player.playback {
        Some(ref playback) => playback.is_playing,
        None => false,
    };
    if let Some(progress_ms) = progress_ms {
        if progress_ms == duration_ms.unwrap() && is_playing {
            send.send(event::Event::GetCurrentPlayback)?;
        }
    }

    let ui = state.ui.lock().unwrap();

    match ui.page {
        PageState::Browsing(ref uri) => {
            if player.context_uri != *uri {
                utils::update_context(state, uri.clone());
            }
        }
        PageState::CurrentPlaying => {
            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = player.playback {
                match playback.context {
                    Some(ref context) => {
                        let uri = context.uri.clone();

                        if uri != player.context_uri {
                            utils::update_context(state, uri.clone());
                            if player.context_cache.peek(&uri).is_none() {
                                match context._type {
                                    rspotify::senum::Type::Playlist => send.send(
                                        event::Event::GetContext(event::ContextURI::Playlist(uri)),
                                    )?,
                                    rspotify::senum::Type::Album => send.send(
                                        event::Event::GetContext(event::ContextURI::Album(uri)),
                                    )?,
                                    rspotify::senum::Type::Artist => send.send(
                                        event::Event::GetContext(event::ContextURI::Artist(uri)),
                                    )?,
                                    _ => {
                                        send.send(event::Event::GetContext(
                                            event::ContextURI::Unknown(uri),
                                        ))?;
                                        log::info!(
                                            "encountered not supported context type: {:#?}",
                                            context._type
                                        )
                                    }
                                };
                            }
                        }
                    }
                    None => {
                        if !player.context_uri.is_empty() {
                            utils::update_context(state, "".to_string());
                            send.send(event::Event::GetContext(event::ContextURI::Unknown(
                                "".to_string(),
                            )))?;
                            log::info!("current playback doesn't have a playing context");
                        }
                    }
                }
            };
        }
    }

    Ok(())
}

/// cleans up the resources before quitting the application
fn clean_up(mut terminal: Terminal) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn render_application_layout(
    frame: &mut Frame,
    mut ui: UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    let rect = render_shortcut_helps(frame, &ui, state, rect);
    let (rect, is_active) = popup::render_popup(frame, &mut ui, state, rect);
    render_player_layout(is_active, frame, &mut ui, state, rect);
}

fn render_shortcut_helps(
    frame: &mut Frame,
    ui: &UIStateGuard,
    state: &SharedState,
    rect: Rect,
) -> Rect {
    let input = &ui.input_key_sequence;
    // render the shortcuts help table if needed
    let matches = if input.keys.is_empty() {
        vec![]
    } else {
        state
            .keymap_config
            .find_matched_prefix_keymaps(input)
            .into_iter()
            .map(|keymap| {
                let mut keymap = keymap.clone();
                keymap.key_sequence.keys.drain(0..input.keys.len());
                keymap
            })
            .filter(|keymap| !keymap.key_sequence.keys.is_empty())
            .collect::<Vec<_>>()
    };

    if matches.is_empty() {
        rect
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
            .split(rect);
        help::render_shortcuts_help_widget(matches, frame, ui, chunks[1]);
        chunks[0]
    }
}

fn render_player_layout(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(rect);
    render_current_playback_widget(frame, ui, state, chunks[0]);
    context::render_context_widget(is_active, frame, ui, state, chunks[1]);
}

fn render_current_playback_widget(
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .margin(1)
        .split(rect);

    let block = Block::default()
        .title(ui.theme.block_title_with_style("Playback"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    let player = state.player.read().unwrap();
    if let Some(ref playback) = player.playback {
        if let Some(rspotify::model::PlayingItem::Track(ref track)) = playback.item {
            let playback_info = vec![
                Span::styled(
                    format!(
                        "{} by {}",
                        track.name,
                        track
                            .artists
                            .iter()
                            .map(|a| a.name.clone())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    ui.theme.playback_track(),
                )
                .into(),
                Span::styled(track.album.name.to_string(), ui.theme.playback_album()).into(),
                Span::styled(
                    format!(
                        "repeat: {} | shuffle: {} | volume: {}%",
                        playback.repeat_state.as_str(),
                        playback.shuffle_state,
                        playback.device.volume_percent,
                    ),
                    ui.theme.playback_metadata(),
                )
                .into(),
            ];

            let playback_desc = Paragraph::new(playback_info)
                .wrap(Wrap { trim: true })
                // .style(theme.text_desc_style())
                .block(Block::default());
            let progress_ms = player.get_playback_progress().unwrap();
            let progress_bar = Gauge::default()
                .block(Block::default())
                .gauge_style(ui.theme.playback_progress_bar())
                .ratio((progress_ms as f64) / (track.duration_ms as f64))
                .label(Span::styled(
                    format!(
                        "{}/{}",
                        utils::format_duration(progress_ms),
                        utils::format_duration(track.duration_ms),
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                ));

            ui.progress_bar_rect = chunks[1];

            frame.render_widget(playback_desc, chunks[0]);
            frame.render_widget(progress_bar, chunks[1]);
        }
    };
}
