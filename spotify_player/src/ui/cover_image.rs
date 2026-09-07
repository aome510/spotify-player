use std::io::Write;

use anyhow::{Context, Result};
use base64::Engine;
use image::DynamicImage;
use ratatui::{buffer::CellDiffOption, layout::Rect, Frame};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::Protocol,
    Image, Resize,
};

/// A cover image prepared for a fixed render area. Construct it once per `(url, area)` and reuse
/// it across frames.
pub enum CoverImage {
    /// Rendered through `ratatui-image` as a widget (kitty / sixel / halfblocks).
    Widget(Box<Protocol>),
    /// A cursor-anchored iTerm2 inline-image escape, written directly to the terminal.
    /// `drawn` tracks whether it has been emitted yet — being grid-anchored, it only needs
    /// to be emitted once.
    Iterm2 { escape: String, drawn: bool },
}

impl CoverImage {
    /// Prepare `img` for rendering into `area` using the protocol selected by `picker`.
    pub fn new(picker: &Picker, img: &DynamicImage, area: Rect) -> Result<Self> {
        if picker.protocol_type() == ProtocolType::Iterm2 {
            Ok(Self::Iterm2 {
                escape: encode_iterm2(img, area)?,
                drawn: false,
            })
        } else {
            let protocol = picker
                .new_protocol(img.clone(), area.into(), Resize::Fit(None))
                .context("encode cover image protocol")?;
            Ok(Self::Widget(Box::new(protocol)))
        }
    }

    /// Render the cover image into `area`.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self {
            Self::Widget(protocol) => frame.render_widget(Image::new(protocol.as_ref()), area),
            Self::Iterm2 { escape, drawn } => {
                reserve_area(frame, area);
                if !*drawn {
                    if let Err(err) = write_iterm2(escape, area) {
                        tracing::error!("Failed to draw iTerm2 cover image: {err:#}");
                    } else {
                        *drawn = true;
                    }
                }
            }
        }
    }
}

/// Mark every cell in `area` as skipped so `ratatui`'s renderer leaves it untouched.
fn reserve_area(frame: &mut Frame, area: Rect) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
                cell.set_diff_option(CellDiffOption::Skip);
            }
        }
    }
}

/// Encode `img` as a cursor-anchored, cell-sized iTerm2 inline-image escape sequence.
fn encode_iterm2(img: &DynamicImage, area: Rect) -> Result<String> {
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .context("encode cover image to PNG")?;
    let data = base64::engine::general_purpose::STANDARD.encode(&png);
    Ok(format!(
        "\x1b]1337;File=inline=1;preserveAspectRatio=1;size={};width={};height={}:{data}\x07",
        png.len(),
        area.width,
        area.height,
    ))
}

/// Write a prepared iTerm2 image escape at `area`'s top-left, erasing the cell box first (so
/// letterboxing doesn't reveal stale content) and restoring the cursor afterwards so
/// `ratatui`'s own rendering is unaffected.
fn write_iterm2(escape: &str, area: Rect) -> std::io::Result<()> {
    // `area` is always a sub-rectangle of the screen, so the cursor-anchored image fits and
    // does not scroll the alternate screen.
    let mut out = std::io::stdout().lock();
    out.write_all(b"\x1b7")?; // DEC save cursor
    for row in area.top()..area.bottom() {
        // move to the start of the row (1-based) and erase `width` cells
        write!(out, "\x1b[{};{}H\x1b[{}X", row + 1, area.x + 1, area.width)?;
    }
    // position at the image origin and draw it
    write!(out, "\x1b[{};{}H{escape}", area.y + 1, area.x + 1)?;
    out.write_all(b"\x1b8")?; // DEC restore cursor
    out.flush()
}
