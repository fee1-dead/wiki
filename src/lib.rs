use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;

use api::{CsrfToken, QueryAllGenerator, RequestBuilderExt, Token};
use futures_util::Future;
use generators::GeneratorStream;
use req::{Main, SerializeAdaptor};
use reqwest::header::{HeaderMap, HeaderValue, InvalidHeaderValue};
use reqwest::{Client, RequestBuilder, Url};
use serde_json::Value;

use crate::generators::WikiGenerator;

extern crate self as wiki;

pub mod api;
mod boring_impls;
pub mod builder;
pub mod events;
pub mod generators;
pub mod macro_support;
pub mod req;
pub mod res;
pub mod types;
pub mod url;
pub mod util;

#[derive(Clone)]
pub struct AnonymousAccess;

#[derive(Clone)]
pub struct AuthorizedAccess(());

pub(crate) mod sealed {
    pub trait Access {}
    impl Access for super::AnonymousAccess {}
    impl Access for super::AuthorizedAccess {}
}

pub struct Site<T: sealed::Access = AnonymousAccess> {
    pub client: reqwest::Client,
    url: Url,
    acc: PhantomData<T>,
}

impl<T: sealed::Access> Clone for Site<T> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            url: self.url.clone(),
            acc: PhantomData,
        }
    }
}

impl<T: sealed::Access> fmt::Debug for Site<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Site")
            .field("client", &self.client)
            .field("url", &self.url)
            .finish()
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
    #[error("{0}")]
    HttpTypes(http_types::Error),
    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("MediaWiki API returned error: {0}")]
    MediaWiki(serde_json::Value),
    #[error("failed to log in")]
    Unauthorized,
}

impl From<http_types::Error> for Error {
    fn from(e: http_types::Error) -> Self {
        Self::HttpTypes(e)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub type Bot = Site<AuthorizedAccess>;

impl<A: sealed::Access> Site<A> {
    pub fn mkurl(&self, m: Main) -> Url {
        crate::api::mkurl(self.url.clone(), m)
    }

    pub fn mkurl_with_ext(&self, m: Main, ext: Value) -> Result<Url, serde_urlencoded::ser::Error> {
        crate::api::mkurl_with_ext(self.url.clone(), m, ext)
    }

    pub fn get(&self, action: req::Action) -> RequestBuilder {
        let url = self.mkurl(Main {
            action,
            format: req::Format::Json { formatversion: 2 },
        });
        self.client.get(url)
    }

    pub fn post(&self, action: req::Action) -> RequestBuilder {
        let main = Main {
            action,
            format: req::Format::Json { formatversion: 2 },
        };
        self.client
            .post(self.url.clone())
            .form(&SerializeAdaptor(main))
    }

    pub fn get_csrf_token(&self) -> Pin<Box<dyn Future<Output = Result<CsrfToken>> + Send + Sync>> {
        self.get_token()
    }

    pub fn get_token<T: Token>(&self) -> Pin<Box<dyn Future<Output = Result<T>> + Send + Sync>> {
        let url = self.mkurl(Main {
            action: req::Action::Query(req::Query {
                meta: Some(req::QueryMeta::Tokens { type_: T::types() }.into()),
                ..Default::default()
            }),
            format: req::Format::Json { formatversion: 2 },
        });

        self.client.get(url).send_parse()
    }

    pub fn query_all(&self, query: req::Query) -> GeneratorStream<QueryAllGenerator<A>> {
        let m = Main::query(query);

        fn clone(_: &Url, _: &Client, v: &Main) -> Main {
            v.clone()
        }

        fn response(_: &Url, _: &Client, _: &Main, v: Value) -> Result<Vec<Value>> {
            Ok(vec![v])
        }

        QueryAllGenerator::new(self.clone(), m, clone, response).into_stream()
    }
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
    pub fn new(api_url: &str) -> Result<Self> {
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

        Ok(Site {
            client,
            url,
            acc: PhantomData,
        })
    }

    pub fn enwiki() -> Self {
        Site::new("https://en.wikipedia.org/w/api.php").unwrap()
    }

    pub fn test_wikipedia() -> Self {
        Site::new("https://test.wikipedia.org/w/api.php").unwrap()
    }

    pub fn test_miraheze() -> Self {
        Site::new("https://publictestwiki.com/w/api.php").unwrap()
    }
}

#[cfg(test)]
mod tests;
