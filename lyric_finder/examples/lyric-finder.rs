extern crate lyric_finder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = std::env::args().collect::<Vec<_>>();

    if args.len() < 2 {
        println!("Please specify the first argument to be the search query");
        std::process::exit(1);
    }

    let client = lyric_finder::Client::new();
    let result = client.get_lyric(&args[1]).await?;
    match result {
        lyric_finder::LyricResult::Some {
            track,
            artists,
            lyric,
        } => {
            println!("{} by {}'s lyric:\n{}", track, artists, lyric);
        }
        lyric_finder::LyricResult::None => {
            println!("lyric not found!");
        }
    }

    Ok(())
}
