use crate::{auth::AuthConfig, client};

use super::{
    config, init_cli, start_socket, Command, ContextType, EditAction, GetRequest, IdOrName, ItemType, Key,
    PlaylistCommand, PlaylistId, Request, Response, TrackId, AlbumId, MAX_REQUEST_SIZE,
};
use anyhow::{Context, Result};
use clap::{ArgMatches, Id};
use clap_complete::{generate, Shell};
use std::net::UdpSocket;

fn receive_response(socket: &UdpSocket) -> Result<Response> {
    // read response from the server's socket, which can be split into
    // smaller chunks of data
    let mut data = Vec::new();
    let mut buf = [0; 4096];
    loop {
        let (n_bytes, _) = socket.recv_from(&mut buf)?;
        if n_bytes == 0 {
            // end of chunk
            break;
        }
        data.extend_from_slice(&buf[..n_bytes]);
    }

    Ok(serde_json::from_slice(&data)?)
}

fn get_id_or_name(args: &ArgMatches) -> IdOrName {
    match args
        .get_one::<Id>("id_or_name")
        .expect("id_or_name group is required")
        .as_str()
    {
        "name" => IdOrName::Name(
            args.get_one::<String>("name")
                .expect("name should be specified")
                .to_owned(),
        ),
        "id" => IdOrName::Id(
            args.get_one::<String>("id")
                .expect("id should be specified")
                .to_owned(),
        ),
        id => panic!("unknown id: {id}"),
    }
}

fn handle_get_subcommand(args: &ArgMatches) -> Request {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");

    let request = match cmd {
        "key" => {
            let key = args
                .get_one::<Key>("key")
                .expect("key is required")
                .to_owned();
            Request::Get(GetRequest::Key(key))
        }
        "item" => {
            let item_type = args
                .get_one::<ItemType>("item_type")
                .expect("context_type is required")
                .to_owned();
            let id_or_name = get_id_or_name(args);
            Request::Get(GetRequest::Item(item_type, id_or_name))
        }
        _ => unreachable!(),
    };

    request
}

fn handle_playback_subcommand(args: &ArgMatches) -> Result<Request> {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");
    let command = match cmd {
        "start" => match args.subcommand() {
            Some(("track", args)) => Command::StartTrack(get_id_or_name(args)),
            Some(("context", args)) => {
                let context_type = args
                    .get_one::<ContextType>("context_type")
                    .expect("context_type is required")
                    .to_owned();
                let shuffle = args.get_flag("shuffle");

                let id_or_name = get_id_or_name(args);
                Command::StartContext {
                    context_type,
                    id_or_name,
                    shuffle,
                }
            }
            Some(("liked", args)) => {
                let limit = *args
                    .get_one::<usize>("limit")
                    .expect("limit should have a default value");
                let random = args.get_flag("random");
                Command::StartLikedTracks { limit, random }
            }
            Some(("radio", args)) => {
                let item_type = args
                    .get_one::<ItemType>("item_type")
                    .expect("item_type is required")
                    .to_owned();
                let id_or_name = get_id_or_name(args);
                Command::StartRadio(item_type, id_or_name)
            }
            _ => {
                anyhow::bail!("invalid command!");
            }
        },
        "play-pause" => Command::PlayPause,
        "play" => Command::Play,
        "pause" => Command::Pause,
        "next" => Command::Next,
        "previous" => Command::Previous,
        "shuffle" => Command::Shuffle,
        "repeat" => Command::Repeat,
        "volume" => {
            let percent = args
                .get_one::<i8>("percent")
                .expect("percent arg is required");
            let offset = args.get_flag("offset");
            Command::Volume {
                percent: *percent,
                is_offset: offset,
            }
        }
        "seek" => {
            let position_offset_ms = args
                .get_one::<i64>("position_offset_ms")
                .expect("position_offset_ms is required");
            Command::Seek(*position_offset_ms)
        }
        _ => unreachable!(),
    };

    Ok(Request::Playback(command))
}

/// Tries to connect to a running client, if exists, by sending a connection request
/// to the client via a UDP socket.
/// If no running client found, create a new client running in a separate thread to
/// handle the socket request.
fn try_connect_to_client(socket: &UdpSocket, configs: &config::Configs) -> Result<()> {
    let port = configs.app_config.client_port;
    socket.connect(("127.0.0.1", port))?;

    // send an empty buffer as a connection request to the client
    socket.send(&[])?;
    if let Err(err) = socket.recv(&mut [0; 1]) {
        if let std::io::ErrorKind::ConnectionRefused = err.kind() {
            // no running `spotify_player` instance found,
            // initialize a new client to handle the current CLI command

            let auth_config = AuthConfig::new(configs)?;
            let rt = tokio::runtime::Runtime::new()?;

            // create a Spotify API client
            let client = client::Client::new(auth_config);
            rt.block_on(client.new_session(None, false))
                .context("new session")?;

            // create a client socket for handling CLI commands
            let client_socket = rt.block_on(tokio::net::UdpSocket::bind(("127.0.0.1", port)))?;

            // spawn a thread to handle the CLI request
            std::thread::spawn(move || rt.block_on(start_socket(client, client_socket, None)));
        } else {
            return Err(err.into());
        }
    }

    Ok(())
}

