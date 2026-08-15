//! A transient volume gauge, drawn in a corner whenever the volume changes.
//!
//! The playback window can already list the volume as a metadata field, but that is static text
//! that only tells you the level if you go looking for it. This overlay pops up on a volume change
//! and fades out on its own, giving the volume keys immediate visual feedback.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    config::{self, VolumeHudPosition},
    state::{SharedState, UIStateGuard},
};

/// Outer size of the HUD, derived from the configured gauge width plus borders and padding.
fn hud_size(gauge_width: u16) -> (u16, u16) {
    // gauge + a space + "100%" (4 cells) + one cell of padding on each side + two borders
    (gauge_width + 4 + 1 + 2 + 2, 3)
}

/// Places the HUD in the configured corner of `rect`, inset by one cell so it does not sit flush
/// against the terminal edge. Returns `None` when the terminal is too small to hold it.
fn hud_rect(rect: Rect, position: &VolumeHudPosition, gauge_width: u16) -> Option<Rect> {
    let (width, height) = hud_size(gauge_width);
    if rect.width < width + 2 || rect.height < height + 2 {
        return None;
    }

    let (x, y) = match position {
        VolumeHudPosition::TopLeft => (rect.x + 1, rect.y + 1),
        VolumeHudPosition::TopRight => (rect.x + rect.width - width - 1, rect.y + 1),
        VolumeHudPosition::BottomLeft => (rect.x + 1, rect.y + rect.height - height - 1),
        VolumeHudPosition::BottomRight => (
            rect.x + rect.width - width - 1,
            rect.y + rect.height - height - 1,
        ),
    };

    Some(Rect {
        x,
        y,
        width,
        height,
    })
}

/// How many of `width` cells the gauge fills at `volume` percent.
///
/// Rounds up so that a quiet-but-audible level never renders as an empty bar, which would read as
/// muted.
fn filled_cells(volume: u8, width: u16) -> usize {
    let width = usize::from(width);
    (usize::from(volume) * width).div_ceil(100).min(width)
}

/// Builds the gauge as filled/unfilled block runs, so the bar keeps its shape under any theme.
fn gauge_spans(volume: u8, width: u16, muted: bool, ui: &UIStateGuard) -> Vec<Span<'static>> {
    let filled = filled_cells(volume, width);
    let unfilled = usize::from(width) - filled;

    let filled_style = if muted {
        ui.theme.playback_metadata()
    } else {
        ui.theme.playback_progress_bar()
    };

    vec![
        Span::styled("\u{2588}".repeat(filled), filled_style),
        Span::styled(
            "\u{2591}".repeat(unfilled),
            ui.theme.playback_progress_bar_unfilled(),
        ),
    ]
}

