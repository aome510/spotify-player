use crate::{
    command::{Action, ActionTarget, Command, CommandOrAction},
    key::{Key, KeySequence},
};
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
/// Application's keymap configurations
pub struct KeymapConfig {
    #[serde(default)]
    pub keymaps: Vec<Keymap>,
    #[serde(default)]
    pub actions: Vec<ActionMap>,
}

#[derive(Clone, Debug, Deserialize)]
/// A keymap that maps a `KeySequence` to a `Command`
pub struct Keymap {
    pub key_sequence: KeySequence,
    pub command: Command,
}

#[derive(Clone, Debug, Deserialize)]
/// A keymap that triggers an `Action` when a key sequence is pressed
pub struct ActionMap {
    pub key_sequence: KeySequence,
    #[serde(default)]
    pub target: ActionTarget,
    pub action: Action,
}

impl Default for KeymapConfig {
    fn default() -> Self {
        KeymapConfig {
            actions: vec![],
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
                    key_sequence: "M-r".into(),
                    command: Command::ToggleFakeTrackRepeatMode,
                },
                Keymap {
                    key_sequence: "C-s".into(),
                    command: Command::Shuffle,
                },
                Keymap {
                    key_sequence: "+".into(),
                    command: Command::VolumeChange { offset: 5 },
                },
                Keymap {
                    key_sequence: "-".into(),
                    command: Command::VolumeChange { offset: -5 },
                },
                Keymap {
                    key_sequence: "_".into(),
                    command: Command::Mute,
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
                    key_sequence: "C-z".into(),
                    command: Command::AddSelectedItemToQueue,
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
                Keymap {
                    key_sequence: "g L".into(),
                    command: Command::LyricsPage,
                },
                Keymap {
                    key_sequence: "l".into(),
                    command: Command::LyricsPage,
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
                    key_sequence: "O".into(),
                    command: Command::OpenSpotifyLinkFromClipboard,
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
                Keymap {
                    key_sequence: "N".into(),
                    command: Command::CreatePlaylist,
                },
                Keymap {
                    key_sequence: "g c".into(),
                    command: Command::JumpToCurrentTrackInContext,
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
                let mut parsed = toml::from_str::<Self>(&content)?;
                std::mem::swap(&mut self.keymaps, &mut parsed.keymaps);
                std::mem::swap(&mut self.actions, &mut parsed.actions);

                // a dumb approach (with quadratic complexity) to merge two different keymap arrays
                // while keeping the invariant:
                // - each `KeySequence` is mapped to only one `Command`.
                parsed.keymaps.into_iter().for_each(|keymap| {
                    if !self
                        .keymaps
                        .iter()
                        .any(|k| k.key_sequence == keymap.key_sequence)
                    {
                        self.keymaps.push(keymap);
                    }
                });
                parsed.actions.into_iter().for_each(|action| {
                    if !self
                        .actions
                        .iter()
                        .any(|k| k.key_sequence == action.key_sequence)
                    {
                        self.actions.push(action);
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

    /// finds all actions whose mapped key sequence has a given `prefix` key sequence as its prefix
    pub fn find_matched_prefix_actions(&self, prefix: &KeySequence) -> Vec<&ActionMap> {
        self.actions
            .iter()
            .filter(|&action| prefix.is_prefix(&action.key_sequence))
            .collect()
    }

    /// checks if there is any command or action that has a given `prefix` key sequence as its prefix
    pub fn has_matched_prefix(&self, prefix: &KeySequence) -> bool {
        let keymaps = self.find_matched_prefix_keymaps(prefix);
        let actions = self.find_matched_prefix_actions(prefix);
        !keymaps.is_empty() || !actions.is_empty()
    }

    /// finds a command from a mapped key sequence
    pub fn find_command_from_key_sequence(&self, key_sequence: &KeySequence) -> Option<Command> {
        self.keymaps
            .iter()
            .find(|&keymap| keymap.key_sequence == *key_sequence && keymap.command != Command::None)
            .map(|keymap| keymap.command)
    }

    /// finds an action from a mapped key sequence
    pub fn find_action_from_key_sequence(
        &self,
        key_sequence: &KeySequence,
    ) -> Option<(Action, ActionTarget)> {
        self.actions
            .iter()
            .find(|&action| action.key_sequence == *key_sequence)
            .map(|action| (action.action, action.target))
    }

    /// finds a command or action from a mapped key sequence
    pub fn find_command_or_action_from_key_sequence(
        &self,
        key_sequence: &KeySequence,
    ) -> Option<CommandOrAction> {
        if let Some(command) = self.find_command_from_key_sequence(key_sequence) {
            return Some(CommandOrAction::Command(command));
        }
        if let Some((action, target)) = self.find_action_from_key_sequence(key_sequence) {
            return Some(CommandOrAction::Action(action, target));
        }
        None
    }
}

impl Keymap {
    pub fn include_in_help_screen(&self) -> bool {
        !matches!(&self.command, Command::None)
    }
}

impl std::fmt::Display for Keymap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} -> {:?}: {}",
            self.key_sequence,
            self.command,
            self.command.desc()
        )
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
