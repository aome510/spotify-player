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
    state: &state::SharedState,
    send: &std::sync::mpsc::Sender<event::Event>,
) -> Result<()> {
    let player = state.player.read().unwrap();

    // updates the auth token if expired
    if std::time::SystemTime::now() > player.auth_token_expires_at {
        send.send(event::Event::RefreshToken)?;
    }

    // updates the playback when the current playing song ends
    let progress_ms = player.get_playback_progress();
    let duration_ms = player.get_current_playing_track().map(|t| t.duration_ms);
    if let Some(progress_ms) = progress_ms {
        if progress_ms == duration_ms.unwrap() {
            send.send(event::Event::GetCurrentPlayback)?;
        }
    }

    let ui = state.ui.lock().unwrap();

    match ui.frame_state {
        state::FrameState::Browse(ref uri) => {
            let context_uri = player.context.get_uri();
            if context_uri != uri {
                let context = player.context_cache.peek(uri);
                if let Some(context) = context {
                    utils::update_context(state, context.clone());
                }
            }
        }
        state::FrameState::Default => {
            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = player.playback {
                if let Some(ref playback) = playback.context {
                    let uri = playback.uri.clone();
                    let context_uri = player.context.get_uri();

                    if uri != context_uri {
                        let context = player.context_cache.peek(&uri);
                        match context {
                            Some(context) => {
                                utils::update_context(state, context.clone());
                            }
                            None => {
                                match playback._type {
                                    rspotify::senum::Type::Playlist => send.send(
                                        event::Event::GetContext(event::Context::Playlist(uri)),
                                    )?,
                                    rspotify::senum::Type::Album => send.send(
                                        event::Event::GetContext(event::Context::Album(uri)),
                                    )?,
                                    rspotify::senum::Type::Artist => send.send(
                                        event::Event::GetContext(event::Context::Artist(uri)),
                                    )?,
                                    _ => {
                                        send.send(event::Event::GetContext(
                                            event::Context::Unknown(uri),
                                        ))?;
                                        log::info!(
                                            "encountered not supported context type: {:#?}",
                                            playback._type
                                        )
                                    }
                                };
                            }
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
    mut ui: state::UIStateGuard,
    state: &state::SharedState,
    rect: Rect,
) {
    // render the shortcuts help table if needed
    let matches = {
        if ui.shortcuts_help_ui_state {
            let prefix = &ui.input_key_sequence;
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
        help::render_shortcuts_help_widget(matches, frame, &ui, chunks[1]);
        chunks[0]
    };

    let (player_layout_rect, is_active) = render_popup(frame, &mut ui, state, rect);

    render_player_layout(is_active, frame, &mut ui, state, player_layout_rect);
}

fn render_popup(
    frame: &mut Frame,
    ui: &mut state::UIStateGuard,
    state: &state::SharedState,
    rect: Rect,
) -> (Rect, bool) {
    // handle popup windows
    match ui.popup_state {
        state::PopupState::None => (rect, true),
        state::PopupState::CommandHelp => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
                .split(rect);
            help::render_commands_help_widget(frame, ui, state, chunks[1]);
            (chunks[0], false)
        }
        state::PopupState::DeviceList => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(5)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let player = state.player.read().unwrap();
                    let current_device_id = match player.playback {
                        Some(ref playback) => &playback.device.id,
                        None => "",
                    };
                    let items = player
                        .devices
                        .iter()
                        .map(|d| (format!("{} | {}", d.name, d.id), current_device_id == d.id))
                        .collect();
                    construct_list_widget(ui, items, "Devices")
                },
                chunks[1],
                &mut ui.devices_list_ui_state,
            );
            (chunks[0], false)
        }
        state::PopupState::ThemeList(ref themes) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let items = themes.iter().map(|t| (t.name.clone(), false)).collect();
                    construct_list_widget(ui, items, "Themes")
                },
                chunks[1],
                &mut ui.themes_list_ui_state,
            );
            (chunks[0], false)
        }
        state::PopupState::PlaylistList => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(10)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let player = state.player.read().unwrap();
                    let current_playlist_name =
                        if let state::Context::Playlist(ref playlist, _) = player.context {
                            &playlist.name
                        } else {
                            ""
                        };
                    let items = player
                        .user_playlists
                        .iter()
                        .map(|p| (p.name.clone(), p.name == current_playlist_name))
                        .collect();
                    construct_list_widget(ui, items, "Playlists")
                },
                chunks[1],
                &mut ui.playlists_list_ui_state,
            );
            (chunks[0], false)
        }
        state::PopupState::ArtistList(ref artists) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(5)].as_ref())
                .split(rect);
            frame.render_stateful_widget(
                {
                    let items = artists.iter().map(|a| (a.name.clone(), false)).collect();
                    construct_list_widget(ui, items, "Artists")
                },
                chunks[1],
                &mut ui.artist_list_ui_state,
            );
            (chunks[0], false)
        }
        state::PopupState::ContextSearch(ref query) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                .split(frame.size());
            render_search_box_widget(frame, ui, chunks[1], format!("/{}", query));
            (chunks[0], true)
        }
    }
}

