use std::process::exit;

use crate::client::Client;
use anyhow::Result;
use clap::{builder::EnumValueParser, value_parser, Arg, ArgMatches, Command};
use rspotify::{model::RepeatState, prelude::*};

#[derive(clap::ValueEnum, Clone)]
enum Key {
    Playback,
    Devices,
    UserPlaylists,
    UserLikedTracks,
    UserSavedAlbums,
    UserFollowedArtists,
    UserTopTracks,
    Queue,
}

enum ContextType {
    Playlist,
    Album,
    Artist
}

pub fn init_get_subcommand() -> Command {
    Command::new("get").about("Get spotify data").arg(
        Arg::new("key")
            .value_parser(EnumValueParser::<Key>::new())
            .required(true),
    )
}

fn init_playback_play_subcommand() -> Command {
    Command::new("play")
        .about("Start a playback")
        .arg(Arg::new("context_id").required(true))
}

pub fn init_playback_subcommand() -> Command {
    Command::new("playback")
        .about("Interact with the playback")
        .subcommand_required(true)
        .subcommand(init_playback_play_subcommand())
        .subcommand(Command::new("resume").about("Resume the playback"))
        .subcommand(Command::new("pause").about("Pause the playback"))
        .subcommand(Command::new("next").about("Next track"))
        .subcommand(Command::new("previous").about("Previous track"))
        .subcommand(Command::new("shuffle").about("Toggle the shuffle mode"))
        .subcommand(Command::new("repeat").about("Cycle the repeat mode"))
        .subcommand(
            Command::new("volume")
                .about("Set playback's volume percentage")
                .arg(
                    Arg::new("percent")
                        .value_parser(value_parser!(u8).range(0..100))
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("seek")
                .about("Seek the playback by an offset")
                .arg(
                    Arg::new("position_offset_ms")
                        .value_parser(value_parser!(i32))
                        .required(true),
                ),
        )
}

async fn handle_get_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
    let key = args.get_one::<Key>("key").expect("key is required");
    match key {
        Key::Playback => {
            let playback = client
                .spotify
                .current_playback(None, None::<Vec<_>>)
                .await?;
            println!("{}", serde_json::to_string(&playback)?);
        }
        Key::Devices => {
            let devices = client.spotify.device().await?;
            println!("{}", serde_json::to_string(&devices)?);
        }
        Key::UserPlaylists => {
            let playlists = client.current_user_playlists().await?;
            println!("{}", serde_json::to_string(&playlists)?);
        }
        Key::UserLikedTracks => {
            let tracks = client.current_user_saved_tracks().await?;
            println!("{}", serde_json::to_string(&tracks)?);
        }
        Key::UserTopTracks => {
            let tracks = client.current_user_top_tracks().await?;
            println!("{}", serde_json::to_string(&tracks)?);
        }
        Key::UserSavedAlbums => {
            let albums = client.current_user_saved_albums().await?;
            println!("{}", serde_json::to_string(&albums)?);
        }
        Key::UserFollowedArtists => {
            let artists = client.current_user_followed_artists().await?;
            println!("{}", serde_json::to_string(&artists)?);
        }
        Key::Queue => {
            let queue = client.spotify.current_user_queue().await?;
            println!("{}", serde_json::to_string(&queue)?);
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
            eprintln!("No playback found!");
            exit(1);
        }
    };
    let device_id = playback.device.id.as_deref();

    let (cmd, args) = args.subcommand().expect("playback subcommand is required");
    match cmd {
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
        "volume" => {
            let percent = args
                .get_one::<u8>("percent")
                .expect("percent arg is required");

            client.spotify.volume(*percent, device_id).await?;
        }
        "seek" => {
            let progress_ms = match playback.progress {
                Some(progress) => progress.as_millis(),
                None => {
                    eprintln!("Playback has no progress!");
                    exit(1);
                }
            };
            let position_offset_ms = args
                .get_one::<i32>("position_offset_ms")
                .expect("position_offset_ms is required");

            client
                .spotify
                .seek_track(
                    (progress_ms as u32).saturating_add_signed(*position_offset_ms),
                    device_id,
                )
                .await?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

pub async fn handle_cli_subcommand(cmd: &str, args: &ArgMatches, client: Client) -> Result<()> {
    match cmd {
        "get" => handle_get_subcommand(args, client).await?,
        "playback" => handle_playback_subcommand(args, client).await?,
        _ => unreachable!(),
    }
    Ok(())
}
