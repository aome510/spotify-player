use std::str::FromStr;

use anyhow::Result;
use serde::Deserialize;
use tui::style;

#[derive(Clone, Debug, Deserialize)]
/// Application theme configurations.
pub struct ThemeConfig {
    #[serde(default)]
    pub themes: Vec<Theme>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(default)]
    palette: Palette,
    #[serde(default)]
    component_style: ComponentStyle,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Palette {
    pub background: Option<Color>,
    pub foreground: Option<Color>,

    #[serde(default = "Color::black")]
    pub black: Color,
    #[serde(default = "Color::blue")]
    pub blue: Color,
    #[serde(default = "Color::cyan")]
    pub cyan: Color,
    #[serde(default = "Color::green")]
    pub green: Color,
    #[serde(default = "Color::magenta")]
    pub magenta: Color,
    #[serde(default = "Color::red")]
    pub red: Color,
    #[serde(default = "Color::white")]
    pub white: Color,
    #[serde(default = "Color::yellow")]
    pub yellow: Color,

    #[serde(default = "Color::bright_black")]
    pub bright_black: Color,
    #[serde(default = "Color::bright_white")]
    pub bright_white: Color,
    #[serde(default = "Color::bright_red")]
    pub bright_red: Color,
    #[serde(default = "Color::bright_magenta")]
    pub bright_magenta: Color,
    #[serde(default = "Color::bright_green")]
    pub bright_green: Color,
    #[serde(default = "Color::bright_cyan")]
    pub bright_cyan: Color,
    #[serde(default = "Color::bright_blue")]
    pub bright_blue: Color,
    #[serde(default = "Color::bright_yellow")]
    pub bright_yellow: Color,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ComponentStyle {
    pub block_title: Option<Style>,
    pub border: Option<Style>,
    pub playback_track: Option<Style>,
    pub playback_artists: Option<Style>,
    pub playback_album: Option<Style>,
    pub playback_metadata: Option<Style>,
    pub playback_progress_bar: Option<Style>,
    pub current_playing: Option<Style>,
    pub page_desc: Option<Style>,
    pub table_header: Option<Style>,
    pub selection: Option<Style>,
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Style {
    pub fg: Option<StyleColor>,
    pub bg: Option<StyleColor>,
    #[serde(default)]
    pub modifiers: Vec<StyleModifier>,
}

#[derive(Copy, Clone, Debug)]
pub enum StyleColor {
    Black,
    Blue,
    Cyan,
    Green,
    Magenta,
    Red,
    White,
    Yellow,
    BrightBlack,
    BrightWhite,
    BrightRed,
    BrightMagenta,
    BrightGreen,
    BrightCyan,
    BrightBlue,
    BrightYellow,
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum StyleModifier {
    Bold,
    Italic,
    Reversed,
}

#[derive(Clone, Debug)]
pub struct Color {
    pub color: style::Color,
}

impl ThemeConfig {
    /// finds a theme whose name matches a given `name`
    pub fn find_theme(&self, name: &str) -> Option<Theme> {
        self.themes.iter().find(|&t| t.name == name).cloned()
    }

    pub fn new(path: &std::path::Path) -> Result<Self> {
        let mut config = Self::default();
        config.parse_config_file(path)?;

        Ok(config)
    }

    /// parses configurations from a theme config file in `path` folder,
    /// then updates the current configurations accordingly.
    fn parse_config_file(&mut self, path: &std::path::Path) -> Result<()> {
        let file_path = path.join(super::THEME_CONFIG_FILE);
        match std::fs::read_to_string(&file_path) {
            Err(err) => {
                tracing::warn!(
                    "Failed to open the theme config file (path={file_path:?}): {err:#}. Use the default configurations instead",
                );
            }
            Ok(content) => {
                let config = toml::from_str::<Self>(&content)?;

                // merge user-defined themes and the application default themes
                // Skip any theme whose name conflicts with already existed theme in the current application's themes
                config.themes.into_iter().for_each(|theme| {
                    if !self.themes.iter().any(|t| t.name == theme.name) {
                        self.themes.push(theme);
                    }
                });
            }
        }
        Ok(())
    }
}

impl Theme {
    pub fn app_style(&self) -> style::Style {
        let mut style = style::Style::default();
        if let Some(ref c) = self.palette.background {
            style = style.bg(c.color);
        }
        if let Some(ref c) = self.palette.foreground {
            style = style.fg(c.color);
        }
        style
    }

    pub fn selection_style(&self, is_active: bool) -> style::Style {
        if is_active {
            match &self.component_style.selection {
                None => style::Style::default()
                    .add_modifier(style::Modifier::REVERSED)
                    .add_modifier(style::Modifier::BOLD),
                Some(s) => s.style(&self.palette),
            }
        } else {
            style::Style::default()
        }
    }

    pub fn _text_with_style<'a, S>(
        &self,
        content: S,
        style: tui::style::Style,
    ) -> tui::text::Span<'a>
    where
        S: Into<String>,
    {
        tui::text::Span::styled(content.into(), style)
    }

    pub fn block_title_with_style<'a, S>(&self, content: S) -> tui::text::Span<'a>
    where
        S: Into<String>,
    {
        tui::text::Span::styled(content.into(), self.block_title())
    }

    pub fn block_title(&self) -> tui::style::Style {
        match &self.component_style.block_title {
            None => Style::default()
                .fg(StyleColor::Magenta)
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn border(&self) -> tui::style::Style {
        match &self.component_style.border {
            None => Style::default().style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn playback_track(&self) -> tui::style::Style {
        match &self.component_style.playback_track {
            None => Style::default()
                .fg(StyleColor::Cyan)
                .modifiers(vec![StyleModifier::Bold])
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn playback_artists(&self) -> tui::style::Style {
        match &self.component_style.playback_artists {
            None => Style::default()
                .fg(StyleColor::Cyan)
                .modifiers(vec![StyleModifier::Bold])
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn playback_album(&self) -> tui::style::Style {
        match &self.component_style.playback_album {
            None => Style::default().fg(StyleColor::Yellow).style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn playback_metadata(&self) -> tui::style::Style {
        match &self.component_style.playback_metadata {
            None => Style::default()
                .fg(StyleColor::BrightBlack)
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn playback_progress_bar(&self) -> tui::style::Style {
        match &self.component_style.playback_progress_bar {
            None => Style::default()
                .bg(StyleColor::BrightBlack)
                .fg(StyleColor::Green)
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn current_playing(&self) -> tui::style::Style {
        match &self.component_style.current_playing {
            None => Style::default()
                .fg(StyleColor::Green)
                .modifiers(vec![StyleModifier::Bold])
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn page_desc(&self) -> tui::style::Style {
        match &self.component_style.page_desc {
            None => Style::default()
                .fg(StyleColor::Cyan)
                .modifiers(vec![StyleModifier::Bold])
                .style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }

    pub fn table_header(&self) -> tui::style::Style {
        match &self.component_style.table_header {
            None => Style::default().fg(StyleColor::Blue).style(&self.palette),
            Some(s) => s.style(&self.palette),
        }
    }
}

impl Style {
    pub fn style(&self, palette: &Palette) -> style::Style {
        let mut style = style::Style::default();
        if let Some(fg) = self.fg {
            style = style.fg(fg.color(palette));
        }
        if let Some(bg) = self.bg {
            style = style.bg(bg.color(palette));
        }
        self.modifiers.iter().for_each(|&m| {
            style = style.add_modifier(m.into());
        });
        style
    }

    pub fn fg(mut self, fg: StyleColor) -> Self {
        self.fg = Some(fg);
        self
    }

    pub fn bg(mut self, bg: StyleColor) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn modifiers(mut self, modifiers: Vec<StyleModifier>) -> Self {
        self.modifiers = modifiers;
        self
    }
}

impl<'de> serde::de::Deserialize<'de> for StyleColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        fn rgb_from_hex(s: &str) -> Option<(u8, u8, u8)> {
            if !s.starts_with('#') || s.len() != 7 {
                None
            } else {
                Some((
                    u8::from_str_radix(&s[1..3], 16).ok()?,
                    u8::from_str_radix(&s[3..5], 16).ok()?,
                    u8::from_str_radix(&s[5..7], 16).ok()?,
                ))
            }
        }

        let str = String::deserialize(deserializer)?;
        Ok(match str.as_str() {
            "Black" => StyleColor::Black,
            "Blue" => StyleColor::Blue,
            "Cyan" => StyleColor::Cyan,
            "Green" => StyleColor::Green,
            "Magenta" => StyleColor::Magenta,
            "Red" => StyleColor::Red,
            "White" => StyleColor::White,
            "Yellow" => StyleColor::Yellow,
            "BrightBlack" => StyleColor::BrightBlack,
            "BrightWhite" => StyleColor::BrightWhite,
            "BrightRed" => StyleColor::BrightRed,
            "BrightMagenta" => StyleColor::BrightMagenta,
            "BrightGreen" => StyleColor::BrightGreen,
            "BrightCyan" => StyleColor::BrightCyan,
            "BrightBlue" => StyleColor::BrightBlue,
            "BrightYellow" => StyleColor::BrightYellow,
            s => match rgb_from_hex(s) {
                Some((r, g, b)) => StyleColor::Rgb { r, g, b },
                None => return Err(serde::de::Error::custom(format!("invalid hex color: {s}"))),
            },
        })
    }
}

impl StyleColor {
    pub fn color(&self, palette: &Palette) -> style::Color {
        match *self {
            Self::Black => palette.black.color,
            Self::Blue => palette.blue.color,
            Self::Cyan => palette.cyan.color,
            Self::Green => palette.green.color,
            Self::Magenta => palette.magenta.color,
            Self::Red => palette.red.color,
            Self::White => palette.white.color,
            Self::Yellow => palette.yellow.color,
            Self::BrightBlack => palette.bright_black.color,
            Self::BrightWhite => palette.bright_white.color,
            Self::BrightRed => palette.bright_red.color,
            Self::BrightMagenta => palette.bright_magenta.color,
            Self::BrightGreen => palette.bright_green.color,
            Self::BrightCyan => palette.bright_cyan.color,
            Self::BrightBlue => palette.bright_blue.color,
            Self::BrightYellow => palette.bright_yellow.color,
            Self::Rgb { r, g, b } => style::Color::Rgb(r, g, b),
        }
    }
}

impl From<StyleModifier> for style::Modifier {
    fn from(m: StyleModifier) -> Self {
        match m {
            StyleModifier::Bold => style::Modifier::BOLD,
            StyleModifier::Italic => style::Modifier::ITALIC,
            StyleModifier::Reversed => style::Modifier::REVERSED,
        }
    }
}

impl<'de> serde::de::Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        match style::Color::from_str(&str) {
            Err(err) => Err(serde::de::Error::custom(format!(
                "invalid color {str}: {err:#}"
            ))),
            Ok(color) => Ok(Color { color }),
        }
    }
}

impl Color {
    pub fn black() -> Self {
        style::Color::Black.into()
    }
    pub fn red() -> Self {
        style::Color::Red.into()
    }
    pub fn green() -> Self {
        style::Color::Green.into()
    }
    pub fn yellow() -> Self {
        style::Color::Yellow.into()
    }
    pub fn blue() -> Self {
        style::Color::Blue.into()
    }
    pub fn magenta() -> Self {
        style::Color::Magenta.into()
    }
    pub fn cyan() -> Self {
        style::Color::Cyan.into()
    }
    pub fn white() -> Self {
        style::Color::Gray.into()
    }
    pub fn bright_black() -> Self {
        style::Color::DarkGray.into()
    }
    pub fn bright_red() -> Self {
        style::Color::LightRed.into()
    }
    pub fn bright_green() -> Self {
        style::Color::LightGreen.into()
    }
    pub fn bright_yellow() -> Self {
        style::Color::LightYellow.into()
    }
    pub fn bright_blue() -> Self {
        style::Color::LightBlue.into()
    }
    pub fn bright_magenta() -> Self {
        style::Color::LightMagenta.into()
    }
    pub fn bright_cyan() -> Self {
        style::Color::LightCyan.into()
    }
    pub fn bright_white() -> Self {
        style::Color::White.into()
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        Color {
            color: style::Color::from_str(s).expect("valid color"),
        }
    }
}

impl From<style::Color> for Color {
    fn from(value: style::Color) -> Self {
        Self { color: value }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            themes: vec![Theme::default()],
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_owned(),
            palette: Palette::default(),
            component_style: ComponentStyle::default(),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: None,
            foreground: None,
            // the default theme uses the terminal's ANSI colors
            black: Color::black(),
            red: Color::red(),
            green: Color::green(),
            yellow: Color::yellow(),
            blue: Color::blue(),
            magenta: Color::magenta(),
            cyan: Color::cyan(),
            white: Color::white(),
            bright_black: Color::bright_black(),
            bright_red: Color::bright_red(),
            bright_green: Color::bright_green(),
            bright_yellow: Color::bright_yellow(),
            bright_blue: Color::bright_blue(),
            bright_magenta: Color::bright_magenta(),
            bright_cyan: Color::bright_cyan(),
            bright_white: Color::bright_white(),
        }
    }
}
