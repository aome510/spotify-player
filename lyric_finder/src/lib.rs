//! # lyric_finder
//!
//! This crate provides a [`Client`](Client) struct for retrieving a song's lyric.
//!
//! It ultilizes the [Genius](https://genius.com) website and its APIs to get lyric data.
//!
//! ## Example
//!
//! ```rust
//! # use anyhow::Result;
//! #
//! # async fn run() -> Result<()> {
//! let client =  lyric_finder::Client::new();
//! let result = client.get_lyric("shape of you").await?;
//! println!(
//!     "{} by {}'s lyric:\n{}",
//!     result.title, result.artist_names, result.lyric
//! );
//! # Ok(())
//! # }
//! ```

const SEARCH_BASE_URL: &str = "https://genius.com/api/search";

pub struct Client {
    http: reqwest::Client,
}

#[derive(Debug)]
pub enum LyricResult {
    Some {
        track: String,
        artists: String,
        lyric: String,
    },
    None,
}

impl Client {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Construct a client reusing an exisiting http client
    pub fn from_http_client(http: &reqwest::Client) -> Self {
        Self { http: http.clone() }
    }

    /// Search songs satisfying a given `query`.
    pub async fn search_songs(&self, query: &str) -> anyhow::Result<Vec<search::Result>> {
        log::debug!("search songs: query={query}");

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
            anyhow::bail!(message);
        }

        Ok(body
            .response
            .map(|r| {
                r.hits
                    .into_iter()
                    .filter(|hit| hit.ty == "song")
                    .map(|hit| hit.result)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default())
    }

    /// Retrieve a song's lyric from a "genius.com" `url`.
    pub async fn retrieve_lyric(&self, url: &str) -> anyhow::Result<String> {
        let html = self.http.get(url).send().await?.text().await?;
        log::debug!("retrieve lyric from url={url}: html={html}");
        let lyric = parse::parse(html)?;
        Ok(lyric.trim().to_string())
    }

    /// Process a lyric obtained by crawling the [Genius](https://genius.com) website.
    ///
    /// The lyric received this way may have weird newline spacings between sections (*).
    /// The below function tries an ad-hoc method to fix this issue.
    ///
    /// (*): A section often starts with `[`.
    fn process_lyric(lyric: String) -> String {
        // the below code modifies the `lyric` to make the newline between sections consistent
        lyric.replace("\n\n[", "\n[").replace("\n[", "\n\n[")
    }

    /// Get the lyric of a song satisfying a given `query`.
    pub async fn get_lyric(&self, query: &str) -> anyhow::Result<LyricResult> {
        // The function first searches songs satisfying the query
        // then it retrieves the song's lyric by crawling the "genius.com" website.

        let result = {
            let mut results = self.search_songs(query).await?;
            log::debug!("search results: {results:?}");
            if results.is_empty() {
                return Ok(LyricResult::None);
            }
            results.remove(0)
        };

        let lyric = self.retrieve_lyric(&result.url).await?;
        Ok(LyricResult::Some {
            track: result.title,
            artists: result.artist_names,
            lyric: Self::process_lyric(lyric),
        })
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

mod parse {
    use html5ever::tendril::TendrilSink;
    use html5ever::*;
    use markup5ever_rcdom::{Handle, NodeData, RcDom};

    const LYRIC_CONTAINER_ATTR: &str = "data-lyrics-container";

    /// Parse the HTML content of a "genius.com" lyric page to retrieve the corresponding lyric.
    pub fn parse(html: String) -> anyhow::Result<String> {
        // parse HTML content into DOM node(s)
        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut (html.as_bytes()))?;

        let filter = |data: &NodeData| match data {
            NodeData::Element { ref attrs, .. } => attrs
                .borrow()
                .iter()
                .any(|attr| attr.name.local.to_string() == LYRIC_CONTAINER_ATTR),
            _ => false,
        };

        Ok(parse_dom_node(dom.document, &Some(filter), false))
    }

    /// Parse a dom node and extract the text of children nodes satisfying a requirement.
    ///
    /// The requirement is represented by a `filter` function and a `should_parse` variable.
    /// Once a node satisfies a requirement, its children should also satisfy it.
    fn parse_dom_node<F>(node: Handle, filter: &Option<F>, mut should_parse: bool) -> String
    where
        F: Fn(&NodeData) -> bool,
    {
        log::debug!("parse dom node: node={node:?}, should_parse={should_parse}");

        let mut s = String::new();

        if !should_parse {
            if let Some(f) = filter {
                should_parse = f(&node.data);
            }
        }

        match &node.data {
            NodeData::Text { contents } => {
                if should_parse {
                    s.push_str(&contents.borrow().to_string());
                }
            }
            NodeData::Element { ref name, .. } => {
                if let expanded_name!(html "br") = name.expanded() {
                    if should_parse {
                        s.push('\n');
                    }
                }
            }
            _ => {}
        }

        node.children.borrow().iter().for_each(|node| {
            s.push_str(&parse_dom_node(node.clone(), filter, should_parse));
        });

        s
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
        pub title: String,
        pub artist_names: String,
    }
}
