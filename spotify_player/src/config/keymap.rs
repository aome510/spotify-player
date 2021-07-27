use config_parser2::*;
use crossterm::event::KeyCode;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub enum Command {
    // player commands
    NextTrack,
    PreviousTrack,
    Search,
    ResumePause,
    Repeat,
    Shuffle,
    PlaySelectedTrack,
    SearchContextTracks,
    SortContextTracks,
    SwitchPlaylists,

    Quit,

    SelectNext,
    SelectPrevious,
}

#[derive(Debug, Deserialize, ConfigParse)]
/// Application's key mappings
pub struct KeymapConfig {
    pub keymaps: Vec<Keymap>,
}

/// Key denotes a key received from user's input
#[derive(Debug)]
pub enum Key {
    None(KeyCode),
    Ctrl(KeyCode),
    Alt(KeyCode),
}

#[derive(Debug, Deserialize, ConfigParse)]
pub struct Keymap {
    command: Command,
    keys: Vec<Key>,
}

impl KeymapConfig {
    pub fn parse_config_file(&mut self, path: &std::path::Path) -> Result<()> {
        match std::fs::read_to_string(path.join(super::KEYMAP_CONFIG_FILE)) {
            Err(err) => {
                log::warn!(
                    "failed to open the keymap config file: {:#?}...\nUse the default configurations instead...",
                    err
                );
            }
            Ok(content) => {
                self.parse(toml::from_str::<toml::Value>(&content)?)?;
            }
        }
        Ok(())
    }
}

impl<'de> serde::de::Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let err = Err(serde::de::Error::custom(format!(
            "failed to parse key: unknown key {}",
            s
        )));

        let chars: Vec<char> = s.chars().collect();
        if chars.len() == 1 {
            // a single character
            Ok(Key::None(KeyCode::Char(chars[0])))
        } else if chars.len() > 2 && chars[1] == '-' {
            // M-<c> for alt-<c> and C-<c> for ctrl-C
            match chars[0] {
                'C' => Ok(Key::Ctrl(KeyCode::Char(chars[2]))),
                'M' => Ok(Key::Alt(KeyCode::Char(chars[2]))),
                _ => err,
            }
        } else {
            match s.as_str() {
                "enter" => Ok(Key::None(KeyCode::Enter)),
                "tab" => Ok(Key::None(KeyCode::Tab)),
                "backspace" => Ok(Key::None(KeyCode::Backspace)),
                "esc" => Ok(Key::None(KeyCode::Esc)),

                "left" => Ok(Key::None(KeyCode::Left)),
                "right" => Ok(Key::None(KeyCode::Right)),
                "up" => Ok(Key::None(KeyCode::Up)),
                "down" => Ok(Key::None(KeyCode::Down)),

                "insert" => Ok(Key::None(KeyCode::Insert)),
                "delete" => Ok(Key::None(KeyCode::Delete)),
                "home" => Ok(Key::None(KeyCode::Home)),
                "end" => Ok(Key::None(KeyCode::End)),
                "page_up" => Ok(Key::None(KeyCode::PageUp)),
                "page_down" => Ok(Key::None(KeyCode::PageDown)),

                "f1" => Ok(Key::None(KeyCode::F(1))),
                "f2" => Ok(Key::None(KeyCode::F(2))),
                "f3" => Ok(Key::None(KeyCode::F(3))),
                "f4" => Ok(Key::None(KeyCode::F(4))),
                "f5" => Ok(Key::None(KeyCode::F(5))),
                "f6" => Ok(Key::None(KeyCode::F(6))),
                "f7" => Ok(Key::None(KeyCode::F(7))),
                "f8" => Ok(Key::None(KeyCode::F(8))),
                "f9" => Ok(Key::None(KeyCode::F(9))),
                "f10" => Ok(Key::None(KeyCode::F(10))),
                "f11" => Ok(Key::None(KeyCode::F(11))),
                "f12" => Ok(Key::None(KeyCode::F(12))),

                _ => err,
            }
        }
    }
}

config_parser_impl!(Command);
config_parser_impl!(Key);
