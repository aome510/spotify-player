use crate::event;
use crate::state;
use crate::utils;
use anyhow::Result;
use tui::{layout::*, style::*, text::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod help;

/// starts the application UI as the main thread
pub fn start_ui(
    state: state::SharedState,
    send: std::sync::mpsc::Sender<event::Event>,
) -> Result<()> {
    // terminal UI initializations
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;

    let ui_refresh_duration = std::time::Duration::from_millis(
        state.read().unwrap().app_config.app_refresh_duration_in_ms,
    );
    loop {
        {
            let state = state.read().unwrap();
            if !state.is_running {
                clean_up(terminal)?;
                return Ok(());
            }
            if std::time::SystemTime::now() > state.auth_token_expires_at {
                send.send(event::Event::RefreshToken)?;
            }

            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = state.playback {
                if let Some(ref context) = playback.context {
                    match context._type {
                        rspotify::senum::Type::Playlist => {
                            let playlist_id = context.uri.split(':').nth(2).unwrap();
                            let current_playlist_id =
                                if let state::PlayingContext::Playlist(ref playlist, _) =
                                    state.context
                                {
                                    &playlist.id
                                } else {
                                    ""
                                };
                            if current_playlist_id != playlist_id {
                                send.send(event::Event::PlaylistAsContext(playlist_id.to_owned()))?;
                            }
                        }
                        rspotify::senum::Type::Album => {
                            let album_id = context.uri.split(':').nth(2).unwrap();
                            let current_album_id =
                                if let state::PlayingContext::Album(ref album, _) = state.context {
                                    &album.id
                                } else {
                                    ""
                                };
                            if current_album_id != album_id {
                                send.send(event::Event::AlbumAsContext(album_id.to_owned()))?;
                            }
                        }
                        _ => {}
                    };
                }
            };
        }

        terminal.draw(|f| {
            let block =
                Block::default().style(state.read().unwrap().app_config.theme_config.app_style());
            f.render_widget(block, f.size());

            render_application_layout(f, &state, f.size());
        })?;

        std::thread::sleep(ui_refresh_duration);
    }
}

/// cleans up the resources before quitting the application
fn clean_up(mut terminal: Terminal) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn render_application_layout(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    // render the shortcuts help table if needed
    let matches = {
        let state = state.read().unwrap();
        if state.shortcuts_help_ui_state {
            let prefix = &state.input_key_sequence;
            state
                .keymap_config
                .find_matched_prefix_keymaps(prefix)
                .into_iter()
                .map(|keymap| {
                    let mut keymap = keymap.clone();
                    keymap.key_sequence.keys.drain(0..prefix.keys.len());
                    keymap
                })
                .filter(|keymap| !keymap.key_sequence.keys.is_empty())
                .collect::<Vec<_>>()
        } else {
            vec![]
        }
    };
    let rect = if matches.is_empty() {
        rect
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
            .split(rect);
        help::render_shortcuts_help_widget(matches, frame, state, chunks[1]);
        chunks[0]
    };

    let (player_layout_rect, is_active) = {
        let event_state = state.read().unwrap().popup_state.clone();
        match event_state {
            state::PopupState::None => (rect, true),
            state::PopupState::CommandHelp => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                    .split(rect);
                help::render_commands_help_widget(frame, state, chunks[1]);
                (chunks[0], false)
            }
            state::PopupState::PlaylistSwitch => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                    .split(rect);
                render_playlists_widget(frame, state, chunks[1]);
                (chunks[0], false)
            }
            state::PopupState::ContextSearch => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(frame.size());
                render_search_box_widget(frame, state, chunks[1]);
                (chunks[0], true)
            }
        }
    };

    render_player_layout(is_active, frame, state, player_layout_rect);
}

fn render_player_layout(is_active: bool, f: &mut Frame, state: &state::SharedState, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(rect);
    render_current_playback_widget(f, state, chunks[0]);
    render_playlist_tracks_widget(is_active, f, state, chunks[1]);
}

fn render_search_box_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let state = state.read().unwrap();
    let theme = &state.app_config.theme_config;
    let search_box = Paragraph::new(state.context_search_state.query.clone().unwrap()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(theme.block_title_with_style("Search")),
    );
    frame.render_widget(search_box, rect);
}

