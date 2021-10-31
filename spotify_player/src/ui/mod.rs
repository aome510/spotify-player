use crate::{event::ClientRequest, state::*, utils};
use anyhow::Result;
use std::sync::mpsc;
use tui::{layout::*, style::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod help;
mod popup;
mod window;

/// starts the application UI rendering function(s)
pub fn start_ui(state: SharedState, send: mpsc::Sender<ClientRequest>) -> Result<()> {
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
        if !state.ui.lock().is_running {
            clean_up(terminal)?;
            return Ok(());
        }

        handle_page_state_change(&state, &send)?;

        terminal.draw(|frame| {
            // set the background and foreground colors for the application
            let block = Block::default().style(state.ui.lock().theme.app_style());
            frame.render_widget(block, frame.size());

            render_application(frame, &state, frame.size());
        })?;

        std::thread::sleep(ui_refresh_duration);
    }
}

/// checks the current UI page state for new changes
/// to update the UI window state and other states accordingly
fn handle_page_state_change(state: &SharedState, send: &mpsc::Sender<ClientRequest>) -> Result<()> {
    let mut ui = state.ui.lock();

    match ui.current_page() {
        PageState::Library => match ui.window {
            WindowState::Library { .. } => {}
            _ => {
                send.send(ClientRequest::GetUserPlaylists)?;
                send.send(ClientRequest::GetUserSavedAlbums)?;
                send.send(ClientRequest::GetUserFollowedArtists)?;

                ui.window = WindowState::Library {
                    playlist_list: utils::new_list_state(),
                    saved_album_list: utils::new_list_state(),
                    followed_artist_list: utils::new_list_state(),
                    focus: LibraryFocusState::Playlists,
                }
            }
        },
        PageState::Searching { current_query, .. } => {
            state.player.write().context_id = None;
            match ui.window {
                WindowState::Search { .. } => {}
                _ => {
                    send.send(ClientRequest::Search(current_query.clone()))?;
                    ui.window = WindowState::new_search_state();
                }
            }
        }
        PageState::Recommendations(seed) => {
            state.player.write().context_id = None;
            match ui.window {
                WindowState::Recommendations { .. } => {}
                _ => {
                    send.send(ClientRequest::GetRecommendations(seed.clone()))?;
                    ui.window = WindowState::Recommendations {
                        track_table: utils::new_table_state(),
                    };
                }
            }
        }
        PageState::Browsing(id) => {
            let should_update = match state.player.read().context_id {
                None => true,
                Some(ref context_id) => context_id != id,
            };
            if should_update {
                utils::update_context(state, Some(id.clone()));
            }
        }
        PageState::CurrentPlaying => {
            let player = state.player.read();
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
                                rspotify_model::Type::Playlist => {
                                    let context_id =
                                        ContextId::Playlist(PlaylistId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                rspotify_model::Type::Album => {
                                    let context_id =
                                        ContextId::Album(AlbumId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                rspotify_model::Type::Artist => {
                                    let context_id =
                                        ContextId::Artist(ArtistId::from_uri(&context.uri)?);
                                    send.send(ClientRequest::GetContext(context_id.clone()))?;
                                    utils::update_context(state, Some(context_id));
                                }
                                _ => {
                                    tracing::info!(
                                        "encountered not supported context type: {:?}",
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
                            tracing::info!("current playback does not have a playing context");
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
fn render_application(frame: &mut Frame, state: &SharedState, rect: Rect) {
    let rect = help::render_shortcut_help_popup(frame, state, rect);

    let (rect, is_active) = popup::render_popup(frame, state, rect);

    render_main_layout(is_active, frame, state, rect);
}

/// renders the application's main layout which consists of:
/// - a playback window on top
/// - a context window or a search window at bottom depending on the current UI's `PageState`
fn render_main_layout(is_active: bool, frame: &mut Frame, state: &SharedState, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(rect);
    window::render_playback_window(frame, state, chunks[0]);

    let ui = state.ui.lock();
    match ui.current_page() {
        PageState::Library => {
            drop(ui);
            window::render_library_window(is_active, frame, state, chunks[1]);
        }
        PageState::CurrentPlaying => {
            drop(ui);
            window::render_context_window(
                is_active,
                frame,
                state,
                chunks[1],
                "Context (Current Playing)",
            );
        }
        PageState::Browsing { .. } => {
            drop(ui);
            window::render_context_window(is_active, frame, state, chunks[1], "Context (Browsing)");
        }
        PageState::Recommendations { .. } => {
            drop(ui);
            window::render_recommendation_window(is_active, frame, state, chunks[1]);
        }
        PageState::Searching { .. } => {
            drop(ui);
            // make sure that the window state matches the current page state.
            // The mismatch can happen when going back to the search from another page
            window::render_search_window(is_active, frame, state, chunks[1]);
        }
    };
}

/// constructs a generic list widget
fn construct_list_widget<'a>(
    state: &SharedState,
    items: Vec<(String, bool)>,
    title: &str,
    is_active: bool,
    borders: Option<Borders>,
) -> List<'a> {
    let ui = state.ui.lock();
    let borders = borders.unwrap_or(Borders::ALL);

    List::new(
        items
            .into_iter()
            .map(|(s, is_active)| {
                ListItem::new(s).style(if is_active {
                    ui.theme.current_playing()
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

/// renders a track table widget
pub fn render_track_table_widget(
    frame: &mut Frame,
    rect: Rect,
    is_active: bool,
    state: &SharedState,
    tracks: Vec<&Track>,
) {
    let mut ui = state.ui.lock();

    // get the current playing track's URI to
    // highlight such track (if exists) in the track table
    let mut playing_track_uri = "".to_string();
    let mut active_desc = "";
    if let Some(ref playback) = state.player.read().playback {
        if let Some(rspotify_model::PlayableItem::Track(ref track)) = playback.item {
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
                (active_desc.to_string(), ui.theme.current_playing())
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

    let table = Table::new(rows)
        .header(
            Row::new(vec![
                Cell::from("#"),
                Cell::from("Track"),
                Cell::from("Artists"),
                Cell::from("Album"),
                Cell::from("Duration"),
            ])
            .style(ui.theme.table_header()),
        )
        .block(Block::default())
        .widths(&[
            Constraint::Length(4),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
        ])
        .highlight_style(ui.theme.selection_style(is_active));

    if let Some(state) = ui.window.track_table_state() {
        frame.render_stateful_widget(table, rect, state)
    }
}
