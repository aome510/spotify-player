use crate::{config, state::*, utils};
use anyhow::{Context as AnyhowContext, Result};
use tui::{layout::*, style::*, text::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod page;
mod popup;

/// run the application UI
pub fn run(state: SharedState) -> Result<()> {
    let mut terminal = init_ui().context("failed to initialize the application's UI")?;

    let ui_refresh_duration =
        std::time::Duration::from_millis(state.app_config.app_refresh_duration_in_ms);
    loop {
        if !state.ui.lock().is_running {
            clean_up(terminal).context("failed to clean up the application's UI resources")?;
            std::process::exit(0);
        }

        if let Err(err) = terminal.draw(|frame| {
            // set the background and foreground colors for the application
            let block = Block::default().style(state.ui.lock().theme.app_style());
            frame.render_widget(block, frame.size());

            if let Err(err) = render_application(frame, &state, frame.size()) {
                tracing::error!("Failed to render the application: {err:#}");
            }
        }) {
            tracing::error!("Failed to draw the application: {err:#}");
        }

        std::thread::sleep(ui_refresh_duration);
    }
}

// initialize the application's UI
fn init_ui() -> Result<Terminal> {
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
    Ok(terminal)
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
fn render_application(frame: &mut Frame, state: &SharedState, rect: Rect) -> Result<()> {
    let rect = popup::render_shortcut_help_popup(frame, state, rect);
    let (rect, is_active) = popup::render_popup(frame, state, rect);

    render_main_layout(is_active, frame, state, rect)?;
    Ok(())
}

/// renders the application's main layout
fn render_main_layout(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    rect: Rect,
) -> Result<()> {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)].as_ref())
        .split(rect);
    render_playback_window(frame, state, chunks[0])?;

    let page_type = state.ui.lock().current_page().page_type();
    match page_type {
        PageType::Library => page::render_library_page(is_active, frame, state, chunks[1]),
        PageType::Search => page::render_search_page(is_active, frame, state, chunks[1]),
        PageType::Context => page::render_context_page(is_active, frame, state, chunks[1]),
        PageType::Tracks => page::render_tracks_page(is_active, frame, state, chunks[1]),
        #[cfg(feature = "lyric-finder")]
        PageType::Lyric => page::render_lyric_page(is_active, frame, state, chunks[1]),
    }
}

/// constructs a generic list widget
pub fn construct_list_widget<'a>(
    theme: &config::Theme,
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
                    theme.current_playing()
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>(),
    )
    .highlight_style(theme.selection_style(is_active))
    .block(
        Block::default()
            .title(theme.block_title_with_style(title))
            .borders(borders),
    )
}

/// Renders a playback window showing information about the current playback, which includes
/// - track title, artists, album
/// - playback metadata (playing state, repeat state, shuffle state, volume, device, etc)
fn render_playback_window(frame: &mut Frame, state: &SharedState, rect: Rect) -> Result<()> {
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
                        "{} {} • {}",
                        if !playback.is_playing { "⏸" } else { "▶" },
                        track.name,
                        utils::map_join(&track.artists, |a| &a.name, ", ")
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
                .block(Block::default());
            let progress = std::cmp::min(
                player
                    .playback_progress()
                    .context("playback should exist")?,
                track.duration,
            );
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

            let metadata_rect = {
                #[cfg(feature = "cover")]
                {
                    // Render the track's cover image if `cover` feature is enabled
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(10), Constraint::Min(0)].as_ref())
                        .split(chunks[0]);

                    if let Some(url) = utils::get_track_album_image_url(track) {
                        if let Some(image) = state.data.read().caches.images.peek(url) {
                            let url = Some(url.to_string());

                            // Try to not render the same image multiple times.
                            // Rendering images on the terminal is expensive...
                            if ui.last_rendered_cover_image_url != url {
                                viuer::print(
                                    image,
                                    &viuer::Config {
                                        x: chunks[0].x,
                                        y: chunks[0].y as i16,
                                        width: Some(chunks[0].width as u32),
                                        height: Some(chunks[0].height as u32),
                                        ..Default::default()
                                    },
                                )?;

                                ui.last_rendered_cover_image_url = url;
                            }
                        }
                    }

                    chunks[1]
                }

                #[cfg(not(feature = "cover"))]
                {
                    chunks[0]
                }
            };
            let progress_bar_rect = chunks[1];

            ui.playback_progress_bar_rect = progress_bar_rect;

            frame.render_widget(playback_desc, metadata_rect);
            frame.render_widget(progress_bar, progress_bar_rect);
        }
    } else {
        frame.render_widget(
            Paragraph::new(
                "No playback found. \
                 Please make sure there is a running Spotify client and try to connect to it using the `SwitchDevice` command."
            )
            .wrap(Wrap { trim: true })
            .block(Block::default()),
            chunks[0],
        );
    };

    Ok(())
}
