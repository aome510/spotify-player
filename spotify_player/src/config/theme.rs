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
    palette: Palette,
    #[serde(default)]
    component_style: ComponentStyle,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Palette {
    pub background: Color,
    pub foreground: Color,
    pub selection_background: Color,
    pub selection_foreground: Color,

    pub black: Color,
    pub blue: Color,
    pub cyan: Color,
    pub green: Color,
    pub magenta: Color,
    pub red: Color,
    pub white: Color,
    pub yellow: Color,

    pub bright_black: Color,
    pub bright_white: Color,
    pub bright_red: Color,
    pub bright_magenta: Color,
    pub bright_green: Color,
    pub bright_cyan: Color,
    pub bright_blue: Color,
    pub bright_yellow: Color,
}

// TODO: find a way to parse ComponentStyle config options without
// having to specify all the fields
#[derive(Clone, Debug, Deserialize)]
pub struct ComponentStyle {
    pub block_title: Style,

    pub playback_track: Style,
    pub playback_album: Style,
    pub playback_metadata: Style,
    pub playback_progress_bar: Style,

    pub current_playing: Style,

    pub page_desc: Style,
    pub table_header: Style,
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Style {
    pub fg: Option<StyleColor>,
    pub bg: Option<StyleColor>,
    #[serde(default)]
    pub modifiers: Vec<StyleModifier>,
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum StyleColor {
    Background,
    Foreground,
    SelectionBackground,
    SelectionForeground,
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
}

#[derive(Clone, Debug)]
pub struct Color {
    pub color: style::Color,
}

macro_rules! impl_component_style_getters {
	($($f:ident),+) => {
		$(
            pub fn $f(&self) -> tui::style::Style {
                self.component_style.$f.style(&self.palette)
            }
        )*
	};
}

impl ThemeConfig {
    /// finds a theme whose name matches a given `name`
    pub fn find_theme(&self, name: &str) -> Option<Theme> {
        self.themes.iter().find(|&t| t.name == name).cloned()
    }

    /// parses configurations from a theme config file in `path` folder,
    /// then updates the current configurations accordingly.
    pub fn parse_config_file(&mut self, path: &std::path::Path) -> Result<()> {
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
        style::Style::default()
            .bg(self.palette.background.color)
            .fg(self.palette.foreground.color)
    }

    pub fn selection_style(&self, is_active: bool) -> style::Style {
        if is_active {
            style::Style::default()
                .bg(self.palette.selection_background.color)
                .fg(self.palette.selection_foreground.color)
                .add_modifier(style::Modifier::BOLD)
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

    impl_component_style_getters!(
        block_title,
        playback_track,
        playback_album,
        playback_metadata,
        playback_progress_bar,
        current_playing,
        page_desc,
        table_header
    );
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

impl StyleColor {
    pub fn color(&self, palette: &Palette) -> style::Color {
        match *self {
            Self::Background => palette.background.color,
            Self::Foreground => palette.foreground.color,
            Self::SelectionBackground => palette.selection_background.color,
            Self::SelectionForeground => palette.selection_foreground.color,
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
        }
    }
}

fn to_hex_digit(c: char) -> u8 {
    c.to_digit(16).unwrap() as u8
}

impl<'de> serde::de::Deserialize<'de> for Color {
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

impl Color {
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
            color: style::Color::Rgb(r, g, b),
        })
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        Self::from_hex(s).unwrap()
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
            palette: Palette {
                background: "#1e1f29".into(),
                foreground: "#f8f8f2".into(),

                // ANSI colors for default palette
                // Reference: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
                // The conversion from `style::Color` can be a bit counter-intuitive
                // as the `tui-rs` library doesn't follow the ANSI naming standard.
                black: style::Color::Black.into(),
                red: style::Color::LightRed.into(),
                green: style::Color::LightGreen.into(),
                yellow: style::Color::LightYellow.into(),
                blue: style::Color::LightBlue.into(),
                magenta: style::Color::LightMagenta.into(),
                cyan: style::Color::LightCyan.into(),
                white: style::Color::Gray.into(),

                bright_black: style::Color::DarkGray.into(),
                bright_red: style::Color::Red.into(),
                bright_green: style::Color::Green.into(),
                bright_yellow: style::Color::Yellow.into(),
                bright_blue: style::Color::Blue.into(),
                bright_magenta: style::Color::Magenta.into(),
                bright_cyan: style::Color::Cyan.into(),
                bright_white: style::Color::White.into(),
            },
            component_style: ComponentStyle::default(),
        }
    }
}

impl Default for ComponentStyle {
    fn default() -> Self {
        Self {
            block_title: Style::default().fg(StyleColor::Magenta),

            playback_track: Style::default()
                .fg(StyleColor::Cyan)
                .modifiers(vec![StyleModifier::Bold]),
            playback_album: Style::default().fg(StyleColor::Yellow),
            playback_metadata: Style::default().fg(StyleColor::BrightBlack),
            playback_progress_bar: Style::default()
                .bg(StyleColor::SelectionBackground)
                .fg(StyleColor::Green),

            current_playing: Style::default()
                .fg(StyleColor::Green)
                .modifiers(vec![StyleModifier::Bold]),

            page_desc: Style::default()
                .fg(StyleColor::Cyan)
                .modifiers(vec![StyleModifier::Bold]),
            table_header: Style::default().fg(StyleColor::Blue),
        }
    }
}
