use crate::{event::ClientRequest, state::*, utils};
use anyhow::Result;
use tokio::sync::mpsc;
use tui::{layout::*, style::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod help;
mod popup;
mod window;

/// starts the application UI rendering function(s)
pub fn start_ui(state: SharedState, client_pub: mpsc::Sender<ClientRequest>) -> Result<()> {
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

        if let Err(err) = handle_page_state_change(&state, &client_pub) {
            tracing::warn!("failed to handle page state change events: {}", err);
        }

        if let Err(err) = terminal.draw(|frame| {
            // set the background and foreground colors for the application
            let block = Block::default().style(state.ui.lock().theme.app_style());
            frame.render_widget(block, frame.size());

            render_application(frame, &state, frame.size());
        }) {
            tracing::warn!("failed to draw the application: {}", err);
        }

        std::thread::sleep(ui_refresh_duration);
    }
}

/// checks the current UI page state for new changes
/// to update the UI window state and other states accordingly
fn handle_page_state_change(
    state: &SharedState,
    client_pub: &mpsc::Sender<ClientRequest>,
) -> Result<()> {
    let mut ui = state.ui.lock();

    match ui.current_page() {
        PageState::Library => match ui.window {
            WindowState::Library { .. } => {}
            _ => {
                client_pub.blocking_send(ClientRequest::GetUserPlaylists)?;
                client_pub.blocking_send(ClientRequest::GetUserSavedAlbums)?;
                client_pub.blocking_send(ClientRequest::GetUserFollowedArtists)?;

                ui.window = WindowState::Library {
                    playlist_list: utils::new_list_state(),
                    saved_album_list: utils::new_list_state(),
                    followed_artist_list: utils::new_list_state(),
                    focus: LibraryFocusState::Playlists,
                }
            }
        },
        PageState::Search { current_query, .. } => match ui.window {
            WindowState::Search { .. } => {}
            _ => {
                client_pub.blocking_send(ClientRequest::Search(current_query.clone()))?;
                ui.window = WindowState::new_search_state();
            }
        },
        PageState::Recommendations(seed) => match ui.window {
            WindowState::Recommendations { .. } => {}
            _ => {
                client_pub.blocking_send(ClientRequest::GetRecommendations(seed.clone()))?;
                ui.window = WindowState::Recommendations {
                    track_table: utils::new_table_state(),
                };
            }
        },
        PageState::Context(context_id, context_type) => {
            let expected_context_id = match context_type {
                ContextPageType::Browsing(context_id) => Some(context_id.clone()),
                ContextPageType::CurrentPlaying => state.player.read().playing_context_id(),
            };

            if *context_id != expected_context_id {
                tracing::info!(
                    "update current page's context_id to {:?}",
                    expected_context_id
                );

                if let Some(ref id) = expected_context_id {
                    client_pub.blocking_send(ClientRequest::GetContext(id.clone()))?;

                    ui.window = match id {
                        ContextId::Artist { .. } => WindowState::Artist {
                            top_track_table: utils::new_table_state(),
                            album_list: utils::new_list_state(),
                            related_artist_list: utils::new_list_state(),
                            focus: ArtistFocusState::TopTracks,
                        },
                        ContextId::Album { .. } => WindowState::Album {
                            track_table: utils::new_table_state(),
                        },
                        ContextId::Playlist { .. } => WindowState::Playlist {
                            track_table: utils::new_table_state(),
                        },
                    };
                }

                // update the current context page's `context_id`
                if let PageState::Context(ref mut context_id, _) = ui.current_page_mut() {
                    *context_id = expected_context_id;
                }
            }
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
        PageState::Context(_, context_type) => {
            let title = match context_type {
                ContextPageType::CurrentPlaying => "Context (Current Playing)",
                ContextPageType::Browsing(_) => "Context (Browsing)",
            };
            drop(ui);
            window::render_context_window(is_active, frame, state, chunks[1], title);
        }
        PageState::Recommendations { .. } => {
            drop(ui);
            window::render_recommendation_window(is_active, frame, state, chunks[1]);
        }
        PageState::Search { .. } => {
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
            playing_track_uri = track.id.as_ref().map(|id| id.uri()).unwrap_or_default();

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
