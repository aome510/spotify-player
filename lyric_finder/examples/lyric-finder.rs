extern crate lyric_finder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = lyric_finder::Client::new();
    let lyric = client.get_lyric("bts").await?;
    println!("lyric: {}", lyric);

    Ok(())
}
