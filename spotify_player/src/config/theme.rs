use config_parser2::*;
use serde::Deserialize;
use tui::{style::*, text::*};

#[derive(Debug, Deserialize, ConfigParse)]
/// Application theme configurations
pub struct ThemeConfig {
    pub background: ColorConfig,
    pub foreground: ColorConfig,
    pub selection: ColorConfig,
    pub comment: ColorConfig,
    pub cyan: ColorConfig,
    pub green: ColorConfig,
    pub orange: ColorConfig,
    pub pink: ColorConfig,
    pub purple: ColorConfig,
    pub red: ColorConfig,
    pub yellow: ColorConfig,
}

#[derive(Debug, Deserialize)]
pub struct ColorConfig {
    pub color: Color,
}

impl ColorConfig {
    pub fn _from_color(color: Color) -> Self {
        Self { color }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            color: Color::Rgb(r, g, b),
        }
    }
}

config_parser_impl!(ColorConfig);

impl ThemeConfig {
    pub fn app_style(&self) -> Style {
        Style::default()
            .bg(self.background.color)
            .fg(self.foreground.color)
    }

    pub fn text_desc_style(&self) -> Style {
        Style::default().fg(self.cyan.color)
    }

    pub fn gauge_style(&self) -> Style {
        Style::default()
            .fg(self.selection.color)
            .bg(self.green.color)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn _comment_style(&self) -> Style {
        Style::default().fg(self.comment.color)
    }

    pub fn current_playing_style(&self) -> Style {
        Style::default().fg(self.green.color)
    }

    pub fn selection_style(&self) -> Style {
        Style::default().bg(self.selection.color)
    }

    pub fn table_header_style(&self) -> Style {
        Style::default().fg(self.purple.color)
    }

    pub fn block_title_with_style<'a, S>(&self, content: S) -> Span<'a>
    where
        S: Into<String>,
    {
        Span::styled(
            content.into(),
            Style::default()
                .fg(self.pink.color)
                .add_modifier(Modifier::BOLD),
        )
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            // dracula theme's color palette
            // (https://github.com/dracula/dracula-theme#color-palette)
            background: ColorConfig::from_rgb(40, 42, 54),
            foreground: ColorConfig::from_rgb(248, 245, 242),
            selection: ColorConfig::from_rgb(68, 71, 90),
            comment: ColorConfig::from_rgb(98, 114, 164),
            cyan: ColorConfig::from_rgb(139, 233, 253),
            green: ColorConfig::from_rgb(80, 250, 123),
            orange: ColorConfig::from_rgb(255, 184, 108),
            pink: ColorConfig::from_rgb(255, 121, 198),
            purple: ColorConfig::from_rgb(189, 147, 249),
            red: ColorConfig::from_rgb(255, 85, 85),
            yellow: ColorConfig::from_rgb(241, 250, 140),
        }
    }
}
