use super::*;
use anyhow::Result;
use clap::{ArgMatches, Id};
use std::net::UdpSocket;

fn receive_response(socket: &UdpSocket) -> Result<Response> {
    // read response from the server's socket, which can be splitted into
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

fn get_id_or_name(args: &ArgMatches) -> Result<IdOrName> {
    match args
        .get_one::<Id>("id_or_name")
        .expect("id_or_name group is required")
        .as_str()
    {
        "name" => Ok(IdOrName::Name(
            args.get_one::<String>("name")
                .expect("name should be specified")
                .to_owned(),
        )),
        "id" => Ok(IdOrName::Id(
            args.get_one::<String>("id")
                .expect("id should be specified")
                .to_owned(),
        )),
        id => anyhow::bail!("unknown id: {id}"),
    }
}

fn handle_get_subcommand(args: &ArgMatches, socket: &UdpSocket) -> Result<()> {
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
            let context_type = args
                .get_one::<ContextType>("context_type")
                .expect("context_type is required")
                .to_owned();
            let id_or_name = get_id_or_name(args)?;
            Request::Get(GetRequest::Context(context_type, id_or_name))
        }
        _ => unreachable!(),
    };

    socket.send(&serde_json::to_vec(&request)?)?;
    Ok(())
}

fn handle_playback_subcommand(args: &ArgMatches, socket: &UdpSocket) -> Result<()> {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");
    let command = match cmd {
        "start" => match args.subcommand() {
            Some(("context", args)) => {
                let context_type = args
                    .get_one::<ContextType>("context_type")
                    .expect("context_type is required")
                    .to_owned();
                let id_or_name = get_id_or_name(args)?;
                Command::StartContext(context_type, id_or_name)
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
                let id_or_name = get_id_or_name(args)?;
                Command::StartRadio(item_type, id_or_name)
            }
            _ => {
                anyhow::bail!("invalid command!");
            }
        },
        "play-pause" => Command::PlayPause,
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

    let request = Request::Playback(command);
    socket.send(&serde_json::to_vec(&request)?)?;

    Ok(())
}

fn handle_connect_subcommand(args: &ArgMatches, socket: &UdpSocket) -> Result<()> {
    let id_or_name = get_id_or_name(args)?;

    let request = Request::Connect(id_or_name);
    socket.send(&serde_json::to_vec(&request)?)?;

    Ok(())
}

fn handle_like_subcommand(args: &ArgMatches, socket: &UdpSocket) -> Result<()> {
    let unlike = args.get_flag("unlike");

    let request = Request::Like { unlike };
    socket.send(&serde_json::to_vec(&request)?)?;

    Ok(())
}

pub fn handle_cli_subcommand(cmd: &str, args: &ArgMatches, client_port: u16) -> Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.connect(("127.0.0.1", client_port))?;

    match cmd {
        "get" => handle_get_subcommand(args, &socket)?,
        "playback" => handle_playback_subcommand(args, &socket)?,
        "connect" => handle_connect_subcommand(args, &socket)?,
        "like" => handle_like_subcommand(args, &socket)?,
        _ => unreachable!(),
    }

    match receive_response(&socket)? {
        Response::Err(err) => {
            eprint!("{}", String::from_utf8_lossy(&err));
            std::process::exit(1);
        }
        Response::Ok(data) => {
            print!("{}", String::from_utf8_lossy(&data));
            std::process::exit(0);
        }
    }
}
