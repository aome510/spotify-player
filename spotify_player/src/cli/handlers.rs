use super::*;
use anyhow::Result;
use clap::ArgMatches;
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
    let data = receive_data(&socket)?;
    println!("{}", String::from_utf8_lossy(&data));

    Ok(())
}

fn handle_playback_subcommand(args: &ArgMatches, socket: UdpSocket) -> Result<()> {
    let (cmd, args) = args.subcommand().expect("playback subcommand is required");
    let command = match cmd {
        "start" => {
            let context_id = args
                .get_one::<String>("context_id")
                .expect("context_id is required");
            let context_type = args
                .get_one::<ContextType>("context_type")
                .expect("context_type is required");
            Command::Start(context_id.to_owned(), context_type.to_owned())
        }
        "play-pause" => Command::PlayPause,
        "next" => Command::Next,
        "previous" => Command::Previous,
        "shuffle" => Command::Shuffle,
        "repeat" => Command::Repeat,
        "volume" => {
            let percent = args
                .get_one::<u8>("percent")
                .expect("percent arg is required");
            Command::Volume(*percent)
        }
        "seek" => {
            let position_offset_ms = args
                .get_one::<i32>("position_offset_ms")
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
