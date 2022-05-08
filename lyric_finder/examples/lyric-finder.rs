extern crate lyric_finder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    if args.len() < 2 {
        println!("Please specify the first argument to be the search query");
        std::process::exit(1);
    }

    let client = lyric_finder::Client::new();
    let lyric = client.get_lyric(&args[1]).await?;
    println!("lyric: {}", lyric);

    Ok(())
}
