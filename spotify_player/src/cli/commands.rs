use clap::{builder::EnumValueParser, value_parser, Arg, Command};

use super::{ContextType, Key};

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
        .subcommand(
            Command::new("context")
                .about("Get context data")
                .arg(
                    Arg::new("context_type")
                        .value_parser(EnumValueParser::<ContextType>::new())
                        .required(true),
                )
                .arg(Arg::new("context_id").required(true)),
        )
}

fn init_playback_start_subcommand() -> Command {
    Command::new("start")
        .about("Start a context playback")
        .arg(
            Arg::new("context_type")
                .value_parser(EnumValueParser::<ContextType>::new())
                .required(true),
        )
        .arg(Arg::new("context_id").required(true))
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
                        .value_parser(value_parser!(u8).range(0..=100))
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("seek")
                .about("Seek by an offset milliseconds")
                .arg(
                    Arg::new("position_offset_ms")
                        .value_parser(value_parser!(i32))
                        .required(true),
                ),
        )
}
