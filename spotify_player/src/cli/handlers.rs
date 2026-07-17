use crate::{auth::AuthConfig, client};

use super::{
    config, init_cli,
    ipc::{
        read_frame, write_frame, CONNECT_TIMEOUT, IO_TIMEOUT, MAX_REQUEST_SIZE, MAX_RESPONSE_SIZE,
    },
    start_socket, AlbumId, Command, ContextType, EditAction, GetRequest, IdOrName, ItemType, Key,
    PlaylistCommand, PlaylistId, Request, Response, TrackId,
};
use anyhow::{Context, Result};
use clap::{ArgMatches, Id};
use clap_complete::{generate, Shell};
use std::{
    io,
    net::{SocketAddr, TcpStream},
    time::Duration,
};

fn receive_response(stream: &mut TcpStream) -> Result<Response> {
    let data = read_frame(stream, MAX_RESPONSE_SIZE, "CLI response").with_context(|| {
        format!(
            "receive a complete response within {} seconds",
            IO_TIMEOUT.as_secs()
        )
    })?;
    serde_json::from_slice(&data).context("deserialize CLI response")
}

fn serialize_request(request: &Request) -> Result<Vec<u8>> {
    let data = serde_json::to_vec(request).context("serialize CLI request")?;
    anyhow::ensure!(
        data.len() <= MAX_REQUEST_SIZE,
        "CLI request is {} bytes, but the maximum is {} bytes; shorten the query or text fields",
        data.len(),
        MAX_REQUEST_SIZE
    );
    Ok(data)
}

fn connect_with_timeout(addr: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
    TcpStream::connect_timeout(&addr, timeout)
}

fn configure_stream(stream: TcpStream) -> Result<TcpStream> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .context("set CLI response deadline")?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .context("set CLI request deadline")?;
    stream
        .set_nodelay(true)
        .context("configure CLI connection")?;
    Ok(stream)
}

fn get_id_or_name(args: &ArgMatches) -> IdOrName {
    try_get_id_or_name(args).expect("id_or_name group is required")
}

fn try_get_id_or_name(args: &ArgMatches) -> Option<IdOrName> {
    match args.get_one::<Id>("id_or_name")?.as_str() {
        "name" => Some(IdOrName::Name(
            args.get_one::<String>("name")
                .expect("name should be specified")
                .to_owned(),
        )),
        "id" => Some(IdOrName::Id(
            args.get_one::<String>("id")
                .expect("id should be specified")
                .to_owned(),
        )),
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

/// Tries to connect to a running client using a bounded TCP connection.
/// If no running client found, create a new client running in a separate thread to
/// handle the socket request.
fn try_connect_to_client(configs: &config::Configs) -> Result<TcpStream> {
    let port = configs.app_config.client_port;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    match connect_with_timeout(addr, CONNECT_TIMEOUT) {
        Ok(stream) => configure_stream(stream),
        Err(err) if err.kind() == io::ErrorKind::ConnectionRefused => {
            // no running `spotify_player` instance found,
            // initialize a new client to handle the current CLI command

            let rt = tokio::runtime::Runtime::new()?;

            // create a Spotify API client
            let client = rt
                .block_on(client::AppClient::new())
                .context("construct app client")?;
            rt.block_on(client.new_session(None, false))
                .context("new session")?;

            // Bind before spawning the thread so the caller cannot race the listener startup.
            let client_listener = rt.block_on(tokio::net::TcpListener::bind(addr))?;

            std::thread::spawn(move || {
                rt.block_on(start_socket(&client, None, Some(client_listener)));
            });

            let stream = connect_with_timeout(addr, CONNECT_TIMEOUT).with_context(|| {
                format!(
                    "connect to the new Spotify client within {} seconds",
                    CONNECT_TIMEOUT.as_secs()
                )
            })?;
            configure_stream(stream)
        }
        Err(err) => Err(err).with_context(|| {
            format!(
                "connect to the Spotify client within {} seconds",
                CONNECT_TIMEOUT.as_secs()
            )
        }),
    }
}

pub fn handle_cli_subcommand(cmd: &str, args: &ArgMatches) -> Result<()> {
    let configs = config::get_config();

    // handle commands that don't require a client separately
    match cmd {
        "authenticate" => {
            // Force re-authentication of both credentials the application relies on:
            // the Web API token and the librespot session credentials.
            // Each runs its own interactive OAuth flow under a different client ID.
            let mut api_client = client::new_api_client()?;
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(crate::auth::prompt_for_user_token(&mut api_client, true))
                .context("authenticate Spotify Web API client")?;

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
        "features" => {
            print_features();
            std::process::exit(0);
        }
        _ => {}
    }

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
        "lyrics" => Request::Lyrics {
            id_or_name: try_get_id_or_name(args),
        },
        _ => unreachable!(),
    };

    let request_buf = serialize_request(&request)?;
    let mut stream = try_connect_to_client(configs).context("connect to a Spotify client")?;
    write_frame(&mut stream, &request_buf, MAX_REQUEST_SIZE, "CLI request")
        .with_context(|| format!("send the request within {} seconds", IO_TIMEOUT.as_secs()))?;

    // receive and handle a response from the client's socket
    match receive_response(&mut stream)? {
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
            let playlist_id = PlaylistId::from_id(
                args.get_one::<String>("playlist_id")
                    .expect("playlist_id arg is required")
                    .to_owned(),
            )?;

            let action = *args
                .get_one::<EditAction>("action")
                .expect("action arg is required");

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

macro_rules! print_feature {
    ($feature:literal) => {
        #[cfg(feature = $feature)]
        println!("  ✓ {}", $feature);
        #[cfg(not(feature = $feature))]
        println!("  ✗ {}", $feature);
    };
}

fn print_features() {
    println!("Compile-time features:");

    print_feature!("daemon");
    print_feature!("streaming");
    print_feature!("media-control");
    print_feature!("image");
    print_feature!("ratatui-image");
    print_feature!("sixel");
    print_feature!("pixelate");
    print_feature!("notify");
    print_feature!("fzf");

    // Audio backends
    print_feature!("pulseaudio-backend");
    print_feature!("alsa-backend");
    print_feature!("rodio-backend");
    print_feature!("jackaudio-backend");
    print_feature!("sdl-backend");
    print_feature!("gstreamer-backend");
}

#[cfg(test)]
mod tests {
    use std::{
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    use super::{connect_with_timeout, receive_response, serialize_request, Request};
    use crate::cli::ipc::MAX_REQUEST_SIZE;

    #[test]
    fn oversized_user_input_returns_an_error() {
        let request = Request::Search {
            query: "x".repeat(MAX_REQUEST_SIZE),
        };

        let error = serialize_request(&request).unwrap_err();

        assert!(error.to_string().contains("maximum"));
        assert!(error.to_string().contains("shorten"));
    }

    #[test]
    fn missing_server_connection_fails_within_the_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let result = connect_with_timeout(addr, Duration::from_millis(100));

        assert!(result.is_err());
    }

    #[test]
    fn stalled_server_response_returns_an_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(150));
        });
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(25)))
            .unwrap();

        let error = receive_response(&mut stream).unwrap_err();

        assert!(error.to_string().contains("complete response"));
        server.join().unwrap();
    }
}
