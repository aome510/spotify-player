use super::*;
use anyhow::Result;
use clap::{ArgMatches, Id};
use std::net::UdpSocket;

fn receive_data(socket: &UdpSocket) -> Result<Vec<u8>> {
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

    Ok(data)
}

fn get_context_id(args: &ArgMatches) -> Result<ContextId> {
    let id = args
        .get_one::<Id>("context")
        .expect("context group is required");

    match id.as_str() {
        "name" => Ok(ContextId::Name(
            args.get_one::<String>("name")
                .expect("name should be specified")
                .to_owned(),
        )),
        "id" => Ok(ContextId::Id(
            args.get_one::<String>("id")
                .expect("id should be specified")
                .to_owned(),
        )),
        id => anyhow::bail!("unknown id: {id}"),
    }
}

fn handle_get_subcommand(args: &ArgMatches, socket: UdpSocket) -> Result<()> {
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
            let context_id = get_context_id(args)?;
            Request::Get(GetRequest::Context(context_type, context_id))
        }
        _ => unreachable!(),
    };

    socket.send(&serde_json::to_vec(&request)?)?;
    let data = receive_data(&socket)?;
    println!("{}", String::from_utf8_lossy(&data));

    Ok(())
}

fn handle_playback_subcommand(args: &ArgMatches, socket: UdpSocket) -> Result<()> {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");
    let command = match cmd {
        "start" => match args.subcommand() {
            Some(("context", args)) => {
                let context_type = args
                    .get_one::<ContextType>("context_type")
                    .expect("context_type is required")
                    .to_owned();
                let context_id = get_context_id(args)?;
                Command::Start(context_type, context_id)
            }
            _ => unimplemented!(),
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
            Command::Volume(*percent, offset)
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

pub fn handle_cli_subcommand(cmd: &str, args: &ArgMatches, client_port: u16) -> Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.connect(("127.0.0.1", client_port))?;

    match cmd {
        "get" => handle_get_subcommand(args, socket),
        "playback" => handle_playback_subcommand(args, socket),
        _ => unreachable!(),
    }
}
