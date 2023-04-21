mod commands;
mod handlers;
mod socket;

use serde::{Deserialize, Serialize};

pub use commands::{init_get_subcommand, init_playback_subcommand};
pub use handlers::handle_cli_subcommand;
pub use socket::start_socket;

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
    Context(String, ContextType),
}

#[derive(Serialize, Deserialize)]
pub enum Request {
    Get(GetRequest),
}