fn render_player_layout(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut state::UIStateGuard,
    state: &state::SharedState,
    rect: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(rect);
    render_current_playback_widget(frame, ui, state, chunks[0]);
    render_widget(is_active, frame, ui, state, chunks[1]);
}

fn render_widget(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut state::UIStateGuard,
    state: &state::SharedState,
    rect: Rect,
) {
    // context widget box border
    let block = Block::default()
        .title(ui.theme.block_title_with_style("Context"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    // context description
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .margin(1)
        .split(rect);
    let context_desc = Paragraph::new(state.player.read().unwrap().context.get_description())
        .block(Block::default().style(ui.theme.context_desc()));
    frame.render_widget(context_desc, chunks[0]);

    // context data widget(s)
    //
    // TODO: improve UI for artist context
    let rect = {
        let player = state.player.read().unwrap();
        match player.context {
            state::Context::Artist(_, _, ref albums) => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(12), Constraint::Min(1)].as_ref())
                    .split(chunks[1]);

                let current_album = player
                    .get_current_playing_track()
                    .map(|t| t.album.name.clone())
                    .unwrap_or_default();

                let list = List::new(
                    albums
                        .iter()
                        .map(|a| {
                            ListItem::new(a.name.clone()).style(if a.name == current_album {
                                ui.theme.current_active()
                            } else {
                                Style::default()
                            })
                        })
                        .collect::<Vec<_>>(),
                )
                .highlight_style(ui.theme.selection_style())
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .title(ui.theme.block_title_with_style("Albums")),
                );

                frame.render_widget(list, chunks[1]);
                chunks[0]
            }
            _ => chunks[1],
        }
    };

    render_context_tracks_widget(is_active, frame, ui, state, rect);
}

fn render_search_box_widget(
    frame: &mut Frame,
    ui: &state::UIStateGuard,
    rect: Rect,
    query: String,
) {
    let search_box = Paragraph::new(query).block(
        Block::default()
            .borders(Borders::ALL)
            .title(ui.theme.block_title_with_style("Search")),
    );
    frame.render_widget(search_box, rect);
}

fn render_current_playback_widget(
    frame: &mut Frame,
    ui: &mut state::UIStateGuard,
    state: &state::SharedState,
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

fn render_context_tracks_widget(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut state::UIStateGuard,
    state: &state::SharedState,
    rect: Rect,
) {
    let track_table = {
        let player = state.player.read().unwrap();

        let mut playing_track_uri = "";
        if let Some(ref playback) = player.playback {
            if let Some(rspotify::model::PlayingItem::Track(ref track)) = playback.item {
                playing_track_uri = &track.uri;
            }
        }

        let item_max_len = state.app_config.track_table_item_max_len;
        let rows = ui
            .get_context_tracks(&player)
            .into_iter()
            .enumerate()
            .map(|(id, t)| {
                let (id, style) = if playing_track_uri == t.uri {
                    ("▶".to_string(), ui.theme.current_active())
                } else {
                    ((id + 1).to_string(), Style::default())
                };
                Row::new(vec![
                    Cell::from(id),
                    Cell::from(utils::truncate_string(t.name.clone(), item_max_len)),
                    Cell::from(utils::truncate_string(t.get_artists_info(), item_max_len)),
                    Cell::from(utils::truncate_string(t.album.name.clone(), item_max_len)),
                    Cell::from(utils::format_duration(t.duration)),
                ])
                .style(style)
            })
            .collect::<Vec<_>>();

        Table::new(rows)
            .header(
                Row::new(vec![
                    Cell::from("#"),
                    Cell::from("Track"),
                    Cell::from("Artists"),
                    Cell::from("Album"),
                    Cell::from("Duration"),
                ])
                .style(ui.theme.context_tracks_table_header()),
            )
            .block(Block::default())
            .widths(&[
                Constraint::Percentage(3),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
                Constraint::Percentage(7),
            ])
            .highlight_style(if is_active {
                ui.theme.selection_style()
            } else {
                Style::default()
            })
    };

    frame.render_stateful_widget(track_table, rect, &mut ui.context_tracks_table_ui_state);
}

fn construct_list_widget<'a>(
    ui: &state::UIStateGuard,
    items: Vec<(String, bool)>,
    title: &str,
) -> List<'a> {
    List::new(
        items
            .into_iter()
            .map(|(s, is_active)| {
                ListItem::new(s).style(if is_active {
                    ui.theme.current_active()
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>(),
    )
    .highlight_style(ui.theme.selection_style())
    .block(
        Block::default()
            .title(ui.theme.block_title_with_style(title))
            .borders(Borders::ALL),
    )
}
