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
                    command: Command::PlayRandom,
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
                    key_sequence: "+".into(),
                    command: Command::VolumeUp,
                },
                Keymap {
                    key_sequence: "-".into(),
                    command: Command::VolumeDown,
                },
                Keymap {
                    key_sequence: ">".into(),
                    command: Command::SeekForward,
                },
                Keymap {
                    key_sequence: "<".into(),
                    command: Command::SeekBackward,
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
                    command: Command::Search,
                },
                Keymap {
                    key_sequence: "z".into(),
                    command: Command::Queue,
                },
                Keymap {
                    key_sequence: "Z".into(),
                    command: Command::AddSelectedItemToQueue,
                },
                Keymap {
                    key_sequence: "C-space".into(),
                    command: Command::ShowActionsOnSelectedItem,
                },
                Keymap {
                    key_sequence: "g a".into(),
                    command: Command::ShowActionsOnSelectedItem,
                },
                Keymap {
                    key_sequence: "a".into(),
                    command: Command::ShowActionsOnCurrentTrack,
                },
                #[cfg(feature = "streaming")]
                Keymap {
                    key_sequence: "R".into(),
                    command: Command::RestartIntegratedClient,
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
                    key_sequence: "u p".into(),
                    command: Command::BrowseUserPlaylists,
                },
                Keymap {
                    key_sequence: "u a".into(),
                    command: Command::BrowseUserFollowedArtists,
                },
                Keymap {
                    key_sequence: "u A".into(),
                    command: Command::BrowseUserSavedAlbums,
                },
                Keymap {
                    key_sequence: "g space".into(),
                    command: Command::CurrentlyPlayingContextPage,
                },
                Keymap {
                    key_sequence: "g t".into(),
                    command: Command::TopTrackPage,
                },
                Keymap {
                    key_sequence: "g r".into(),
                    command: Command::RecentlyPlayedTrackPage,
                },
                Keymap {
                    key_sequence: "g y".into(),
                    command: Command::LikedTrackPage,
                },
                #[cfg(feature = "lyric-finder")]
                Keymap {
                    key_sequence: "g L".into(),
                    command: Command::LyricPage,
                },
                #[cfg(feature = "lyric-finder")]
                Keymap {
                    key_sequence: "l".into(),
                    command: Command::LyricPage,
                },
                Keymap {
                    key_sequence: "g l".into(),
                    command: Command::LibraryPage,
                },
                Keymap {
                    key_sequence: "g s".into(),
                    command: Command::SearchPage,
                },
                Keymap {
                    key_sequence: "g b".into(),
                    command: Command::BrowsePage,
                },
                Keymap {
                    key_sequence: "backspace".into(),
                    command: Command::PreviousPage,
                },
                Keymap {
                    key_sequence: "C-q".into(),
                    command: Command::PreviousPage,
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
                    command: Command::SelectNextOrScrollDown,
                },
                Keymap {
                    key_sequence: "C-n".into(),
                    command: Command::SelectNextOrScrollDown,
                },
                Keymap {
                    key_sequence: "down".into(),
                    command: Command::SelectNextOrScrollDown,
                },
                Keymap {
                    key_sequence: "k".into(),
                    command: Command::SelectPreviousOrScrollUp,
                },
                Keymap {
                    key_sequence: "C-p".into(),
                    command: Command::SelectPreviousOrScrollUp,
                },
                Keymap {
                    key_sequence: "up".into(),
                    command: Command::SelectPreviousOrScrollUp,
                },
                Keymap {
                    key_sequence: "page_up".into(),
                    command: Command::PageSelectPreviousOrScrollUp,
                },
                Keymap {
                    key_sequence: "C-b".into(),
                    command: Command::PageSelectPreviousOrScrollUp,
                },
                Keymap {
                    key_sequence: "page_down".into(),
                    command: Command::PageSelectNextOrScrollDown,
                },
                Keymap {
                    key_sequence: "C-f".into(),
                    command: Command::PageSelectNextOrScrollDown,
                },
                Keymap {
                    key_sequence: "g g".into(),
                    command: Command::SelectFirstOrScrollToTop,
                },
                Keymap {
                    key_sequence: "home".into(),
                    command: Command::SelectFirstOrScrollToTop,
                },
                Keymap {
                    key_sequence: "G".into(),
                    command: Command::SelectLastOrScrollToBottom,
                },
                Keymap {
                    key_sequence: "end".into(),
                    command: Command::SelectLastOrScrollToBottom,
                },
                Keymap {
                    key_sequence: "s t".into(),
                    command: Command::SortTrackByTitle,
                },
                Keymap {
                    key_sequence: "s a".into(),
                    command: Command::SortTrackByArtists,
                },
                Keymap {
                    key_sequence: "s A".into(),
                    command: Command::SortTrackByAlbum,
                },
                Keymap {
                    key_sequence: "s d".into(),
                    command: Command::SortTrackByDuration,
                },
                Keymap {
                    key_sequence: "s D".into(),
                    command: Command::SortTrackByAddedDate,
                },
                Keymap {
                    key_sequence: "s r".into(),
                    command: Command::ReverseTrackOrder,
                },
                Keymap {
                    key_sequence: "C-k".into(),
                    command: Command::MovePlaylistItemUp,
                },
                Keymap {
                    key_sequence: "C-j".into(),
                    command: Command::MovePlaylistItemDown,
                },
            ],
        }
    }
}

impl KeymapConfig {
    pub fn new(path: &std::path::Path) -> Result<Self> {
        let mut config = Self::default();
        config.parse_config_file(path)?;

        Ok(config)
    }
    /// parses a list of keymaps from the keymap config file in `path` folder
    /// and updates the current keymaps accordingly.
    fn parse_config_file(&mut self, path: &std::path::Path) -> Result<()> {
        let file_path = path.join(super::KEYMAP_CONFIG_FILE);
        match std::fs::read_to_string(&file_path) {
            Err(err) => {
                tracing::warn!(
                    "Failed to open the keymap config file (path={file_path:?}): {err:#}. Use the default configurations instead",
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

impl Keymap {
    pub fn include_in_help_screen(&self) -> bool {
        !matches!(&self.command, Command::None)
    }
}

impl From<&str> for Key {
    /// converts a string into a `Key`.
    /// # Panics
    /// This function will panic if the given string is not a valid
    /// representation of a `Key`.
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_else(|| panic!("invalid key {s}"))
    }
}

impl From<&str> for KeySequence {
    /// converts a string into a `KeySequence`.
    /// # Panics
    /// This function will panic if the given string is not a valid
    /// representation of a `KeySequence`.
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_else(|| panic!("invalid key sequence {s}"))
    }
}
