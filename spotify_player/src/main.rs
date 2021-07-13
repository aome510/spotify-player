use anyhow::Result;
use rspotify::oauth2::SpotifyOAuth;

const SCOPES: [&str; 10] = [
    "user-read-recently-played",
    "user-top-read",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "streaming",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
];

mod client;
mod config;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config_folder = config::get_config_folder_path()?;
    let client_config = config::ClientConfig::from_config_file(config_folder)?;

    let oauth = SpotifyOAuth::default()
        .client_id(&client_config.client_id)
        .client_secret(&client_config.client_secret)
        .redirect_uri("http://localhost:8888/callback")
        .cache_path(config::get_token_cache_file_path()?)
        .scope(&SCOPES.join(" "))
        .build();

    let client = client::Client::new(oauth).await?;
    let result = client.get_currently_playing().await;
    println!("{:?}", result);

    Ok(())
}
