//! Find lyric for songs.
//!
//! This crate provides Rust APIs to retrieve a song lyric.

use anyhow::Result;

const BASE_REQUEST_URL: &str = "https://genius.com/api/search/multi?per_page=5";

pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn get_lyric<S>(&self, query: S) -> Result<String>
    where
        S: std::fmt::Display,
    {
        let text = self
            .http
            .get(format!("{}&q={}", BASE_REQUEST_URL, query))
            .send()
            .await?
            .text()
            .await?;

        Ok(text)
    }
}
