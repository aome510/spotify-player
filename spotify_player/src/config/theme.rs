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
            style::Style::default()
                .add_modifier(style::Modifier::REVERSED)
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

    // Terminal's ANSI colors construction functions.
    // Reference: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
    // The conversion from `style::Color` can be a bit counter-intuitive
    // as the `tui-rs` library doesn't follow the ANSI naming standard.

    pub fn black() -> Self {
        style::Color::Black.into()
    }
    pub fn red() -> Self {
        style::Color::LightRed.into()
    }
    pub fn green() -> Self {
        style::Color::LightGreen.into()
    }
    pub fn yellow() -> Self {
        style::Color::LightYellow.into()
    }
    pub fn blue() -> Self {
        style::Color::LightBlue.into()
    }
    pub fn magenta() -> Self {
        style::Color::LightMagenta.into()
    }
    pub fn cyan() -> Self {
        style::Color::LightCyan.into()
    }
    pub fn white() -> Self {
        style::Color::Gray.into()
    }
    pub fn bright_black() -> Self {
        style::Color::DarkGray.into()
    }
    pub fn bright_red() -> Self {
        style::Color::Red.into()
    }
    pub fn bright_green() -> Self {
        style::Color::Green.into()
    }
    pub fn bright_yellow() -> Self {
        style::Color::Yellow.into()
    }
    pub fn bright_blue() -> Self {
        style::Color::Blue.into()
    }
    pub fn bright_magenta() -> Self {
        style::Color::Magenta.into()
    }
    pub fn bright_cyan() -> Self {
        style::Color::Cyan.into()
    }
    pub fn bright_white() -> Self {
        style::Color::White.into()
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
                .bg(StyleColor::BrightBlack)
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
