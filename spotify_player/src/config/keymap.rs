use crate::key::{self, KeySequence};
use anyhow::Result;
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
    SwitchPlaylists,

    SortByTrack,
    SortByArtists,
    SortByAlbum,
    SortByDuration,
    SortByAddedDate,
    ReverseSort,

    Quit,
    ToDefaultMode,

    SelectNext,
    SelectPrevious,
    PlaySelected,
}

#[derive(Debug, Deserialize)]
/// Application's key mappings
pub struct KeymapConfig {
    keymaps: Vec<Keymap>,
}

#[derive(Debug, Deserialize)]
pub struct Keymap {
    key_sequence: KeySequence,
    command: Command,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        KeymapConfig {
            keymaps: vec![
                Keymap {
                    key_sequence: "n".into(),
                    command: Command::NextTrack,
                },
                Keymap {
                    key_sequence: "p".into(),
                    command: Command::PreviousTrack,
                },
                Keymap {
                    key_sequence: "space".into(),
                    command: Command::ResumePause,
                },
                Keymap {
                    key_sequence: "C-r".into(),
                    command: Command::Repeat,
                },
                Keymap {
                    key_sequence: "C-s".into(),
                    command: Command::Shuffle,
                },
                Keymap {
                    key_sequence: "enter".into(),
                    command: Command::PlaySelected,
                },
                Keymap {
                    key_sequence: "/".into(),
                    command: Command::SearchContextTracks,
                },
                Keymap {
                    key_sequence: "P".into(),
                    command: Command::SwitchPlaylists,
                },
                Keymap {
                    key_sequence: "q".into(),
                    command: Command::Quit,
                },
                Keymap {
                    key_sequence: "C-c".into(),
                    command: Command::Quit,
                },
                Keymap {
                    key_sequence: "esc".into(),
                    command: Command::ToDefaultMode,
                },
                Keymap {
                    key_sequence: "j".into(),
                    command: Command::SelectNext,
                },
                Keymap {
                    key_sequence: "C-j".into(),
                    command: Command::SelectNext,
                },
                Keymap {
                    key_sequence: "k".into(),
                    command: Command::SelectPrevious,
                },
                Keymap {
                    key_sequence: "C-k".into(),
                    command: Command::SelectPrevious,
                },
                Keymap {
                    key_sequence: "s q".into(),
                    command: Command::SortByTrack,
                },
                Keymap {
                    key_sequence: "s w".into(),
                    command: Command::SortByArtists,
                },
                Keymap {
                    key_sequence: "s e".into(),
                    command: Command::SortByAlbum,
                },
                Keymap {
                    key_sequence: "s r".into(),
                    command: Command::SortByDuration,
                },
                Keymap {
                    key_sequence: "s t".into(),
                    command: Command::SortByAddedDate,
                },
                Keymap {
                    key_sequence: "s y".into(),
                    command: Command::ReverseSort,
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
                    if self
                        .keymaps
                        .iter()
                        .find(|&k| k.key_sequence == keymap.key_sequence)
                        .is_none()
                    {
                        self.keymaps.push(keymap);
                    }
                });
            }
        }
        Ok(())
    }

    /// finds all mapped key sequences that has a given key sequence `prefix` as a prefix
    pub fn find_matched_prefix_key_sequences(
        &self,
        prefix: &key::KeySequence,
    ) -> Vec<&KeySequence> {
        self.keymaps
            .iter()
            .map(|keymap| &keymap.key_sequence)
            .filter(|&key_sequence| prefix.is_prefix(key_sequence))
            .collect()
    }

    /// finds a command from a mapped key sequence
    pub fn find_command_from_key_sequence(
        &self,
        key_sequence: &key::KeySequence,
    ) -> Option<Command> {
        self.keymaps
            .iter()
            .find(|&keymap| keymap.key_sequence == *key_sequence)
            .map(|keymap| keymap.command.clone())
    }
}

impl From<&str> for key::Key {
    /// converts a string into a `Key`.
    /// **Note** this function will panic if the given string is not a valid
    /// representation of a `Key`.
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap()
    }
}

impl From<&str> for key::KeySequence {
    /// converts a string into a `KeySequence`.
    /// **Note** this function will panic if the given string is not a valid
    /// representation of a `KeySequence`.
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap()
    }
}
