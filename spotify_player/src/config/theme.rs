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

    pub current_active: Style,

    pub context_desc: Style,
    pub context_tracks_table_header: Style,
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Style {
    pub fg: Option<StyleColor>,
    pub bg: Option<StyleColor>,
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
    Rgb(u8, u8, u8),
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
        match std::fs::read_to_string(path.join(super::THEME_CONFIG_FILE)) {
            Err(err) => {
                log::warn!(
                    "failed to open the theme config file: {:#?}...\nUse the default configurations instead...",
                    err
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

    pub fn selection_style(&self) -> style::Style {
        style::Style::default()
            .bg(self.palette.selection_background.color)
            .fg(self.palette.selection_foreground.color)
            .add_modifier(style::Modifier::BOLD)
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
        current_active,
        context_desc,
        context_tracks_table_header
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
            Self::Rgb(r, g, b) => style::Color::Rgb(r, g, b),
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

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            themes: vec![
                Theme::default(),
                Theme {
                    // Ayu Light color palette based on https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/ayu_light.yml
                    name: "ayu_light".to_owned(),
                    palette: Palette {
                        foreground: "#5c6773".into(),
                        background: "#fafafa".into(),
                        selection_foreground: "#5c6773".into(),
                        selection_background: "#f0eee4".into(),
                        black: "#000000".into(),
                        blue: "#41a6d9".into(),
                        cyan: "#4dbf99".into(),
                        green: "#86b300".into(),
                        magenta: "#f07178".into(),
                        red: "#ff3333".into(),
                        white: "#ffffff".into(),
                        yellow: "#f29718".into(),
                        bright_black: "#323232".into(),
                        bright_blue: "#73d8ff".into(),
                        bright_cyan: "#7ff1cb".into(),
                        bright_green: "#b8e532".into(),
                        bright_magenta: "#ffa3aa".into(),
                        bright_red: "#ff6565".into(),
                        bright_white: "#ffffff".into(),
                        bright_yellow: "#ffc94a".into(),
                    },
                    component_style: ComponentStyle::default(),
                },
                Theme {
                    // Gruvbox Dark color palette based on https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Gruvbox%20Dark.yml
                    name: "gruvbox_dark".to_owned(),
                    palette: Palette {
                        foreground: "#e6d4a3".into(),
                        background: "#1e1e1e".into(),
                        selection_foreground: "#534a42".into(),
                        selection_background: "#e6d4a3".into(),
                        black: "#1e1e1e".into(),
                        blue: "#377375".into(),
                        cyan: "#578e57".into(),
                        green: "#868715".into(),
                        magenta: "#a04b73".into(),
                        red: "#be0f17".into(),
                        white: "#978771".into(),
                        yellow: "#cc881a".into(),
                        bright_black: "#7f7061".into(),
                        bright_blue: "#719586".into(),
                        bright_cyan: "#7db669".into(),
                        bright_green: "#aab01e".into(),
                        bright_magenta: "#c77089".into(),
                        bright_red: "#f73028".into(),
                        bright_white: "#e6d4a3".into(),
                        bright_yellow: "#f7b125".into(),
                    },
                    component_style: ComponentStyle::default(),
                },
                Theme {
                    // Solarized Light palette based on https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Builtin%20Solarized%20Light.yml
                    name: "solarized_light".to_owned(),
                    palette: Palette {
                        background: "#fdf6e3".into(),
                        foreground: "#657b83".into(),
                        selection_background: "#eee8d5".into(),
                        selection_foreground: "#586e75".into(),
                        black: "#073642".into(),
                        blue: "#268bd2".into(),
                        cyan: "#2aa198".into(),
                        green: "#859900".into(),
                        magenta: "#d33682".into(),
                        red: "#dc322f".into(),
                        white: "#eee8d5".into(),
                        yellow: "#b58900".into(),
                        bright_black: "#002b36".into(),
                        bright_blue: "#839496".into(),
                        bright_cyan: "#93a1a1".into(),
                        bright_green: "#586e75".into(),
                        bright_magenta: "#6c71c4".into(),
                        bright_red: "#cb4b16".into(),
                        bright_white: "#fdf6e3".into(),
                        bright_yellow: "#657b83".into(),
                    },
                    component_style: ComponentStyle::default(),
                },
            ],
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // Dracula color palette based on https://github.com/mbadolato/iTerm2-Color-Schemes/blob/master/alacritty/Dracula.yml
            name: "dracula".to_owned(),
            palette: Palette {
                background: "#1e1f29".into(),
                foreground: "#f8f8f2".into(),
                selection_background: "#44475a".into(),
                selection_foreground: "#ffffff".into(),
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
                .fg(StyleColor::SelectionBackground)
                .bg(StyleColor::Green)
                .modifiers(vec![StyleModifier::Italic]),

            current_active: Style::default()
                .fg(StyleColor::Green)
                .modifiers(vec![StyleModifier::Bold]),

            context_desc: Style::default()
                .fg(StyleColor::Cyan)
                .modifiers(vec![StyleModifier::Bold]),
            context_tracks_table_header: Style::default().fg(StyleColor::Blue),
        }
    }
}
