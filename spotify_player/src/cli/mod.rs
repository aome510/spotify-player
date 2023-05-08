mod commands;
mod handlers;
mod socket;

use serde::{Deserialize, Serialize};

pub use commands::{init_get_subcommand, init_playback_subcommand};
pub use handlers::handle_cli_subcommand;
pub use socket::start_socket;

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

#[derive(Debug, Serialize, Deserialize)]
pub enum GetRequest {
    Key(Key),
    Context(String, ContextType),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    Start(String, ContextType),
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
}