pub fn handle_cli_subcommand(cmd: &str, args: &ArgMatches) -> Result<()> {
    let configs = config::get_config();

    // handle commands that don't require a client separately
    match cmd {
        "authenticate" => {
            let auth_config = AuthConfig::new(configs)?;
            crate::auth::get_creds(&auth_config, true, false)?;
            std::process::exit(0);
        }
        "generate" => {
            let gen = *args
                .get_one::<Shell>("shell")
                .expect("shell argument is required");
            let mut cmd = init_cli()?;
            let name = cmd.get_name().to_string();
            generate(gen, &mut cmd, name, &mut std::io::stdout());
            std::process::exit(0);
        }
        _ => {}
    }

    let socket = UdpSocket::bind("127.0.0.1:0")?;
    try_connect_to_client(&socket, configs).context("try to connect to a client")?;

    // construct a socket request based on the CLI command and its arguments
    let request = match cmd {
        "get" => handle_get_subcommand(args),
        "playback" => handle_playback_subcommand(args)?,
        "playlist" => handle_playlist_subcommand(args)?,
        "connect" => Request::Connect(get_id_or_name(args)),
        "like" => Request::Like {
            unlike: args.get_flag("unlike"),
        },
        "search" => Request::Search {
            query: args
                .get_one::<String>("query")
                .expect("query is required")
                .to_owned(),
        },
        _ => unreachable!(),
    };

    // send the request to the client's socket
    let request_buf = serde_json::to_vec(&request)?;
    assert!(request_buf.len() <= MAX_REQUEST_SIZE);
    socket.send(&request_buf)?;

    // receive and handle a response from the client's socket
    match receive_response(&socket)? {
        Response::Err(err) => {
            eprintln!("{}", String::from_utf8_lossy(&err));
            std::process::exit(1);
        }
        Response::Ok(data) => {
            println!("{}", String::from_utf8_lossy(&data).replace("\\n", "\n"));
            std::process::exit(0);
        }
    }
}

fn handle_playlist_subcommand(args: &ArgMatches) -> Result<Request> {
    let (cmd, args) = args.subcommand().expect("playlist subcommand is required");
    let command = match cmd {
        "new" => {
            let name = args
                .get_one::<String>("name")
                .expect("name arg is required")
                .to_owned();

            let description = args
                .get_one::<String>("description")
                .map(std::borrow::ToOwned::to_owned)
                .unwrap_or_default();

            let public = args.get_flag("public");
            let collab = args.get_flag("collab");

            PlaylistCommand::New {
                name,
                public,
                collab,
                description,
            }
        }
        "delete" => {
            let id = args
                .get_one::<String>("id")
                .expect("id arg is required")
                .to_owned();

            let pid = PlaylistId::from_id(id)?;

            PlaylistCommand::Delete { id: pid }
        }
        "list" => PlaylistCommand::List,
        "import" => {
            let from_s = args
                .get_one::<String>("from")
                .expect("'from' PlaylistID is required.")
                .to_owned();

            let to_s = args
                .get_one::<String>("to")
                .expect("'to' PlaylistID is required.")
                .to_owned();

            let delete = args.get_flag("delete");

            let from = PlaylistId::from_id(from_s.clone())?;
            let to = PlaylistId::from_id(to_s.clone())?;

            println!("Importing '{from_s}' into '{to_s}'...\n");
            PlaylistCommand::Import { from, to, delete }
        }
        "fork" => {
            let id_s = args
                .get_one::<String>("id")
                .expect("Playlist id is required.")
                .to_owned();

            let id = PlaylistId::from_id(id_s.clone())?;

            println!("Forking '{id_s}'...\n");
            PlaylistCommand::Fork { id }
        }
        "sync" => {
            let id_s = args.get_one::<String>("id");
            let delete = args.get_flag("delete");

            let pid = if let Some(id_s) = id_s {
                println!("Syncing imports for playlist '{id_s}'...\n");
                Some(PlaylistId::from_id(id_s.to_owned())?)
            } else {
                println!("Syncing imports for all playlists...\n");
                None
            };

            PlaylistCommand::Sync { id: pid, delete }
        }
        "edit" => {
            let playlist_id_str = args
                .get_one::<String>("playlist_id")
                .expect("playlist_id arg is required")
                .to_owned();

            let action_str = args
                .get_one::<String>("action")
                .expect("action arg is required");

            let playlist_id = PlaylistId::from_id(playlist_id_str)?;
            
            let action = match action_str.as_str() {
                "add" => EditAction::Add,
                "delete" => EditAction::Delete,
                _ => unreachable!(),
            };

            let track_id = args
                .get_one::<String>("track_id")
                .map(|s| TrackId::from_id(s.to_owned()))
                .transpose()?;

            let album_id = args
                .get_one::<String>("album_id")
                .map(|s| AlbumId::from_id(s.to_owned()))
                .transpose()?;

            PlaylistCommand::Edit {
                playlist_id,
                action,
                track_id,
                album_id,
            }
        }
        _ => unreachable!(),
    };

    Ok(Request::Playlist(command))
}
