use crate::{
    config,
    state::{
        Album, Artist, ArtistFocusState, BrowsePageUIState, Context, ContextPageUIState,
        DataReadGuard, Id, LibraryFocusState, MutableWindowState, PageState, PageType,
        PlaybackMetadata, PlaylistCreateCurrentField, PlaylistFolderItem, PlaylistPopupAction,
        PopupState, SearchFocusState, SharedState, Track, UIStateGuard,
    },
};
use anyhow::{Context as AnyhowContext, Result};
use tui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, LineGauge, List, ListItem, ListState, Paragraph,
        Row, Table, TableState, Wrap,
    },
    Frame,
};

#[cfg(feature = "image")]
use crate::state::ImageRenderInfo;

type Terminal = tui::Terminal<tui::backend::CrosstermBackend<std::io::Stdout>>;

mod page;
mod playback;
mod popup;
pub mod single_line_input;
mod utils;

/// Run the application UI
pub fn run(state: &SharedState) -> Result<()> {
    let mut terminal = init_ui().context("failed to initialize the application's UI")?;

    let ui_refresh_duration = std::time::Duration::from_millis(
        config::get_config().app_config.app_refresh_duration_in_ms,
    );
    let mut last_terminal_size = None;

    loop {
        {
            let mut ui = state.ui.lock();
            if !ui.is_running {
                clean_up(terminal).context("clean up UI resources")?;
                std::process::exit(0);
            }

            let terminal_size = terminal.size()?;
            if Some(terminal_size) != last_terminal_size {
                last_terminal_size = Some(terminal_size);
                #[cfg(feature = "image")]
                {
                    // redraw the cover image when the terminal's size changes
                    ui.last_cover_image_render_info = ImageRenderInfo::default();
                }
            }

            if let Err(err) = terminal.draw(|frame| {
                // set the background and foreground colors for the application
                let rect = frame.area();
                let block = Block::default().style(ui.theme.app());
                frame.render_widget(block, rect);

                render_application(frame, state, &mut ui, rect);
            }) {
                tracing::error!("Failed to render the application: {err:#}");
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

/// Clean up UI resources before quitting the application
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

/// Render the application
fn render_application(frame: &mut Frame, state: &SharedState, ui: &mut UIStateGuard, rect: Rect) {
    // rendering order: playback window -> shortcut help popup -> other popups -> main layout

    // render playback window before other popups and windows to ensure nothing is rendered on top
    // of the playback window, which is to avoid "duplicated images" issue
    // See: https://github.com/aome510/spotify-player/issues/498
    let rect = playback::render_playback_window(frame, state, ui, rect);

    let rect = popup::render_shortcut_help_popup(frame, ui, rect);

    let (rect, is_active) = popup::render_popup(frame, state, ui, rect);

    render_main_layout(is_active, frame, state, ui, rect);
}

/// Render the application's main layout
fn render_main_layout(
    is_active: bool,
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    let page_type = ui.current_page().page_type();
    match page_type {
        PageType::Library => page::render_library_page(is_active, frame, state, ui, rect),
        PageType::Search => page::render_search_page(is_active, frame, state, ui, rect),
        PageType::Context => page::render_context_page(is_active, frame, state, ui, rect),
        PageType::Browse => page::render_browse_page(is_active, frame, state, ui, rect),
        PageType::Lyrics => page::render_lyrics_page(is_active, frame, state, ui, rect),
        PageType::Queue => page::render_queue_page(frame, state, ui, rect),
        PageType::CommandHelp => page::render_commands_help_page(frame, ui, rect),
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Orientation {
    Vertical,
    #[default]
    Horizontal,
}

impl Orientation {
    /// Construct screen orientation based on the terminal's size
    pub fn from_size(columns: u16, rows: u16) -> Self {
        let ratio = f64::from(columns) / f64::from(rows);

        // a larger ratio has to be used since terminal cells aren't square
        if ratio > 2.3 {
            Self::Horizontal
        } else {
            Self::Vertical
        }
    }

    pub fn layout<I>(self, constraints: I) -> Layout
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        match self {
            Self::Vertical => Layout::vertical(constraints),
            Self::Horizontal => Layout::horizontal(constraints),
        }
    }
}
