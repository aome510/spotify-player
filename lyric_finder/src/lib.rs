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
//! match result {
//!     lyric_finder::LyricResult::Some {
//!         track,
//!         artists,
//!         lyric,
//!     } => {
//!         println!("{} by {}'s lyric:\n{}", track, artists, lyric);
//!     }
//!     lyric_finder::LyricResult::None => {
//!         println!("lyric not found!");
//!     }
//! }
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

    /// Construct a client reusing an existing http client
    pub fn from_http_client(http: &reqwest::Client) -> Self {
        Self { http: http.clone() }
    }

    /// Search songs satisfying a given `query`.
    pub async fn search_songs(&self, query: &str) -> anyhow::Result<Vec<search::Result>> {
        let query = improve_query(query);

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

/// Returns `query` without `remaster` & `remix` information from track/artist query.
/// Returned value is lowercase.
/// These caused wildly invalid lyrics to be found.
/// (try yourself adding remastered 2011 to a song's name when searching in Genius!)
fn improve_query(query: &str) -> String {
    // flag for doing something wrong if the song name (after removing remix metadata) is too short.
    const SONG_MIN_LENGTH_WO_REMIX_METADATA: usize = 3;

    let is_dash = |c: char| c == '-';

    // reverse finder for non-filler (space, dashes) chars before an index.
    // Acts like a trim to remove undesired spaces and dashes.
    let rfind_non_filler = |s: &str, idx: usize| {
        let Some(s) = s.get(..idx) else { return idx };
        s.char_indices()
            .rfind(|(_, c)| !(is_dash(*c) || c.is_whitespace()))
            .map_or(idx, |(idx, c)| idx + c.len_utf8())
    };
    // used to handle longer variants of words: `remixed`, `remastered`, etc.
    let end_of_word = |s: &str, idx: usize| {
        let Some(s) = s.get(idx..) else { return idx };
        s.find(|c: char| !c.is_alphanumeric())
            .map_or(idx, |found| found + idx)
    };

    let mut query = query.to_lowercase();
    // remove "xxxx Remaster" from the query
    // For example, `{song} xxxx Remastered {artists}` becomes `{song} {artists}`.
    if let Some(remaster_start) = query.find("remaster") {
        let end = remaster_start + "remaster".len();
        let end = end_of_word(&query, end);

        let mut start = remaster_start.saturating_sub(1);
        let prev = query.get(..remaster_start.saturating_sub(2)).unwrap_or("");
        let end_of_prev_word = prev.rfind(' ').unwrap_or(0);

        if let Some(year) = query.get(end_of_prev_word + 1..remaster_start.saturating_sub(1)) {
            if year.chars().all(|c| c.is_whitespace() || c.is_numeric()) {
                start = end_of_prev_word;
            }
        }
        start = rfind_non_filler(&query, start);
        query.drain(start..end);
    }
    // remove "- xxxx yyy remix" from the query
    // For example, `{song} - xxxx yyy remix {artists}` becomes `{song} {artists}`.
    if let Some(remix_start) = query.find("remix") {
        let end = remix_start + "remix".len();
        let end = end_of_word(&query, end);

        if let Some(metadata_start) = query.rfind(is_dash) {
            if metadata_start >= SONG_MIN_LENGTH_WO_REMIX_METADATA {
                let start = rfind_non_filler(&query, metadata_start);
                query.drain(start..end);
            }
        }
    }
    query
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
