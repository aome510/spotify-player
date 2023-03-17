use crate::client::Client;
use anyhow::Result;
use clap::builder::PossibleValue;
use rspotify::prelude::*;

pub fn init_cli_command() -> clap::Command {
    clap::Command::new("cli")
        .about("cli to interact with a running instance")
        .subcommand(
            clap::Command::new("get").arg(clap::Arg::new("key").value_parser([
                PossibleValue::new("playback"),
                PossibleValue::new("devices"),
                PossibleValue::new("user_playlists"),
                PossibleValue::new("user_liked_tracks"),
                PossibleValue::new("user_top_tracks"),
                PossibleValue::new("queue"),
            ])),
        )
}

async fn handle_get_subcommand(args: &clap::ArgMatches, client: Client) -> Result<()> {
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

pub async fn handle_cli_command(args: &clap::ArgMatches, client: Client) -> Result<()> {
    match args.subcommand() {
        None => {}
        Some((cmd, args)) => match cmd {
            "get" => handle_get_subcommand(args, client).await?,
            _ => {
                println!("Unknown subcommand: {cmd}!");
            }
        },
    }
    Ok(())
}
