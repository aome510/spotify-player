//! Find lyric for songs.
//!
//! This crate provides Rust APIs to retrieve a song lyric.

use anyhow::Result;

const SEARCH_BASE_URL: &str = "https://genius.com/api/search";

pub struct Client {
    http: reqwest::Client,
}

impl Client {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn search_lyric_urls(&self, query: &str) -> Result<Vec<String>> {
        let body = self
            .http
            .get(format!("{SEARCH_BASE_URL}?q={query}"))
            .send()
            .await?
            .json::<search::Body>()
            .await?;

        if body.meta.status != 200 {
            let message = match body.meta.message {
                Some(m) => m,
                None => format!("request failed with status code: {}", body.meta.status),
            };
            return Err(anyhow::anyhow!(message));
        }

        let urls = body.response.map(|r| {
            r.hits
                .into_iter()
                .filter(|hit| hit.ty == "song")
                .map(|hit| hit.result.url)
                .collect::<Vec<_>>()
        });

        match urls {
            Some(v) => Ok(v),
            None => Err(anyhow::anyhow!("lyric not found for query {}", query)),
        }
    }

    pub async fn get_lyric(&self, query: &str) -> Result<String> {
        let urls = self.search_lyric_urls(query).await?;
        println!("get a URL: {urls:?}");
        Ok(String::new())
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

mod search {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct Body {
        pub meta: Metadata,
        pub response: Option<Response>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Metadata {
        pub status: u16,
        pub message: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Response {
        pub hits: Vec<Hit>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Hit {
        #[serde(rename(deserialize = "type"))]
        pub ty: String,
        pub result: Result,
    }

    #[derive(Debug, Deserialize)]
    pub struct Result {
        pub url: String,
    }
}