fn render_playlists_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let list = {
        let state = state.read().unwrap();
        let theme = &state.app_config.theme_config;

        List::new(
            state
                .user_playlists
                .iter()
                .map(|p| ListItem::new(p.name.clone()))
                .collect::<Vec<_>>(),
        )
        .highlight_style(theme.selection_style())
        // mostly to create a left margin
        .highlight_symbol("  ")
        .block(
            Block::default()
                .title(theme.block_title_with_style("Playlists"))
                .borders(Borders::ALL),
        )
    };
    frame.render_stateful_widget(
        list,
        rect,
        &mut state.write().unwrap().playlists_list_ui_state,
    );
}

fn render_current_playback_widget(frame: &mut Frame, state: &state::SharedState, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .margin(1)
        .split(rect);

    let state = state.read().unwrap();
    let theme = &state.app_config.theme_config;

    let block = Block::default()
        .title(theme.block_title_with_style("Current Playback"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    if let Some(ref context) = state.playback {
        if let Some(rspotify::model::PlayingItem::Track(ref track)) = context.item {
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
                    theme.primary_text_desc_style(),
                )
                .into(),
                Span::styled(
                    track.album.name.to_string(),
                    theme.secondary_text_desc_style(),
                )
                .into(),
                Span::styled(
                    format!(
                        "repeat: {} | shuffle: {} | volume: {}%",
                        context.repeat_state.as_str(),
                        context.shuffle_state,
                        context.device.volume_percent,
                    ),
                    theme.comment_style(),
                )
                .into(),
            ];

            let playback_desc = Paragraph::new(playback_info)
                .wrap(Wrap { trim: true })
                // .style(theme.text_desc_style())
                .block(Block::default());
            let progress_bar = Gauge::default()
                .block(Block::default())
                .gauge_style(theme.gauge_style())
                .ratio((context.progress_ms.unwrap() as f64) / (track.duration_ms as f64))
                .label(Span::styled(
                    format!(
                        "{}/{}",
                        utils::format_duration(context.progress_ms.unwrap()),
                        utils::format_duration(track.duration_ms),
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            frame.render_widget(playback_desc, chunks[0]);
            frame.render_widget(progress_bar, chunks[1]);
        }
    };
}

fn render_playlist_tracks_widget(
    is_active: bool,
    frame: &mut Frame,
    state: &state::SharedState,
    rect: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .margin(1)
        .split(rect);

    let (context_desc, track_table) = {
        let state = state.read().unwrap();
        let theme = &state.app_config.theme_config;

        let block = Block::default()
            .title(theme.block_title_with_style("Context Tracks"))
            .borders(Borders::ALL);
        frame.render_widget(block, rect);

        let mut playing_track_uri = "";
        if let Some(ref context) = state.playback {
            if let Some(rspotify::model::PlayingItem::Track(ref track)) = context.item {
                playing_track_uri = &track.uri;
            }
        }

        let item_max_len = state.app_config.track_table_item_max_len;
        let rows = state
            .get_context_filtered_tracks()
            .into_iter()
            .map(|t| {
                let (name, style) = if playing_track_uri == t.uri {
                    (format!("▶ {}", t.name), theme.current_playing_style())
                } else {
                    (t.name.clone(), Style::default())
                };
                Row::new(vec![
                    Cell::from(utils::truncate_string(name, item_max_len)),
                    Cell::from(utils::truncate_string(t.get_artists_info(), item_max_len)),
                    Cell::from(utils::truncate_string(t.album.name.clone(), item_max_len)),
                    Cell::from(utils::format_duration(t.duration)),
                ])
                .style(style)
            })
            .collect::<Vec<_>>();

        let context_desc = Paragraph::new(state.get_context_description())
            .block(Block::default().style(theme.primary_text_desc_style()));
        let track_table = Table::new(rows)
            .header(
                Row::new(vec![
                    Cell::from("Track"),
                    Cell::from("Artists"),
                    Cell::from("Album"),
                    Cell::from("Duration"),
                ])
                .style(theme.table_header_style()),
            )
            .block(Block::default())
            .widths(&[
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(10),
            ])
            .highlight_style(if is_active {
                theme.selection_style()
            } else {
                Style::default()
            })
            // mostly to create a left margin
            .highlight_symbol("  ");

        (context_desc, track_table)
    };

    frame.render_widget(context_desc, chunks[0]);
    frame.render_stateful_widget(
        track_table,
        chunks[1],
        &mut state.write().unwrap().context_tracks_table_ui_state,
    );
}
