mod commands;
mod handlers;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub use commands::{init_get_subcommand, init_playback_subcommand};
pub use handlers::handle_cli_subcommand;

#[derive(Serialize, Deserialize, clap::ValueEnum, Clone)]
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

#[derive(Serialize, Deserialize, clap::ValueEnum, Clone)]
pub enum ContextType {
    Playlist,
    Album,
    Artist,
}

#[derive(Serialize, Deserialize)]
pub enum GetRequest {
    Key(Key),
    Context(ContextType),
}

#[derive(Serialize, Deserialize)]
pub enum Request {
    Get(GetRequest),
}

pub trait ClientSocket {
    fn start_socket(&self, port: u16) -> Result<()>;

    fn handle_request(&self, request: Request) -> Result<()>;
}
