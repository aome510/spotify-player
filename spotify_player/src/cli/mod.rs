mod client;
mod commands;
mod handlers;

use rspotify::model::*;
use serde::{Deserialize, Serialize};

const MAX_REQUEST_SIZE: usize = 4096;

pub use client::start_socket;
pub use commands::*;
pub use handlers::handle_cli_subcommand;

#[derive(Debug, Serialize, Deserialize, clap::ValueEnum, Clone)]
pub enum Key {
    Playback,
    Devices,
    UserPlaylists,
    UserLikedTracks,
    UserSavedAlbums,
    UserFollowedArtists,
    UserTopTracks,
    Queue,
}

#[derive(Debug, Serialize, Deserialize, clap::ValueEnum, Clone)]
pub enum ContextType {
    Playlist,
    Album,
    Artist,
}

#[derive(Debug, Serialize, Deserialize, clap::ValueEnum, Clone)]
pub enum ItemType {
    Playlist,
    Album,
    Artist,
    Track,
}

/// Spotify item's ID
enum ItemId {
    Playlist(PlaylistId<'static>),
    Artist(ArtistId<'static>),
    Album(AlbumId<'static>),
    Track(TrackId<'static>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GetRequest {
    Key(Key),
    Item(ItemType, IdOrName),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IdOrName {
    Id(String),
    Name(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlaylistCommand {
    New {
        name: String,
        public: bool,
        collab: bool,
        description: String,
    },
    Delete {
        id: PlaylistId<'static>,
    },
    List,
    Import {
        from: PlaylistId<'static>,
        to: PlaylistId<'static>,
        delete: bool,
    },
    Fork {
        id: PlaylistId<'static>,
    },
    Sync {
        id: Option<PlaylistId<'static>>,
        delete: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    StartContext {
        context_type: ContextType,
        id_or_name: IdOrName,
        shuffle: bool,
    },
    StartLikedTracks {
        limit: usize,
        random: bool,
    },
    StartRadio(ItemType, IdOrName),
    PlayPause,
    Play,
    Pause,
    Next,
    Previous,
    Shuffle,
    Repeat,
    Volume {
        percent: i8,
        is_offset: bool,
    },
    Seek(i64),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Get(GetRequest),
    Playback(Command),
    Connect(IdOrName),
    Like { unlike: bool },
    Playlist(PlaylistCommand),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok(Vec<u8>),
    Err(Vec<u8>),
}

impl From<ContextType> for ItemType {
    fn from(value: ContextType) -> Self {
        match value {
            ContextType::Playlist => Self::Playlist,
            ContextType::Album => Self::Album,
            ContextType::Artist => Self::Artist,
        }
    }
}

impl ItemId {
    pub fn uri(&self) -> String {
        match self {
            ItemId::Playlist(id) => id.uri(),
            ItemId::Artist(id) => id.uri(),
            ItemId::Album(id) => id.uri(),
            ItemId::Track(id) => id.uri(),
        }
    }
}
