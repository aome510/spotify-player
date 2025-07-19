mod client;
mod commands;
mod handlers;

use crate::config;
use rspotify::model::{AlbumId, ArtistId, Id, PlaylistId, TrackId};
use serde::{Deserialize, Serialize};

const MAX_REQUEST_SIZE: usize = 4096;

pub use client::start_socket;
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

#[derive(Debug, Serialize, Deserialize, clap::ValueEnum, Clone)]
pub enum EditAction {
    Add,
    Delete,
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
    Edit {
        playlist_id: PlaylistId<'static>,
        action: EditAction,
        track_id: TrackId<'static>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Command {
    StartContext {
        context_type: ContextType,
        id_or_name: IdOrName,
        shuffle: bool,
    },
    StartTrack(IdOrName),
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
    Search { query: String },
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

pub fn init_cli() -> anyhow::Result<clap::Command> {
    let default_cache_folder = config::get_cache_folder_path()?;
    let default_config_folder = config::get_config_folder_path()?;

    let cmd = clap::Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .subcommand(commands::init_get_subcommand())
        .subcommand(commands::init_playback_subcommand())
        .subcommand(commands::init_connect_subcommand())
        .subcommand(commands::init_like_command())
        .subcommand(commands::init_authenticate_command())
        .subcommand(commands::init_playlist_subcommand())
        .subcommand(commands::init_generate_command())
        .subcommand(commands::init_search_command())
        .arg(
            clap::Arg::new("theme")
                .short('t')
                .long("theme")
                .value_name("THEME")
                .help("Application theme"),
        )
        .arg(
            clap::Arg::new("config-folder")
                .short('c')
                .long("config-folder")
                .value_name("FOLDER")
                .default_value(default_config_folder.into_os_string())
                .help("Path to the application's config folder"),
        )
        .arg(
            clap::Arg::new("cache-folder")
                .short('C')
                .long("cache-folder")
                .value_name("FOLDER")
                .default_value(default_cache_folder.into_os_string())
                .help("Path to the application's cache folder"),
        );

    #[cfg(feature = "daemon")]
    let cmd = cmd.arg(
        clap::Arg::new("daemon")
            .short('d')
            .long("daemon")
            .action(clap::ArgAction::SetTrue)
            .help("Running the application as a daemon"),
    );

    Ok(cmd)
}
