use crate::client::Client;
use anyhow::Result;
use rspotify::prelude::*;

pub fn init_cli_command() -> clap::Command {
    clap::Command::new("cli")
        .about("cli to interact with a running instance")
        .subcommand(clap::Command::new("get").arg(clap::Arg::new("key")))
}

async fn handle_get_subcommand(args: &clap::ArgMatches, client: Client) -> Result<()> {
    if let Some(key) = args.get_one::<String>("key") {
        match key.as_str() {
            "playback" => {
                let playback = client
                    .spotify
                    .current_playback(None, None::<Vec<_>>)
                    .await?;
                println!("playback: {playback:?}")
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
