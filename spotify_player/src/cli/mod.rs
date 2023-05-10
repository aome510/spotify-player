mod client;
mod commands;
mod handlers;

use serde::{Deserialize, Serialize};

pub use client::start_socket;
pub use commands::{init_connect_subcommand, init_get_subcommand, init_playback_subcommand};
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

#[derive(Debug, Serialize, Deserialize)]
pub enum GetRequest {
    Key(Key),
    Context(ContextType, IdOrName),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IdOrName {
    Id(String),
    Name(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    StartContext(ContextType, IdOrName),
    StartLikedTracks { limit: usize, random: bool },
    StartRadio(ItemType, IdOrName),
    PlayPause,
    Next,
    Previous,
    Shuffle,
    Repeat,
    Volume(i8, bool),
    Seek(i64),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Get(GetRequest),
    Playback(Command),
    Connect(IdOrName),
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
