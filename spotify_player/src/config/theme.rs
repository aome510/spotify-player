use config_parser2::*;
use serde::Deserialize;
use tui::{style::*, text::*};

#[derive(Debug, Deserialize, ConfigParse)]
/// Application theme configurations
pub struct ThemeConfig {
    pub palette: PaletteConfig,
}

#[derive(Debug, Deserialize, ConfigParse)]
pub struct PaletteConfig {
    pub background: ColorConfig,
    pub foreground: ColorConfig,
    pub selection_bg: ColorConfig,
    pub selection_fg: ColorConfig,

    pub black: ColorConfig,
    pub blue: ColorConfig,
    pub cyan: ColorConfig,
    pub green: ColorConfig,
    pub magenta: ColorConfig,
    pub red: ColorConfig,
    pub white: ColorConfig,
    pub yellow: ColorConfig,

    pub bright_black: ColorConfig,
    pub bright_white: ColorConfig,
    pub bright_red: ColorConfig,
    pub bright_magenta: ColorConfig,
    pub bright_green: ColorConfig,
    pub bright_cyan: ColorConfig,
    pub bright_blue: ColorConfig,
    pub bright_yellow: ColorConfig,
}

#[derive(Debug)]
pub struct ColorConfig {
    pub color: Color,
}

fn to_hex_digit(c: char) -> u8 {
    c.to_digit(16).unwrap() as u8
}

impl<'de> serde::de::Deserialize<'de> for ColorConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        match Self::from_hex(&hex) {
            None => Err(serde::de::Error::custom(format!("invalid color {}", hex))),
            Some(c) => Ok(c),
        }
    }
}

impl ColorConfig {
    fn from_hex(hex: &str) -> Option<Self> {
        let mut chars = hex
            .chars()
            .map(|c| c.to_ascii_lowercase())
            .collect::<Vec<_>>();
        if chars.len() != 7 {
            return None;
        }
        if chars.remove(0) != '#' {
            return None;
        }
        if chars.iter().any(|c| !c.is_ascii_hexdigit()) {
            return None;
        }
        let r = to_hex_digit(chars[0]) * 16 + to_hex_digit(chars[1]);
        let g = to_hex_digit(chars[2]) * 16 + to_hex_digit(chars[3]);
        let b = to_hex_digit(chars[4]) * 16 + to_hex_digit(chars[5]);
        Some(Self {
            color: Color::Rgb(r, g, b),
        })
    }
}

config_parser_impl!(ColorConfig);

impl ThemeConfig {
    // parses configurations from a theme config file in `path` folder,
    // then updates the current configurations accordingly.
    pub fn parse_config_file(&mut self, path: &std::path::Path) -> Result<()> {
        match std::fs::read_to_string(path.join(super::THEME_CONFIG_FILE)) {
            Err(err) => {
                log::warn!(
                    "failed to open the theme config file: {:#?}...\nUse the default configurations instead...",
                    err
                );
            }
            Ok(content) => {
                self.parse(toml::from_str::<toml::Value>(&content)?)?;
            }
        }
        Ok(())
    }

    pub fn app_style(&self) -> Style {
        Style::default()
            .bg(self.palette.background.color)
            .fg(self.palette.foreground.color)
    }

    pub fn primary_text_desc_style(&self) -> Style {
        Style::default()
            .fg(self.palette.bright_cyan.color)
            .add_modifier(Modifier::BOLD)
    }

    pub fn secondary_text_desc_style(&self) -> Style {
        Style::default().fg(self.palette.bright_yellow.color)
    }

    pub fn gauge_style(&self) -> Style {
        Style::default()
            .fg(self.palette.selection_bg.color)
            .bg(self.palette.green.color)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn comment_style(&self) -> Style {
        Style::default().fg(self.palette.bright_black.color)
    }

    pub fn current_playing_style(&self) -> Style {
        Style::default()
            .fg(self.palette.green.color)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selection_style(&self) -> Style {
        Style::default()
            .bg(self.palette.selection_bg.color)
            .fg(self.palette.selection_fg.color)
            .add_modifier(Modifier::BOLD)
    }

    pub fn table_header_style(&self) -> Style {
        Style::default().fg(self.palette.bright_blue.color)
    }

    pub fn block_title_with_style<'a, S>(&self, content: S) -> Span<'a>
    where
        S: Into<String>,
    {
        Span::styled(
            content.into(),
            Style::default().fg(self.palette.magenta.color),
        )
    }
}

impl From<&str> for ColorConfig {
    fn from(s: &str) -> Self {
        Self::from_hex(s).unwrap()
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            palette: PaletteConfig {
                // dracula color palette based on https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Dracula.yml
                background: "#1e1f29".into(),
                foreground: "#f8f8f2".into(),
                selection_bg: "#44475a".into(),
                selection_fg: "#ffffff".into(),

                black: "#000000".into(),
                blue: "#bd93f9".into(),
                cyan: "#8be9fd".into(),
                green: "#50fa7b".into(),
                magenta: "#ff79c6".into(),
                red: "#ff5555".into(),
                white: "#bbbbbb".into(),
                yellow: "#f1fa8c".into(),

                bright_black: "#555555".into(),
                bright_blue: "#bd93f9".into(),
                bright_cyan: "#8be9fd".into(),
                bright_green: "#50fa7b".into(),
                bright_magenta: "#ff79c6".into(),
                bright_red: "#ff5555".into(),
                bright_white: "#ffffff".into(),
                bright_yellow: "#f1fa8c".into(),
            },
        }
    }
}
