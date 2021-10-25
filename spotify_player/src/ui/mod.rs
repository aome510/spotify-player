use crate::{
    event::ClientRequest,
    state::*,
    utils::{self, new_table_state},
};
use anyhow::Result;
use rspotify::model;
use std::sync::RwLockReadGuard;
use tui::{layout::*, style::*, text::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod context;
mod help;
mod popup;
mod search;

/// starts the application UI as the main thread
pub fn start_ui(state: SharedState, send: std::sync::mpsc::Sender<ClientRequest>) -> Result<()> {
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

        handle_page_state_change(&state, &send)?;

        terminal.draw(|frame| {
            let ui = state.ui.lock().unwrap();

            // set the background and foreground colors for the application
            let block = Block::default().style(ui.theme.app_style());
            frame.render_widget(block, frame.size());

            render_application(frame, ui, &state, frame.size());
        })?;

        std::thread::sleep(ui_refresh_duration);
    }
}

/// checks the current page state for changes
/// and updates the window state (and other states) accordingly
fn handle_page_state_change(
    state: &SharedState,
    send: &std::sync::mpsc::Sender<ClientRequest>,
) -> Result<()> {
    let mut ui = state.ui.lock().unwrap();

    match ui.current_page() {
        PageState::Searching { .. } => {
            state.player.write().unwrap().context_id = None;
            match ui.window {
                WindowState::Search { .. } => {}
                _ => {
                    ui.window = WindowState::new_search_state();
                }
            }
        }
        PageState::Recommendations(..) => {
            state.player.write().unwrap().context_id = None;
            match ui.window {
                WindowState::Recommendations { .. } => {}
                _ => {
                    ui.window = WindowState::Recommendations {
                        track_table: new_table_state(),
                    };
                }
            }
        }
        PageState::Browsing(id) => {
            let should_update = match state.player.read().unwrap().context_id {
                None => true,
                Some(ref context_id) => context_id != id,
            };
            if should_update {
                utils::update_context(state, Some(id.clone()));
            }
        }
        PageState::CurrentPlaying => {
            let player = state.player.read().unwrap();
            // updates the context (album, playlist, etc) tracks based on the current playback
            if let Some(ref playback) = player.playback {
                match playback.context {
                    Some(ref context) => {
                        let should_update = match player.context_id {
                            None => true,
                            Some(ref context_id) => context_id.uri() != context.uri,
                        };

                        if should_update {
                            match context._type {
                                model::Type::Playlist => {
                                    let context_id =
                                        ContextId::Playlist(PlaylistId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                model::Type::Album => {
                                    let context_id =
                                        ContextId::Album(AlbumId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                model::Type::Artist => {
                                    let context_id =
                                        ContextId::Artist(ArtistId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                _ => {
                                    log::info!(
                                        "encountered not supported context type: {:#?}",
                                        context._type
                                    )
                                }
                            };
                        }
                    }
                    None => {
                        if player.context_id.is_some() {
                            // the current playback doesn't have a playing context,
                            // update the state's `context_id` to `None`
                            utils::update_context(state, None);
                            log::info!("current playback does not have a playing context");
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

/// renders the application
fn render_application(frame: &mut Frame, mut ui: UIStateGuard, state: &SharedState, rect: Rect) {
    let rect = help::render_shortcut_help_window(frame, &ui, state, rect);

    let (rect, is_active) = popup::render_popup(frame, &mut ui, state, rect);

    render_main_layout(is_active, frame, &mut ui, state, rect);
}

/// renders the application's main layout which consists of:
/// - a playback window on top
/// - a context window or a search window at bottom depending on the current UI's `PageState`
fn render_main_layout(
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
    render_playback_window(frame, ui, state, chunks[0]);

    match ui.current_page() {
        PageState::CurrentPlaying => {
            context::render_context_window(
                is_active,
                frame,
                ui,
                state,
                chunks[1],
                "Context (Current Playing)",
            );
        }
        PageState::Browsing { .. } => {
            context::render_context_window(
                is_active,
                frame,
                ui,
                state,
                chunks[1],
                "Context (Browsing)",
            );
        }
        PageState::Recommendations { .. } => {
            render_recommendation_window(is_active, frame, ui, state, chunks[1]);
        }
        PageState::Searching { .. } => {
            // make sure that the window state matches the current page state.
            // The mismatch can happen when going back to the search from another page
            search::render_search_window(is_active, frame, ui, state, chunks[1]);
        }
    };
}

/// renders the recommendation window
fn render_recommendation_window(
    is_active: bool,
    frame: &mut Frame,
    ui: &mut UIStateGuard,
    state: &SharedState,
    rect: Rect,
) {
    let seed = match ui.current_page() {
        PageState::Recommendations(seed) => seed,
        _ => unreachable!(),
    };

    let block = Block::default()
        .title(ui.theme.block_title_with_style("Recommendations"))
        .borders(Borders::ALL);

    let data = state.data.read().unwrap();

    let tracks = match data.caches.recommendation.peek(&seed.uri()) {
        Some(tracks) => tracks,
        None => {
            // recommendation tracks are still loading
            frame.render_widget(Paragraph::new("loading...").block(block), rect);
            return;
        }
    };

    // render the window's border and title
    frame.render_widget(block, rect);

    // render the window's description
    let desc = match seed {
        SeedItem::Track(ref track) => format!("{} Radio", track.name),
        SeedItem::Artist(ref artist) => format!("{} Radio", artist.name),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(rect);
    let context_desc = Paragraph::new(desc).block(Block::default().style(ui.theme.context_desc()));
    frame.render_widget(context_desc, chunks[0]);

    let player = state.player.read().unwrap();
    let track_table = construct_track_table_widget(
        is_active,
        ui,
        state,
        &player,
        ui.filtered_items_by_search(tracks),
    );

    if let Some(state) = ui.window.track_table_state() {
        frame.render_stateful_widget(track_table, chunks[1], state)
    }
}

/// renders a playback window showing information about the current playback such as
/// - track title, artists, album
/// - playback metadata (playing state, repeat state, shuffle state, volume, device, etc)
fn render_playback_window(
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
        if let Some(rspotify::model::PlayableItem::Track(ref track)) = playback.item {
            let playback_info = vec![
                Span::styled(
                    format!(
                        "{}  {} by {}",
                        if !playback.is_playing { "⏸" } else { "▶" },
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
                        "repeat: {} | shuffle: {} | volume: {}% | device: {}",
                        playback.repeat_state.as_ref(),
                        playback.shuffle_state,
                        playback.device.volume_percent.unwrap_or_default(),
                        playback.device.name,
                    ),
                    ui.theme.playback_metadata(),
                )
                .into(),
            ];

            let playback_desc = Paragraph::new(playback_info)
                .wrap(Wrap { trim: true })
                // .style(theme.text_desc_style())
                .block(Block::default());
            let progress = std::cmp::min(player.playback_progress().unwrap(), track.duration);
            let progress_bar = Gauge::default()
                .block(Block::default())
                .gauge_style(ui.theme.playback_progress_bar())
                .ratio(progress.as_secs_f64() / track.duration.as_secs_f64())
                .label(Span::styled(
                    format!(
                        "{}/{}",
                        utils::format_duration(progress),
                        utils::format_duration(track.duration),
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                ));

            ui.progress_bar_rect = chunks[1];

            frame.render_widget(playback_desc, chunks[0]);
            frame.render_widget(progress_bar, chunks[1]);
        }
    };
}

/// constructs a generic list widget
fn construct_list_widget<'a>(
    ui: &UIStateGuard,
    items: Vec<(String, bool)>,
    title: &str,
    is_active: bool,
    borders: Option<Borders>,
) -> List<'a> {
    let borders = borders.unwrap_or(Borders::ALL);

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
    .highlight_style(ui.theme.selection_style(is_active))
    .block(
        Block::default()
            .title(ui.theme.block_title_with_style(title))
            .borders(borders),
    )
}

/// constructs a track table widget
pub fn construct_track_table_widget<'a>(
    is_active: bool,
    ui: &UIStateGuard,
    state: &SharedState,
    player: &RwLockReadGuard<PlayerState>,
    tracks: Vec<&Track>,
) -> Table<'a> {
    // get the current playing track's URI to
    // highlight such track (if exists) in the track table
    let mut playing_track_uri = "".to_string();
    let mut active_desc = "";
    if let Some(ref playback) = player.playback {
        if let Some(rspotify::model::PlayableItem::Track(ref track)) = playback.item {
            playing_track_uri = track.id.uri();
            active_desc = if !playback.is_playing { "⏸" } else { "▶" };
        }
    }

    let item_max_len = state.app_config.track_table_item_max_len;
    let rows = tracks
        .into_iter()
        .enumerate()
        .map(|(id, t)| {
            let (id, style) = if playing_track_uri == t.id.uri() {
                (active_desc.to_string(), ui.theme.current_active())
            } else {
                ((id + 1).to_string(), Style::default())
            };
            Row::new(vec![
                Cell::from(id),
                Cell::from(utils::truncate_string(t.name.clone(), item_max_len)),
                Cell::from(utils::truncate_string(t.artists_info(), item_max_len)),
                Cell::from(utils::truncate_string(t.album_info(), item_max_len)),
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
            Constraint::Length(4),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
        ])
        .highlight_style(ui.theme.selection_style(is_active))
}
