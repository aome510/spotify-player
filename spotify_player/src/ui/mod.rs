use crate::{config, state::*};
use anyhow::{Context as AnyhowContext, Result};
use tui::{layout::*, style::*, text::*, widgets::*};

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;
type Frame<'a> = tui::Frame<'a, tui::backend::CrosstermBackend<std::io::Stdout>>;

mod page;
mod playback;
mod popup;
mod utils;

/// run the application UI
pub fn run(state: SharedState) -> Result<()> {
    #[cfg(feature = "image")]
    {
        // initialize viuer supports for kitty and iterm2
        viuer::get_kitty_support();
        viuer::is_iterm_supported();
        #[cfg(feature = "sixel")]
        viuer::is_sixel_supported();
    }

    let mut terminal = init_ui().context("failed to initialize the application's UI")?;

    let ui_refresh_duration =
        std::time::Duration::from_millis(state.app_config.app_refresh_duration_in_ms);
    loop {
        {
            let mut ui = state.ui.lock();
            if !ui.is_running {
                clean_up(terminal).context("failed to clean up the application's UI resources")?;
                std::process::exit(0);
            }

            if let Err(err) = terminal.draw(|frame| {
                // set the background and foreground colors for the application
                let block = Block::default().style(ui.theme.app_style());
                frame.render_widget(block, frame.size());

                if let Err(err) = render_application(frame, &state, &mut ui, frame.size()) {
                    tracing::error!("Failed to render the application: {err:#}");
                }
            }) {
                tracing::error!("Failed to draw the application: {err:#}");
            }
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
fn render_application(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    // playback window is the window that is always displayed on the screen,
    // hence it's rendered first
    let (playback_rect, rect) = playback::split_rect_for_playback_window(rect, state);
    playback::render_playback_window(frame, state, ui, playback_rect)?;

    let rect = popup::render_shortcut_help_popup(frame, state, ui, rect);
    let (rect, is_active) = popup::render_popup(frame, state, ui, rect);

    render_main_layout(is_active, frame, state, ui, rect)?;
    Ok(())
}

/// renders the application's main layout
fn render_main_layout(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) -> Result<()> {
    let page_type = ui.current_page().page_type();
    match page_type {
        PageType::Library => page::render_library_page(is_active, frame, state, ui, rect),
        PageType::Search => page::render_search_page(is_active, frame, state, ui, rect),
        PageType::Context => page::render_context_page(is_active, frame, state, ui, rect),
        PageType::Browse => page::render_browse_page(is_active, frame, state, ui, rect),
        #[cfg(feature = "lyric-finder")]
        PageType::Lyric => page::render_lyric_page(is_active, frame, state, ui, rect),
    }
}
