use std::sync::Arc;

use api::BotOptions;
use jobs::JobQueue;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Url};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::time::Interval;

extern crate self as wiki;

pub mod api;
mod boring_impls;
pub mod generators;
pub mod jobs;
pub mod macro_support;
pub mod req;
pub mod types;
pub mod url;

#[derive(Debug)]
pub struct Site {
    client: reqwest::Client,
    url: Url,
}

#[derive(Deserialize, Debug)]
pub struct QueryResponse<Q> {
    pub query: Q,
}

pub struct Page {
    content: String,
    id: u32,
    latest_revision: u32,
    changed: bool,
    bot: Option<Bot>,
}

impl Page {
    pub fn content_mut(&mut self) -> &mut String {
        self.changed = true;
        &mut self.content
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidUrl(#[from] ::url::ParseError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    SerdeUrlEncoded(#[from] serde_urlencoded::ser::Error),
    #[error("MediaWiki API returned error: {0}")]
    MediaWiki(serde_json::Value),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// BotInn TM
pub struct BotInn {
    url: Url,
    #[allow(unused)]
    pass: BotPassword,
    control: Mutex<Interval>,
    options: BotOptions,
}

/// A bot that is logged in.
#[derive(Clone)]
pub struct Bot {
    inn: Arc<BotInn>,
    queue: JobQueue,
    client: Client,
}

#[derive(Clone)]
pub struct BotPassword {
    username: String,
    password: String,
}

impl BotPassword {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

const UA: &str = concat!(
    "wiki.rs",
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/fee1-dead/wiki.rs ent3rm4n@gmail.com)"
);

impl Site {
    pub fn new(api_url: &'static str) -> Result<Self> {
        let url: Url = api_url.parse()?;
        assert!(url.query().is_none());
        let mut client = Client::builder();
        #[cfg(feature = "default")]
        {
            client = client.cookie_store(true).user_agent(UA);
        }
        let client = client.build()?;
        let mut headers = HeaderMap::new();
        headers.insert("Api-User-Agent", HeaderValue::from_static(UA));

        Ok(Site { client, url })
    }

    pub fn enwiki() -> Self {
        Site::new("https://en.wikipedia.org/w/api.php").unwrap()
    }

    pub fn testwiki() -> Self {
        Site::new("https://test.wikipedia.org/w/api.php").unwrap()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
