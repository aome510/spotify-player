use super::*;
use anyhow::Result;
use clap::ArgMatches;
use std::net::UdpSocket;

async fn handle_get_subcommand(args: &ArgMatches, socket: UdpSocket) -> Result<()> {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");

    let request = match cmd {
        "key" => {
            let key = args
                .get_one::<Key>("key")
                .expect("key is required")
                .to_owned();
            Request::Get(GetRequest::Key(key))
        }
        "context" => {
            let context_id = args
                .get_one::<String>("context_id")
                .expect("context_id is required")
                .to_owned();
            let context_type = args
                .get_one::<ContextType>("context_type")
                .expect("context_type is required")
                .to_owned();
            Request::Get(GetRequest::Context(context_id, context_type))
        }
        _ => unreachable!(),
    };

    socket.send(&serde_json::to_vec(&request)?)?;
    let mut buf = [0; 4096];
    let n_bytes = socket.recv(&mut buf)?;
    println!("{}", String::from_utf8_lossy(&buf[..n_bytes]));

    Ok(())
}

// async fn handle_playback_subcommand(args: &ArgMatches, client: Client) -> Result<()> {
//     let playback = match client
//         .spotify
//         .current_playback(None, None::<Vec<_>>)
//         .await?
//     {
//         Some(playback) => playback,
//         None => {
//             eprintln!("No playback found!");
//             exit(1);
//         }
//     };
//     let device_id = playback.device.id.as_deref();

//     let (cmd, args) = args.subcommand().expect("playback subcommand is required");
//     match cmd {
//         "start" => {
//             let context_id = args
//                 .get_one::<String>("context_id")
//                 .expect("context_id is required");
//             let context_type = args
//                 .get_one::<ContextType>("context_type")
//                 .expect("context_type is required");

//             let context_id = match context_type {
//                 ContextType::Playlist => PlayContextId::Playlist(PlaylistId::from_id(context_id)?),
//                 ContextType::Album => PlayContextId::Album(AlbumId::from_id(context_id)?),
//                 ContextType::Artist => PlayContextId::Artist(ArtistId::from_id(context_id)?),
//             };

//             client
//                 .spotify
//                 .start_context_playback(context_id, device_id, None, None)
//                 .await?;

//             // for some reasons, when starting a new playback, the integrated `spotify-player`
//             // client doesn't respect the initial shuffle state, so we need to manually update the state
//             client
//                 .spotify
//                 .shuffle(playback.shuffle_state, device_id)
//                 .await?
//         }
//         "play-pause" => {
//             if playback.is_playing {
//                 client.spotify.pause_playback(device_id).await?;
//             } else {
//                 client.spotify.resume_playback(device_id, None).await?;
//             }
//         }
//         "next" => {
//             client.spotify.next_track(device_id).await?;
//         }
//         "previous" => {
//             client.spotify.previous_track(device_id).await?;
//         }
//         "shuffle" => {
//             client
//                 .spotify
//                 .shuffle(!playback.shuffle_state, device_id)
//                 .await?;
//         }
//         "repeat" => {
//             let next_repeat_state = match playback.repeat_state {
//                 RepeatState::Off => RepeatState::Track,
//                 RepeatState::Track => RepeatState::Context,
//                 RepeatState::Context => RepeatState::Off,
//             };

//             client.spotify.repeat(next_repeat_state, device_id).await?;
//         }
//         "volume" => {
//             let percent = args
//                 .get_one::<u8>("percent")
//                 .expect("percent arg is required");

//             client.spotify.volume(*percent, device_id).await?;
//         }
//         "seek" => {
//             let progress_ms = match playback.progress {
//                 Some(progress) => progress.as_millis(),
//                 None => {
//                     eprintln!("Playback has no progress!");
//                     exit(1);
//                 }
//             };
//             let position_offset_ms = args
//                 .get_one::<i32>("position_offset_ms")
//                 .expect("position_offset_ms is required");

//             client
//                 .spotify
//                 .seek_track(
//                     (progress_ms as u32).saturating_add_signed(*position_offset_ms),
//                     device_id,
//                 )
//                 .await?;
//         }
//         _ => unreachable!(),
//     }
//     Ok(())
// }

pub async fn handle_cli_subcommand(cmd: &str, args: &ArgMatches, client_port: u16) -> Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.connect(("127.0.0.1", client_port))?;

    match cmd {
        "get" => handle_get_subcommand(args, socket).await?,
        // "playback" => handle_playback_subcommand(args, client).await?,
        _ => unreachable!(),
    }
    Ok(())
}