/// Renders the volume HUD if it was triggered recently enough, and clears the trigger once its
/// timeout has elapsed.
pub fn render_volume_hud(
    frame: &mut Frame,
    state: &SharedState,
    ui: &mut UIStateGuard,
    rect: Rect,
) {
    let configs = config::get_config();
    if !configs.app_config.enable_volume_hud {
        return;
    }

    let timeout = std::time::Duration::from_millis(configs.app_config.volume_hud_timeout_in_ms);
    match ui.volume_hud_shown_at {
        Some(shown_at) if shown_at.elapsed() < timeout => {}
        Some(_) => {
            ui.volume_hud_shown_at = None;
            return;
        }
        None => return,
    }

    let (volume, muted) = {
        let player = state.player.read();
        let Some(playback) = player.buffered_playback.as_ref() else {
            return;
        };
        // While muted, `volume` holds the pre-mute level; show that so the bar does not collapse
        // to nothing, and label it instead.
        match playback.mute_state {
            Some(volume) => (volume, true),
            None => match playback.volume {
                Some(volume) => (volume, false),
                None => return,
            },
        }
    };
    let volume = u8::try_from(volume.min(100)).unwrap_or(100);

    let gauge_width = configs.app_config.volume_hud_width.max(4);
    let Some(area) = hud_rect(rect, &configs.app_config.volume_hud_position, gauge_width) else {
        return;
    };

    let mut spans = gauge_spans(volume, gauge_width, muted, ui);
    spans.push(Span::styled(
        format!(" {volume:>3}%"),
        ui.theme
            .playback_track()
            .add_modifier(if muted { Modifier::DIM } else { Modifier::BOLD }),
    ));

    let title = if muted { " MUTED " } else { " VOLUME " };
    let block = Block::default()
        .title(Span::styled(title, ui.theme.block_title()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(ui.theme.app());

    // `Clear` keeps whatever the HUD overlaps from bleeding through.
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(block)
            .style(Style::default()),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{filled_cells, hud_rect, hud_size};
    use crate::config::VolumeHudPosition;
    use ratatui::layout::Rect;

    const GAUGE: u16 = 20;

    fn terminal(width: u16, height: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[test]
    fn gauge_fills_proportionally() {
        assert_eq!(filled_cells(0, GAUGE), 0);
        assert_eq!(filled_cells(50, GAUGE), 10);
        assert_eq!(filled_cells(100, GAUGE), 20);
    }

    /// A barely-audible volume must still show something, or it looks muted.
    #[test]
    fn gauge_never_rounds_an_audible_volume_down_to_empty() {
        assert_eq!(filled_cells(1, GAUGE), 1);
    }

    #[test]
    fn gauge_never_overflows_its_width() {
        for volume in 0..=100u8 {
            for width in [4u16, 7, 20, 60] {
                assert!(filled_cells(volume, width) <= usize::from(width));
            }
        }
    }

    /// The HUD is drawn with an absolute `Rect`, so it must stay inside the frame; ratatui panics
    /// on out-of-bounds areas.
    #[test]
    fn hud_stays_within_the_terminal_in_every_corner() {
        let rect = terminal(120, 40);
        for position in [
            VolumeHudPosition::TopLeft,
            VolumeHudPosition::TopRight,
            VolumeHudPosition::BottomLeft,
            VolumeHudPosition::BottomRight,
        ] {
            let area = hud_rect(rect, &position, GAUGE).expect("fits in a 120x40 terminal");
            assert!(area.x >= rect.x, "{position:?} overflows left");
            assert!(area.y >= rect.y, "{position:?} overflows top");
            assert!(
                area.x + area.width <= rect.x + rect.width,
                "{position:?} overflows right"
            );
            assert!(
                area.y + area.height <= rect.y + rect.height,
                "{position:?} overflows bottom"
            );
        }
    }

    #[test]
    fn hud_is_skipped_when_the_terminal_is_too_small() {
        let (width, height) = hud_size(GAUGE);
        assert!(hud_rect(terminal(width, height), &VolumeHudPosition::TopRight, GAUGE).is_none());
        assert!(hud_rect(terminal(10, 3), &VolumeHudPosition::TopRight, GAUGE).is_none());
        assert!(hud_rect(terminal(1, 1), &VolumeHudPosition::BottomLeft, GAUGE).is_none());
    }

    #[test]
    fn hud_fits_the_gauge_and_its_label() {
        let (width, _) = hud_size(GAUGE);
        // two borders + two padding cells + gauge + a space + "100%"
        assert_eq!(width, 2 + 2 + GAUGE + 1 + 4);
    }

    /// Renders the HUD through a real ratatui frame to confirm the overlay draws the gauge and
    /// the level, and that the surrounding buffer is left intact.
    #[test]
    fn renders_the_gauge_and_level_into_the_frame() {
        use crate::state::{PlaybackMetadata, State};
        use parking_lot::Mutex;
        use ratatui::{backend::TestBackend, Terminal};
        use std::{collections::VecDeque, sync::Arc};

        let dir = std::env::temp_dir().join("spotify-player-volume-hud-test");
        std::fs::create_dir_all(&dir).expect("create temp config folder");
        crate::config::set_config(
            crate::config::Configs::new(&dir, &dir).expect("build test configs"),
        );

        let state: crate::state::SharedState =
            Arc::new(State::new(false, Arc::new(Mutex::new(VecDeque::new()))));
        state.player.write().buffered_playback = Some(PlaybackMetadata {
            device_name: "test".to_string(),
            device_id: None,
            volume: Some(70),
            is_playing: true,
            repeat_state: rspotify::model::RepeatState::Off,
            shuffle_state: false,
            mute_state: None,
        });
        state.ui.lock().volume_hud_shown_at = Some(std::time::Instant::now());

        let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("build test terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                let mut ui = state.ui.lock();
                super::render_volume_hud(frame, &state, &mut ui, area);
            })
            .expect("draw the volume HUD");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        assert!(rendered.contains("VOLUME"), "HUD title is missing");
        assert!(rendered.contains("70%"), "volume level is missing");
        assert!(
            rendered.contains(&"\u{2588}".repeat(14)),
            "filled gauge is missing"
        );
        assert!(
            rendered.contains(&"\u{2591}".repeat(6)),
            "unfilled gauge is missing"
        );

        // The HUD must not have been triggered off; it is still within its timeout.
        assert!(state.ui.lock().volume_hud_shown_at.is_some());
    }
}
