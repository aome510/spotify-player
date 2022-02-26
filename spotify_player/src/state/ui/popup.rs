use crate::state::model::*;
use tui::widgets::ListState;

#[derive(Debug)]
pub enum PopupState {
    CommandHelp { offset: usize },
    Search { query: String },
    UserPlaylistList(PlaylistPopupAction, ListState),
    UserFollowedArtistList(ListState),
    UserSavedAlbumList(ListState),
    DeviceList(ListState),
    ArtistList(Vec<Artist>, ListState),
    ThemeList(Vec<crate::config::Theme>, ListState),
    ActionList(Item, ListState),
}

/// An action on a playlist popup list
#[derive(Debug)]
pub enum PlaylistPopupAction {
    Browse,
    AddTrack(TrackId),
}

impl PopupState {
    /// gets the (immutable) list state of a (list) popup
    pub fn list_state(&self) -> Option<&ListState> {
        match self {
            Self::DeviceList(list_state) => Some(list_state),
            Self::UserPlaylistList(.., list_state) => Some(list_state),
            Self::UserFollowedArtistList(list_state) => Some(list_state),
            Self::UserSavedAlbumList(list_state) => Some(list_state),
            Self::ArtistList(.., list_state) => Some(list_state),
            Self::ThemeList(.., list_state) => Some(list_state),
            Self::ActionList(.., list_state) => Some(list_state),
            Self::CommandHelp { .. } | Self::Search { .. } => None,
        }
    }

    /// gets the (mutable) list state of a (list) popup
    pub fn list_state_mut(&mut self) -> Option<&mut ListState> {
        match self {
            Self::DeviceList(list_state) => Some(list_state),
            Self::UserPlaylistList(.., list_state) => Some(list_state),
            Self::UserFollowedArtistList(list_state) => Some(list_state),
            Self::UserSavedAlbumList(list_state) => Some(list_state),
            Self::ArtistList(.., list_state) => Some(list_state),
            Self::ThemeList(.., list_state) => Some(list_state),
            Self::ActionList(.., list_state) => Some(list_state),
            Self::CommandHelp { .. } | Self::Search { .. } => None,
        }
    }

    /// gets the selected position of a (list) popup
    pub fn list_selected(&self) -> Option<usize> {
        match self.list_state() {
            None => None,
            Some(state) => state.selected(),
        }
    }

    /// selects a position in a (list) popup
    pub fn list_select(&mut self, id: Option<usize>) {
        match self.list_state_mut() {
            None => {}
            Some(state) => state.select(id),
        }
    }
}
