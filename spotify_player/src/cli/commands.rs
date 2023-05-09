use clap::{builder::EnumValueParser, value_parser, Arg, ArgAction, ArgGroup, Command};

use super::{ContextType, Key};

pub fn init_connect_subcommand() -> Command {
    Command::new("connect")
        .about("Connect to a Spotify device")
        .arg(Arg::new("id").long("id").short('i').help("device's id"))
        .arg(
            Arg::new("name")
                .long("name")
                .short('n')
                .help("device's name"),
        )
        .group(ArgGroup::new("device").args(["id", "name"]).required(true))
}

pub fn init_get_subcommand() -> Command {
    Command::new("get")
        .about("Get spotify data")
        .subcommand_required(true)
        .subcommand(
            Command::new("key").about("Get data by key").arg(
                Arg::new("key")
                    .value_parser(EnumValueParser::<Key>::new())
                    .required(true),
            ),
        )
        .subcommand(add_context_args(
            Command::new("context").about("Get context data"),
        ))
}

fn init_playback_start_subcommand() -> Command {
    Command::new("start")
        .about("Start a new playback")
        .subcommand_required(true)
        .subcommand(add_context_args(
            Command::new("context").about("Start a context playback"),
        ))
        .subcommand(
            Command::new("liked")
                .about("Start liked tracks playback")
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
}

fn add_context_args(cmd: Command) -> Command {
    cmd.arg(
        Arg::new("context_type")
            .value_parser(EnumValueParser::<ContextType>::new())
            .required(true),
    )
    .arg(Arg::new("id").long("id").short('i').help("context's id"))
    .arg(
        Arg::new("name")
            .long("name")
            .short('n')
            .help("context's name"),
    )
    .group(ArgGroup::new("context").args(["id", "name"]).required(true))
}

pub fn init_playback_subcommand() -> Command {
    Command::new("playback")
        .about("Interact with the playback")
        .subcommand_required(true)
        .subcommand(init_playback_start_subcommand())
        .subcommand(Command::new("play-pause").about("Toggle between play and pause"))
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
                        .help("Modify the volume percent by an offset"),
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
