extern crate lyric_finder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    let client = lyric_finder::Client::new();
    let lyric = client.get_lyric(&args[1]).await?;
    println!("lyric: {}", lyric);

    Ok(())
}
