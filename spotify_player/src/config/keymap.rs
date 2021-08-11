use crate::{
    command::Command,
    key::{Key, KeySequence},
};
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
/// Application's keymap configurations
pub struct KeymapConfig {
    #[serde(default)]
    pub keymaps: Vec<Keymap>,
}

#[derive(Clone, Debug, Deserialize)]
/// A keymap that maps a `KeySequence` to a `Command`
pub struct Keymap {
    pub key_sequence: KeySequence,
    pub command: Command,
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
                    key_sequence: ".".into(),
                    command: Command::PlayContext,
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
                    command: Command::ChooseSelected,
                },
                Keymap {
                    key_sequence: "r".into(),
                    command: Command::RefreshPlayback,
                },
                Keymap {
                    key_sequence: "/".into(),
                    command: Command::SearchContextTracks,
                },
                Keymap {
                    key_sequence: "tab".into(),
                    command: Command::FocusNextWindow,
                },
                Keymap {
                    key_sequence: "backtab".into(),
                    command: Command::FocusPreviousWindow,
                },
                Keymap {
                    key_sequence: "T".into(),
                    command: Command::SwitchTheme,
                },
                Keymap {
                    key_sequence: "D".into(),
                    command: Command::SwitchDevice,
                },
                Keymap {
                    key_sequence: "g p".into(),
                    command: Command::BrowseUserPlaylist,
                },
                Keymap {
                    key_sequence: "g space".into(),
                    command: Command::BrowsePlayingContext,
                },
                Keymap {
                    key_sequence: "g a".into(),
                    command: Command::BrowsePlayingTrackArtist,
                },
                Keymap {
                    key_sequence: "g A".into(),
                    command: Command::BrowsePlayingTrackAlbum,
                },
                Keymap {
                    key_sequence: "C-g a".into(),
                    command: Command::BrowseSelectedTrackArtist,
                },
                Keymap {
                    key_sequence: "C-g A".into(),
                    command: Command::BrowseSelectedTrackAlbum,
                },
                Keymap {
                    key_sequence: "g s a".into(),
                    command: Command::BrowseSelectedTrackArtist,
                },
                Keymap {
                    key_sequence: "g s A".into(),
                    command: Command::BrowseSelectedTrackAlbum,
                },
                Keymap {
                    key_sequence: "backspace".into(),
                    command: Command::PreviousFrame,
                },
                Keymap {
                    key_sequence: "?".into(),
                    command: Command::OpenCommandHelp,
                },
                Keymap {
                    key_sequence: "C-h".into(),
                    command: Command::OpenCommandHelp,
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
                    command: Command::ClosePopup,
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
                    key_sequence: "down".into(),
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
                    key_sequence: "up".into(),
                    command: Command::SelectPrevious,
                },
                Keymap {
                    key_sequence: "s t".into(),
                    command: Command::SortByTrack,
                },
                Keymap {
                    key_sequence: "s a".into(),
                    command: Command::SortByArtists,
                },
                Keymap {
                    key_sequence: "s A".into(),
                    command: Command::SortByAlbum,
                },
                Keymap {
                    key_sequence: "s d".into(),
                    command: Command::SortByDuration,
                },
                Keymap {
                    key_sequence: "s D".into(),
                    command: Command::SortByAddedDate,
                },
                Keymap {
                    key_sequence: "s r".into(),
                    command: Command::ReverseOrder,
                },
            ],
        }
    }
}

impl KeymapConfig {
    /// parses a list of keymaps from the keymap config file in `path` folder
    /// and updates the current keymaps accordingly.
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
                // a dumb approach (with quadratic complexity) to merge two different keymap arrays
                // while keeping the invariant:
                // - each `KeySequence` is mapped to only one `Command`.
                keymaps.into_iter().for_each(|keymap| {
                    if !self
                        .keymaps
                        .iter()
                        .any(|k| k.key_sequence == keymap.key_sequence)
                    {
                        self.keymaps.push(keymap);
                    }
                });
            }
        }
        Ok(())
    }

    /// finds all keymaps whose mapped key sequence has a given `prefix` key sequence as its prefix
    pub fn find_matched_prefix_keymaps(&self, prefix: &KeySequence) -> Vec<&Keymap> {
        self.keymaps
            .iter()
            .filter(|&keymap| prefix.is_prefix(&keymap.key_sequence))
            .collect()
    }

    /// finds a command from a mapped key sequence
    pub fn find_command_from_key_sequence(&self, key_sequence: &KeySequence) -> Option<Command> {
        self.keymaps
            .iter()
            .find(|&keymap| keymap.key_sequence == *key_sequence)
            .map(|keymap| keymap.command)
    }
}

impl From<&str> for Key {
    /// converts a string into a `Key`.
    /// **Note** this function will panic if the given string is not a valid
    /// representation of a `Key`.
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_else(|| panic!("invalid key {}", s))
    }
}

impl From<&str> for KeySequence {
    /// converts a string into a `KeySequence`.
    /// **Note** this function will panic if the given string is not a valid
    /// representation of a `KeySequence`.
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_else(|| panic!("invalid key sequence {}", s))
    }
}
