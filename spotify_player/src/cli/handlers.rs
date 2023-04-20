use anyhow::{Context, Result};
use std::process::exit;

use clap::ArgMatches;
use rspotify::{model::*, prelude::OAuthClient};

use crate::{cli::ContextType, client::Client};

use super::{ClientSocket, Key};

impl ClientSocket for Client {
    fn start_socket(&self, port: u16) -> Result<()> {
        tracing::info!("Starting a client socket at 127.0.0.1:{port}");

        let socket = std::net::UdpSocket::bind(("127.0.0.1", port))
            .context(format!("failed to bind a new socket to port {port}"))?;

        // handle socket's messages in a thread
        tokio::task::spawn_blocking({
            let client = self.clone();

            move || {
                // initialize the receive buffer to be 4096 bytes
                let mut buf = [0; 4096];
                loop {
                    if socket.recv_from(&mut buf).is_ok() {
                        tracing::info!("received {buf:?} from the socket")
                    }
                }
            }
        });

        Ok(())
    }

    fn handle_request(&self, request: super::Request) -> Result<()> {
        todo!()
    }
}

async fn handle_get_key_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
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

async fn handle_get_context_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
    let context_id = args
        .get_one::<String>("context_id")
        .expect("context_id is required");
    let context_type = args
        .get_one::<ContextType>("context_type")
        .expect("context_type is required");

    let context = match context_type {
        ContextType::Playlist => {
            let id = PlaylistId::from_id(context_id)?;
            client.playlist_context(id).await?
        }
        ContextType::Album => {
            let id = AlbumId::from_id(context_id)?;
            client.album_context(id).await?
        }
        ContextType::Artist => {
            let id = ArtistId::from_id(context_id)?;
            client.artist_context(id).await?
        }
    };

    println!("{}", serde_json::to_string(&context)?);

    Ok(())
}

async fn handle_get_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");

    match cmd {
        "key" => handle_get_key_subcommand(args, client).await?,
        "context" => handle_get_context_subcommand(args, client).await?,
        _ => unreachable!(),
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
        "start" => {
            let context_id = args
                .get_one::<String>("context_id")
                .expect("context_id is required");
            let context_type = args
                .get_one::<ContextType>("context_type")
                .expect("context_type is required");

            let context_id = match context_type {
                ContextType::Playlist => PlayContextId::Playlist(PlaylistId::from_id(context_id)?),
                ContextType::Album => PlayContextId::Album(AlbumId::from_id(context_id)?),
                ContextType::Artist => PlayContextId::Artist(ArtistId::from_id(context_id)?),
            };

            client
                .spotify
                .start_context_playback(context_id, device_id, None, None)
                .await?;

            // for some reasons, when starting a new playback, the integrated `spotify-player`
            // client doesn't respect the initial shuffle state, so we need to manually update the state
            client
                .spotify
                .shuffle(playback.shuffle_state, device_id)
                .await?
        }
        "play-pause" => {
            if playback.is_playing {
                client.spotify.pause_playback(device_id).await?;
            } else {
                client.spotify.resume_playback(device_id, None).await?;
            }
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
