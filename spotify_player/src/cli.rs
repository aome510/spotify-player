use crate::client::Client;
use anyhow::Result;
use clap::{builder::PossibleValue, value_parser, Arg, ArgMatches, Command};
use rspotify::{model::RepeatState, prelude::*};

fn init_get_subcommand() -> Command {
    Command::new("get")
        .about("Command(s) to get spotify data")
        .arg(Arg::new("key").value_parser([
            PossibleValue::new("playback"),
            PossibleValue::new("devices"),
            PossibleValue::new("user_playlists"),
            PossibleValue::new("user_liked_tracks"),
            PossibleValue::new("user_top_tracks"),
            PossibleValue::new("queue"),
        ]))
}

fn init_playback_play_subcommand() -> Command {
    Command::new("play").about("Command(s) to start a playback")
}

fn init_playback_subcommand() -> Command {
    Command::new("playback")
        .about("Command(s) to interact with the playback")
        .subcommand(init_playback_play_subcommand())
        .subcommand(Command::new("pause"))
        .subcommand(Command::new("next").about(""))
        .subcommand(Command::new("previous"))
        .subcommand(Command::new("shuffle"))
        .subcommand(Command::new("repeat"))
        .subcommand(
            Command::new("volume").arg(Arg::new("percent").value_parser(-100..=100).required(true)),
        )
        .subcommand(
            Command::new("seek").arg(Arg::new("position_ms").value_parser(value_parser!(u32))),
        )
}

pub fn init_cli_command() -> Command {
    Command::new("cli")
        .about("cli to interact with a running instance")
        .subcommand(init_get_subcommand())
        .subcommand(init_playback_subcommand())
}

async fn handle_get_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
    if let Some(key) = args.get_one::<String>("key") {
        match key.as_str() {
            "playback" => {
                let playback = client
                    .spotify
                    .current_playback(None, None::<Vec<_>>)
                    .await?;
                println!("{}", serde_json::to_string(&playback)?);
            }
            "devices" => {
                let devices = client.spotify.device().await?;
                println!("{}", serde_json::to_string(&devices)?);
            }
            "user_playlists" => {
                let playlists = client.current_user_playlists().await?;
                println!("{}", serde_json::to_string(&playlists)?);
            }
            "user_liked_tracks" => {
                let tracks = client.current_user_saved_tracks().await?;
                println!("{}", serde_json::to_string(&tracks)?);
            }
            "user_top_tracks" => {
                let tracks = client.current_user_top_tracks().await?;
                println!("{}", serde_json::to_string(&tracks)?);
            }
            "queue" => {
                let queue = client.spotify.current_user_queue().await?;
                println!("{}", serde_json::to_string(&queue)?);
            }
            _ => {}
        }
    }
    Ok(())
}

async fn handle_playback_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
    let playback = match client
        .spotify
        .current_playback(None, None::<Vec<_>>)
        .await?
    {
        Some(playback) => playback,
        None => {
            return Ok(());
        }
    };
    let device_id = playback.device.id.as_deref();

    match args.subcommand() {
        None => {}
        Some((cmd, args)) => match cmd {
            "play" => todo!(),
            "pause" => {
                client.spotify.pause_playback(device_id).await?;
            }
            "next" => {
                client.spotify.next_track(device_id).await?;
            }
            "previous" => {
                client.spotify.previous_track(device_id).await?;
            }
            "shuffle" => {
                client
                    .spotify
                    .shuffle(!playback.shuffle_state, device_id)
                    .await?;
            }
            "repeat" => {
                let next_repeat_state = match playback.repeat_state {
                    RepeatState::Off => RepeatState::Track,
                    RepeatState::Track => RepeatState::Context,
                    RepeatState::Context => RepeatState::Off,
                };

                client.spotify.repeat(next_repeat_state, device_id).await?;
            }
            "volume" => todo!(),
            "seek" => todo!(),
            _ => {
                println!("Unknown subcommand: {cmd}!");
            }
        },
    }
    Ok(())
}

pub async fn handle_cli_command(args: &ArgMatches, client: Client) -> Result<()> {
    match args.subcommand() {
        None => {}
        Some((cmd, args)) => match cmd {
            "get" => handle_get_subcommand(args, client).await?,
            "playback" => handle_playback_subcommand(args, client).await?,
            _ => {
                println!("Unknown subcommand: {cmd}!");
            }
        },
    }
    Ok(())
}
