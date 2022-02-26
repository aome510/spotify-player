use crate::{event::ClientRequest, state::*, utils};
use anyhow::Result;
use tokio::sync::mpsc;
use tui::{layout::*, style::*, text::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod page;
mod popup;

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
    let rect = popup::render_shortcut_help_popup(frame, state, rect);

    let (rect, is_active) = popup::render_popup(frame, state, rect);

    render_main_layout(is_active, frame, state, rect);
}

/// renders the application's main layout
fn render_main_layout(is_active: bool, frame: &mut Frame, state: &SharedState, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)].as_ref())
        .split(rect);
    render_playback_window(frame, state, chunks[0]);

    let mut ui = state.ui.lock();
    match ui.current_page_mut() {
        // PageState::Library {  } => {
        //     page::render_library_window(is_active, frame, state, chunks[1]);
        // }
        // PageState::Context { .. } => {
        //     page::render_context_window(is_active, frame, state, chunks[1]);
        // }
        PageState::Search {
            current_query,
            input,
            state: page_ui_state,
        } => {
            page::render_search_page(
                is_active,
                frame,
                chunks[1],
                state,
                input,
                current_query,
                page_ui_state,
            );
        }
        // TODO: handle this!
        _ => {}
    };
}

/// constructs a generic list widget
pub fn construct_list_widget<'a>(
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

/// Renders a playback window showing information about the current playback, which includes
/// - track title, artists, album
/// - playback metadata (playing state, repeat state, shuffle state, volume, device, etc)
fn render_playback_window(frame: &mut Frame, state: &SharedState, rect: Rect) {
    let mut ui = state.ui.lock();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .margin(1)
        .split(rect);

    let block = Block::default()
        .title(ui.theme.block_title_with_style("Playback"))
        .borders(Borders::ALL);
    frame.render_widget(block, rect);

    let player = state.player.read();
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
