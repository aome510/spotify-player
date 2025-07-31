use clap::{builder::EnumValueParser, value_parser, Arg, ArgAction, ArgGroup, Command};
use clap_complete::Shell;

use super::{ContextType, ItemType, Key};

pub fn init_connect_subcommand() -> Command {
    add_id_or_name_group(Command::new("connect").about("Connect to a Spotify device"))
}

pub fn init_get_subcommand() -> Command {
    Command::new("get")
        .about("Get Spotify data")
        .subcommand_required(true)
        .subcommand(
            Command::new("key").about("Get data by key").arg(
                Arg::new("key")
                    .value_parser(EnumValueParser::<Key>::new())
                    .required(true),
            ),
        )
        .subcommand(add_id_or_name_group(
            Command::new("item").about("Get a Spotify item's data").arg(
                Arg::new("item_type")
                    .value_parser(EnumValueParser::<ItemType>::new())
                    .required(true),
            ),
        ))
}

fn init_playback_start_subcommand() -> Command {
    Command::new("start")
        .about("Start a new playback")
        .subcommand_required(true)
        .subcommand(add_id_or_name_group(
            Command::new("context")
                .about("Start a context playback")
                .arg(
                    Arg::new("context_type")
                        .value_parser(EnumValueParser::<ContextType>::new())
                        .required(true),
                )
                .arg(
                    Arg::new("shuffle")
                        .short('s')
                        .long("shuffle")
                        .action(ArgAction::SetTrue)
                        .help("Shuffle tracks within the launched playback"),
                ),
        ))
        .subcommand(add_id_or_name_group(
            Command::new("track").about("Start playback for a track"),
        ))
        .subcommand(
            Command::new("liked")
                .about("Start a liked tracks playback")
                .arg(
                    Arg::new("limit")
                        .short('l')
                        .long("limit")
                        .default_value("200")
                        .value_parser(value_parser!(usize))
                        .help("The limit for number of tracks to play"),
                )
                .arg(
                    Arg::new("random")
                        .short('r')
                        .long("random")
                        .action(ArgAction::SetTrue)
                        .help(
                            "Randomly pick the tracks instead of picking tracks from the beginning",
                        ),
                ),
        )
        .subcommand(add_id_or_name_group(
            Command::new("radio")
                .about("Start a radio playback")
                .arg(Arg::new("item_type").value_parser(EnumValueParser::<ItemType>::new())),
        ))
}

fn add_id_or_name_group(cmd: Command) -> Command {
    cmd.arg(Arg::new("id").long("id").short('i'))
        .arg(Arg::new("name").long("name").short('n'))
        .group(
            ArgGroup::new("id_or_name")
                .args(["id", "name"])
                .required(true),
        )
}

pub fn init_playback_subcommand() -> Command {
    Command::new("playback")
        .about("Interact with the playback")
        .subcommand_required(true)
        .subcommand(init_playback_start_subcommand())
        .subcommand(Command::new("play-pause").about("Toggle between play and pause"))
        .subcommand(Command::new("play").about("Resume the current playback if stopped"))
        .subcommand(Command::new("pause").about("Pause the current playback if playing"))
        .subcommand(Command::new("next").about("Skip to the next track"))
        .subcommand(Command::new("previous").about("Skip to the previous track"))
        .subcommand(Command::new("shuffle").about("Toggle the shuffle mode"))
        .subcommand(Command::new("repeat").about("Cycle the repeat mode"))
        .subcommand(
            Command::new("volume")
                .about("Set the volume percentage")
                .arg(
                    Arg::new("percent")
                        .value_parser(value_parser!(i8).range(-100..=100))
                        .required(true),
                )
                .arg(
                    Arg::new("offset")
                        .long("offset")
                        .action(clap::ArgAction::SetTrue)
                        .help("Increase the volume percent by an offset"),
                ),
        )
        .subcommand(
            Command::new("seek")
                .about("Seek by an offset milliseconds")
                .arg(
                    Arg::new("position_offset_ms")
                        .value_parser(value_parser!(i64))
                        .required(true),
                ),
        )
}

pub fn init_search_command() -> Command {
    Command::new("search")
        .about("Search spotify")
        .arg(Arg::new("query").help("Search query").required(true))
}

pub fn init_like_command() -> Command {
    Command::new("like")
        .about("Like currently playing track")
        .arg(
            Arg::new("unlike")
                .long("unlike")
                .short('u')
                .action(ArgAction::SetTrue)
                .help("Unlike the currently playing track"),
        )
}

pub fn init_authenticate_command() -> Command {
    Command::new("authenticate").about("Authenticate the application")
}

pub fn init_generate_command() -> Command {
    Command::new("generate")
        .about("Generate shell completion for the application CLI")
        .arg(
            Arg::new("shell")
                .action(ArgAction::Set)
                .value_parser(value_parser!(Shell))
                .required(true),
        )
}

pub fn init_playlist_subcommand() -> Command {
    Command::new("playlist")
        .about("Playlist editing")
        .subcommand_required(true)
        .subcommand(Command::new("new").about("Create a new playlist")
            .arg(Arg::new("name")
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .arg(Arg::new("description")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .required(false))
            .arg(Arg::new("public")
                .short('p')
                .long("public")
                .action(clap::ArgAction::SetTrue)
                .help("Sets the playlist to public"))
            .arg(Arg::new("collab")
                .short('c')
                .long("collab")
                .action(clap::ArgAction::SetTrue)
                .help("Sets the playlist to collaborative"))
            )
        .subcommand(Command::new("delete").about("Delete a playlist")
            .arg(Arg::new("id")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())))
        .subcommand(Command::new("import").about("Imports all songs from a playlist into another playlist.")
            .arg(Arg::new("from")
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .arg(Arg::new("to")
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .arg(Arg::new("delete")
                .short('d')
                .long("delete")
                .action(clap::ArgAction::SetTrue)
                .help("Deletes any previously imported tracks that are no longer in the imported playlist since last import."))
            .after_help("Import data for each playlist is stored inside the application's cache folder. If imported again, the command only imports new tracks since last import."))
        .subcommand(Command::new("list").about("Lists all user playlists."))
        .subcommand(Command::new("fork").about("Creates a copy of a playlist and imports it.")
            .arg(Arg::new("id")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())))
        .subcommand(Command::new("sync").about("Syncs imports for all playlists or a single playlist.")
            .arg(Arg::new("id")
                .required(false)
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .arg(Arg::new("delete")
                .short('d')
                .long("delete")
                .action(clap::ArgAction::SetTrue)
                .help("Deletes any previously imported tracks that are no longer in an imported playlist since last import.")))
        .subcommand(Command::new("edit").about("Add or remove tracks or albums from a playlist.")
            .arg(Arg::new("playlist_id")
                .help("Playlist ID")
                .required(true)
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .arg(Arg::new("action")
                .help("Action to perform")
                .required(true)
                .value_parser(["add", "delete"]))
            .arg(Arg::new("track_id")
                .long("track-id")
                .short('t')
                .help("Track ID to add or remove")
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .arg(Arg::new("album_id")
                .long("album-id")
                .short('a')
                .help("Album ID to add or remove")
                .value_parser(clap::builder::NonEmptyStringValueParser::new()))
            .group(
                ArgGroup::new("content_id")
                    .args(["track_id", "album_id"])
                    .required(true)
            ))
}
