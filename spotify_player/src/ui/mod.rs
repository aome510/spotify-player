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
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, LineGauge, List, ListItem, ListState, Paragraph,
        Row, Table, TableState, Wrap,
    },
    Frame,
};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[cfg(feature = "image")]
use crate::state::ImageRenderInfo;

type RatatuiTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>;

const RAW_MODE_ACTIVE: u8 = 1 << 0;
const ALTERNATE_SCREEN_ACTIVE: u8 = 1 << 1;
const MOUSE_CAPTURE_ACTIVE: u8 = 1 << 2;

static ACTIVE_TERMINAL_STATE: AtomicU8 = AtomicU8::new(0);
static APPLICATION_PANICKED: AtomicBool = AtomicBool::new(false);

struct RestoreOnDrop<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> RestoreOnDrop<F> {
    fn new(restore: F) -> Self {
        Self(Some(restore))
    }

    fn disarm(&mut self) {
        self.0.take();
    }
}

impl<F: FnOnce()> Drop for RestoreOnDrop<F> {
    fn drop(&mut self) {
        if let Some(restore) = self.0.take() {
            restore();
        }
    }
}

pub(crate) struct TerminalSession {
    terminal: RatatuiTerminal,
    restore_guard: RestoreOnDrop<fn()>,
}

impl TerminalSession {
    fn restore(mut self) -> Result<()> {
        let result = restore_active_terminal();
        self.restore_guard.disarm();
        result
    }
}

