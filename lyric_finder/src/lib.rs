//! Find lyric for songs.
//!
//! This crate provides Rust APIs to retrieve a song lyric.

const SEARCH_BASE_URL: &str = "https://genius.com/api/search";

pub struct Client {
    http: reqwest::Client,
}

#[derive(Debug)]
pub struct LyricResult {
    pub title: String,
    pub artist_names: String,
    pub lyric: String,
}

impl Client {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn search_lyric(&self, query: &str) -> anyhow::Result<Vec<search::Result>> {
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
                .map(|hit| hit.result)
                .collect::<Vec<_>>()
        });

        let not_found_err = anyhow::anyhow!("lyric not found for query {}", query);

        match urls {
            Some(v) => {
                if v.is_empty() {
                    Err(not_found_err)
                } else {
                    Ok(v)
                }
            }
            None => Err(not_found_err),
        }
    }

    pub async fn retrieve_lyric(&self, url: &str) -> anyhow::Result<String> {
        let html = self.http.get(url).send().await?.text().await?;
        log::debug!("html: {html}");
        let lyric = parse::parse(html)?;
        Ok(lyric.trim().to_string())
    }

    pub async fn get_lyric(&self, query: &str) -> anyhow::Result<LyricResult> {
        let result = {
            let mut results = self.search_lyric(query).await?;
            log::debug!("hits: {results:?}");
            results.remove(0)
        };
        let lyric = self.retrieve_lyric(&result.url).await?;

        Ok(LyricResult {
            title: result.title,
            artist_names: result.artist_names,
            lyric,
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

    pub fn parse(html: String) -> anyhow::Result<String> {
        // parse HTML content into DOM node(s)
        let dom = parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut (html.as_bytes()))?;

        let filter = |data: &NodeData| match data {
            NodeData::Element { ref attrs, .. } => attrs
                .borrow()
                .iter()
                .any(|attr| attr.name.local.to_string() == "data-lyrics-container"),
            _ => false,
        };

        Ok(parse_dom_node(dom.document, &Some(filter), false))
    }

    fn parse_dom_node<F>(node: Handle, filter: &Option<F>, mut should_parse: bool) -> String
    where
        F: Fn(&NodeData) -> bool,
    {
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
