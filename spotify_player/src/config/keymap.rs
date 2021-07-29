use anyhow::Result;
use crossterm::event::KeyCode;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub enum Command {
    // player commands
    NextTrack,
    PreviousTrack,
    ResumePause,
    Repeat,
    Shuffle,
    SearchContextTracks,
    // SortContextTracks,
    SwitchPlaylists,

    Quit,
    ToDefaultMode,

    SelectNext,
    SelectPrevious,
    PlaySelected,

    None,
}

#[derive(Debug, Deserialize)]
/// Application's key mappings
pub struct KeymapConfig {
    keymaps: Vec<Keymap>,
}

/// Key denotes a key received from user's input
#[derive(Debug, PartialEq, Eq)]
pub enum Key {
    None(KeyCode),
    Ctrl(KeyCode),
    Alt(KeyCode),
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct Keymap {
    key: Key,
    command: Command,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        KeymapConfig {
            keymaps: vec![
                Keymap {
                    key: "n".into(),
                    command: Command::NextTrack,
                },
                Keymap {
                    key: "p".into(),
                    command: Command::PreviousTrack,
                },
                Keymap {
                    key: " ".into(),
                    command: Command::ResumePause,
                },
                Keymap {
                    key: "C-r".into(),
                    command: Command::Repeat,
                },
                Keymap {
                    key: "C-s".into(),
                    command: Command::Shuffle,
                },
                Keymap {
                    key: "enter".into(),
                    command: Command::PlaySelected,
                },
                Keymap {
                    key: "/".into(),
                    command: Command::SearchContextTracks,
                },
                // Keymap {
                //     key: "s".into(),
                //     command: Command::SortContextTracks,
                // },
                Keymap {
                    key: "P".into(),
                    command: Command::SwitchPlaylists,
                },
                Keymap {
                    key: "q".into(),
                    command: Command::Quit,
                },
                Keymap {
                    key: "C-c".into(),
                    command: Command::Quit,
                },
                Keymap {
                    key: "esc".into(),
                    command: Command::ToDefaultMode,
                },
                Keymap {
                    key: "j".into(),
                    command: Command::SelectNext,
                },
                Keymap {
                    key: "C-j".into(),
                    command: Command::SelectNext,
                },
                Keymap {
                    key: "k".into(),
                    command: Command::SelectPrevious,
                },
                Keymap {
                    key: "C-k".into(),
                    command: Command::SelectPrevious,
                },
            ],
        }
    }
}

impl KeymapConfig {
    /// parses the list of keymaps from a config file and updates
    /// the current keymaps accordingly.
    pub fn parse_config_file(&mut self, path: &std::path::Path) -> Result<()> {
        match std::fs::read_to_string(path.join(super::KEYMAP_CONFIG_FILE)) {
            Err(err) => {
                log::warn!(
                    "failed to open the keymap config file: {:#?}...\nUse the default configurations instead...",
                    err
                );
            }
            Ok(content) => {
                let mut keymaps = toml::from_str::<Self>(&content)?.keymaps;
                std::mem::swap(&mut self.keymaps, &mut keymaps);
                // a dumb approach (quadratic complexity) to merge two different keymap arrays
                // while keeping the invariant that each `Key` is mapped to only one `Command`.
                keymaps.into_iter().for_each(|keymap| {
                    if self.keymaps.iter().find(|&k| k.key == keymap.key).is_none() {
                        self.keymaps.push(keymap);
                    }
                });
            }
        }
        Ok(())
    }

    /// gets the command from a key. Returns `None` if
    /// the given key is not mapped to any commands.
    pub fn get_command_from_key(&self, key: &Key) -> Option<Command> {
        self.keymaps
            .iter()
            .find(|&keymap| keymap.key == *key)
            .map(|keymap| keymap.command.clone())
    }
}

impl Key {
    pub fn from_str(s: &str) -> Option<Self> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() == 1 {
            // a single character
            Some(Key::None(KeyCode::Char(chars[0])))
        } else if chars.len() > 2 && chars[1] == '-' {
            // M-<c> for alt-<c> and C-<c> for ctrl-C
            match chars[0] {
                'C' => Some(Key::Ctrl(KeyCode::Char(chars[2]))),
                'M' => Some(Key::Alt(KeyCode::Char(chars[2]))),
                _ => None,
            }
        } else {
            match s {
                "enter" => Some(Key::None(KeyCode::Enter)),
                "tab" => Some(Key::None(KeyCode::Tab)),
                "backspace" => Some(Key::None(KeyCode::Backspace)),
                "esc" => Some(Key::None(KeyCode::Esc)),

                "left" => Some(Key::None(KeyCode::Left)),
                "right" => Some(Key::None(KeyCode::Right)),
                "up" => Some(Key::None(KeyCode::Up)),
                "down" => Some(Key::None(KeyCode::Down)),

                "insert" => Some(Key::None(KeyCode::Insert)),
                "delete" => Some(Key::None(KeyCode::Delete)),
                "home" => Some(Key::None(KeyCode::Home)),
                "end" => Some(Key::None(KeyCode::End)),
                "page_up" => Some(Key::None(KeyCode::PageUp)),
                "page_down" => Some(Key::None(KeyCode::PageDown)),

                "f1" => Some(Key::None(KeyCode::F(1))),
                "f2" => Some(Key::None(KeyCode::F(2))),
                "f3" => Some(Key::None(KeyCode::F(3))),
                "f4" => Some(Key::None(KeyCode::F(4))),
                "f5" => Some(Key::None(KeyCode::F(5))),
                "f6" => Some(Key::None(KeyCode::F(6))),
                "f7" => Some(Key::None(KeyCode::F(7))),
                "f8" => Some(Key::None(KeyCode::F(8))),
                "f9" => Some(Key::None(KeyCode::F(9))),
                "f10" => Some(Key::None(KeyCode::F(10))),
                "f11" => Some(Key::None(KeyCode::F(11))),
                "f12" => Some(Key::None(KeyCode::F(12))),

                _ => None,
            }
        }
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap()
    }
}

impl<'de> serde::de::Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match Self::from_str(&s) {
            Some(key) => Ok(key),
            None => Err(serde::de::Error::custom(format!(
                "failed to parse key: unknown key {}",
                s
            ))),
        }
    }
}