impl std::ops::Deref for TerminalSession {
    type Target = RatatuiTerminal;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl std::ops::DerefMut for TerminalSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

#[cfg(feature = "image")]
pub mod cover_image;
mod page;
mod playback;
mod popup;
pub mod single_line_input;
#[cfg(feature = "streaming")]
pub mod streaming;
pub mod utils;

/// Run the application UI
pub(crate) fn run(state: &SharedState, mut terminal: TerminalSession) -> Result<()> {
    let ui_refresh_duration = std::time::Duration::from_millis(
        config::get_config().app_config.app_refresh_duration_in_ms,
    );
    let mut last_terminal_size = None;

    loop {
        if application_panicked() {
            anyhow::bail!("another application thread panicked");
        }

        {
            let mut ui = state.ui.lock();
            if !ui.is_running {
                break;
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

    terminal.restore().context("clean up UI resources")
}

pub(crate) fn init_terminal() -> Result<TerminalSession> {
    let mut stdout = std::io::stdout();
    crossterm::terminal::enable_raw_mode()?;
    ACTIVE_TERMINAL_STATE.fetch_or(RAW_MODE_ACTIVE, Ordering::SeqCst);
    let restore_guard = RestoreOnDrop::new(restore_active_terminal_best_effort as fn());

    ACTIVE_TERMINAL_STATE.fetch_or(ALTERNATE_SCREEN_ACTIVE, Ordering::SeqCst);
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    ACTIVE_TERMINAL_STATE.fetch_or(MOUSE_CAPTURE_ACTIVE, Ordering::SeqCst);
    crossterm::execute!(stdout, crossterm::event::EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;
    Ok(TerminalSession {
        terminal,
        restore_guard,
    })
}

#[cfg(feature = "image")]
pub fn init_image_picker(state: &SharedState) -> Result<()> {
    let mut ui = state.ui.lock();
    crossterm::terminal::enable_raw_mode()?;
    ACTIVE_TERMINAL_STATE.fetch_or(RAW_MODE_ACTIVE, Ordering::SeqCst);
    let mut restore_guard = RestoreOnDrop::new(restore_active_terminal_best_effort as fn());
    ui.picker = match ratatui_image::picker::Picker::from_query_stdio() {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!("Failed to initialize query_stdio picker, error: {err:#}");
            ratatui_image::picker::Picker::halfblocks()
        }
    };
    crossterm::terminal::disable_raw_mode()?;
    ACTIVE_TERMINAL_STATE.fetch_and(!RAW_MODE_ACTIVE, Ordering::SeqCst);
    restore_guard.disarm();

    // ratatui_image might detect the wrong protocol for iTerm2, so override it to the native iTerm2 protocol if detected
    // https://github.com/ratatui/ratatui-image/issues/158
    if is_iterm2() && ui.picker.protocol_type() != ratatui_image::picker::ProtocolType::Iterm2 {
        ui.picker
            .set_protocol_type(ratatui_image::picker::ProtocolType::Iterm2);
        tracing::info!("Detected iTerm2; overriding image protocol to native iTerm2");
    }
    tracing::info!("Image protocol: {:?}", ui.picker.protocol_type());
    Ok(())
}

/// Whether the application is running inside iTerm2.
///
/// iTerm2 sets `TERM_PROGRAM=iTerm.app` locally and forwards `LC_TERMINAL=iTerm2`
/// over SSH, so checking both covers the common cases.
#[cfg(feature = "image")]
fn is_iterm2() -> bool {
    std::env::var("TERM_PROGRAM").is_ok_and(|v| v == "iTerm.app")
        || std::env::var("LC_TERMINAL").is_ok_and(|v| v.eq_ignore_ascii_case("iTerm2"))
}

fn record_first_error(first_error: &mut Option<std::io::Error>, result: std::io::Result<()>) {
    if let Err(err) = result {
        first_error.get_or_insert(err);
    }
}

fn restore_terminal_state_with<W, DisableRawMode>(
    state: u8,
    output: &mut W,
    disable_raw_mode: DisableRawMode,
) -> std::io::Result<()>
where
    W: std::io::Write,
    DisableRawMode: FnOnce() -> std::io::Result<()>,
{
    let mut first_error = None;

    if state & RAW_MODE_ACTIVE != 0 {
        record_first_error(&mut first_error, disable_raw_mode());
    }
    if state & ALTERNATE_SCREEN_ACTIVE != 0 {
        record_first_error(
            &mut first_error,
            crossterm::execute!(output, crossterm::terminal::LeaveAlternateScreen),
        );
    }
    if state & MOUSE_CAPTURE_ACTIVE != 0 {
        record_first_error(
            &mut first_error,
            crossterm::execute!(output, crossterm::event::DisableMouseCapture),
        );
    }
    if state != 0 {
        record_first_error(
            &mut first_error,
            crossterm::execute!(output, crossterm::cursor::Show),
        );
    }

    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn restore_terminal_state(state: u8) -> Result<()> {
    restore_terminal_state_with(
        state,
        &mut std::io::stdout(),
        crossterm::terminal::disable_raw_mode,
    )?;
    Ok(())
}

fn restore_active_terminal() -> Result<()> {
    let state = ACTIVE_TERMINAL_STATE.swap(0, Ordering::SeqCst);
    restore_terminal_state(state)
}

fn restore_active_terminal_best_effort() {
    if let Err(err) = restore_active_terminal() {
        eprintln!("Failed to restore terminal state: {err:#}");
    }
}

/// Restore terminal state immediately after a panic and signal the UI loop to stop.
///
/// The active-state flags remain set so the terminal session guard repeats the cleanup after the
/// UI thread has stopped drawing.
pub(crate) fn handle_panic() {
    APPLICATION_PANICKED.store(true, Ordering::SeqCst);
    let state = ACTIVE_TERMINAL_STATE.load(Ordering::SeqCst);
    if let Err(err) = restore_terminal_state(state) {
        eprintln!("Failed to restore terminal state after panic: {err:#}");
    }
}

pub(crate) fn application_panicked() -> bool {
    APPLICATION_PANICKED.load(Ordering::SeqCst)
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
        PageType::Logs => page::render_logs_page(frame, state, ui, rect),
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

#[cfg(test)]
mod tests {
    use super::{
        restore_terminal_state_with, RestoreOnDrop, ALTERNATE_SCREEN_ACTIVE, MOUSE_CAPTURE_ACTIVE,
        RAW_MODE_ACTIVE,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn contains_bytes(output: &[u8], expected: &[u8]) -> bool {
        output
            .windows(expected.len())
            .any(|window| window == expected)
    }

    #[test]
    fn terminal_restore_resets_every_enabled_mode() {
        let raw_mode_disabled = AtomicBool::new(false);
        let mut output = Vec::new();

        restore_terminal_state_with(
            RAW_MODE_ACTIVE | ALTERNATE_SCREEN_ACTIVE | MOUSE_CAPTURE_ACTIVE,
            &mut output,
            || {
                raw_mode_disabled.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();

        assert!(raw_mode_disabled.load(Ordering::SeqCst));
        assert!(contains_bytes(&output, b"\x1b[?1049l"));
        assert!(contains_bytes(&output, b"\x1b[?1000l"));
        assert!(contains_bytes(&output, b"\x1b[?25h"));
    }

    #[test]
    fn terminal_restore_guard_runs_during_unwind() {
        let restored = Arc::new(AtomicBool::new(false));
        let restored_by_guard = restored.clone();

        let result = std::panic::catch_unwind(move || {
            let _guard = RestoreOnDrop::new(move || {
                restored_by_guard.store(true, Ordering::SeqCst);
            });
            panic!("injected terminal failure");
        });

        assert!(result.is_err());
        assert!(restored.load(Ordering::SeqCst));
    }
}
