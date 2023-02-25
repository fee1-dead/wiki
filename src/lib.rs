//! A crate for working with MediaWiki, mostly with the Action API.
//!
//! ## Examples
//! 
//! To create a client, use either [`Client::new`] or [`ClientBuilder`]:
//! 
//! ```
//! use wiki::Client;
//! let client = Client::new("https://en.wikipedia.org/w/api.php").unwrap();
//! # let _ = client;
//! ```
//! 
//! For common sites such as English Wikipedia, there is a short cut:
//! 
//! ```
//! let client = wiki::Client::enwiki();
//! # let _ = client;
//! ```
//! 
//! To log in using bot passwords or OAuth, use [`ClientBuilder::password`] or [`ClientBuilder::oauth`]:
//! 
//! ```no_run
//! use wiki::{BotPassword, ClientBuilder};
//! # tokio_test::block_on(async {
//! let client = ClientBuilder::enwiki()
//!    .password(BotPassword::new("username", "password"))
//!    .build()
//!    .await.unwrap();
//! # let _ = client;
//! # });
//! ```
//! 
//! ```no_run
//! use wiki::{ClientBuilder};
//! # tokio_test::block_on(async {
//! let client = ClientBuilder::enwiki()
//!   .oauth("oauth_token")
//!   .build()
//!   .await.unwrap();
//! # let _ = client;
//! # });
//! ```
//! 

use std::fmt;
use std::marker::PhantomData;

use api::{BoxFuture, CsrfToken, QueryAllGenerator, RequestBuilderExt, Token};
use deterministic::IsMain;
use futures_util::future::MapOk;
use futures_util::TryFutureExt;
use generators::GeneratorStream;
use req::{Main, PageSpec, SerializeAdaptor};
use reqwest::header::InvalidHeaderValue;
use reqwest::{RequestBuilder, Url};
use serde_json::Value;
use tracing::debug;

#[cfg(target_arch = "wasm32")]
use reqwest::header::{HeaderMap, HeaderValue};

use crate::generators::WikiGenerator;

extern crate self as wiki;

pub mod api;
mod boring_impls;
mod builder;
pub mod deterministic;
pub mod events;
pub mod generators;
pub mod macro_support;
pub mod req;
pub mod res;
pub mod types;
pub mod url;
pub mod util;

pub use builder::ClientBuilder;

/// A marker representing anonymous access to a MediaWiki API endpoint.
#[derive(Clone)]
pub struct AnonymousAccess;

/// Marker for authorized access, meaning that we are logged in, either via BotPassword or OAuth.
#[derive(Clone)]
pub struct AuthorizedAccess(());

pub(crate) mod sealed {
    pub trait Access {}
    impl Access for super::AnonymousAccess {}
    impl Access for super::AuthorizedAccess {}
}

/// A generic client for a MediaWiki API endpoint. Could be logged in depending on the type parameter
pub struct Client<T: sealed::Access = AnonymousAccess> {
    pub client: reqwest::Client,
    url: Url,
    acc: PhantomData<T>,
}

impl<T: sealed::Access> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            url: self.url.clone(),
            acc: PhantomData,
        }
    }
}

impl<T: sealed::Access> fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Site")
            .field("client", &self.client)
            .field("url", &self.url)
            .finish()
    }
}

/// The error type for this crate
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
    #[error("{0}")]
    CustomStatic(&'static str),
}

/// The result type for this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A bot is a client that has authorized access to a wiki.
pub type Bot = Client<AuthorizedAccess>;

type TokensFullResponse<T> = api::QueryResponse<api::Tokens<T>>;
type TokensFuture<T> =
    MapOk<BoxFuture<Result<TokensFullResponse<T>>>, fn(TokensFullResponse<T>) -> T>;

impl<A: sealed::Access> Client<A> {
    /// Create a URL for GET requests.
    pub fn mkurl(&self, m: Main) -> Url {
        crate::api::mkurl(self.url.clone(), m)
    }

    /// Create a URL for GET request, with extra values in the query string. (could be from `continue`)
    pub fn mkurl_with_ext(&self, m: Main, ext: Value) -> Result<Url, serde_urlencoded::ser::Error> {
        crate::api::mkurl_with_ext(self.url.clone(), m, ext)
    }

    /// Fetches the latest wikitext from a page based on page id or page title.
    pub async fn fetch_content(&self, page: impl Into<PageSpec>) -> Result<String> {
        let mut q = req::Query {
            prop: Some(
                req::QueryProp::Revisions(req::QueryPropRevisions {
                    prop: req::RvProp::CONTENT,
                    slots: req::RvSlot::Main.into(),
                    limit: req::Limit::Value(1),
                })
                .into(),
            ),
            ..Default::default()
        };
        // TODO maybe use page spec on query??
        match page.into() {
            PageSpec::PageId(id) => q.pageids = Some(vec![id]),
            PageSpec::Title(title) => q.titles = Some(vec![title]),
        }
        let x: api::QueryResponse<api::Pages<api::RevisionsList<api::RevisionSlots>>> =
            self.get(req::Action::Query(q)).send_parse().await?;
        let page = x
            .query
            .pages
            .into_iter()
            .next()
            .ok_or(Error::CustomStatic("not enough pages"))?;
        let rev = page
            .revisions
            .into_iter()
            .next()
            .ok_or(Error::CustomStatic("not enough revisions"))?;
        Ok(rev.slots.main.content)
    }

    /// Start building an edit.
    pub fn build_edit(&self, page: impl Into<PageSpec>) -> req::EditBuilder<Self> {
        let q = req::EditBuilder::with_access(self.clone());
        match page.into() {
            PageSpec::PageId(id) => q.page_id(id),
            PageSpec::Title(title) => q.title(title),
        }
    }

    /// Build a GET request based on the specific action. This will always use JSON format version 2.
    pub fn get(&self, action: req::Action) -> RequestBuilder {
        let url = self.mkurl(Main {
            action,
            format: req::Format::Json { formatversion: 2 },
        });
        self.client.get(url)
    }

    /// An experimental way for GET requests. Uses const generics to specify the actual request at
    /// compile time and the output type is inferred from the request.
    pub async fn get_d<T: IsMain>(&self, m: T) -> Result<T::Output> {
        let mut q = crate::url::Simple::default();
        if let Err(e) = m.ser(&mut q) {
            match e {}
        }
        let mut url = self.url.clone();
        url.set_query(Some(&q.0));
        debug!(%url, "GET");
        Ok(self.client.get(url).send_parse().await?)
    }

    /// Build a POST request based on the specific action. This will always use JSON format version 2.
    pub fn post(&self, action: req::Action) -> RequestBuilder {
        let main = Main {
            action,
            format: req::Format::Json { formatversion: 2 },
        };
        self.client
            .post(self.url.clone())
            .form(&SerializeAdaptor(main))
    }

    /// Retrieve a CSRF token for editing.
    pub fn get_csrf_token(&self) -> TokensFuture<CsrfToken> {
        self.get_token()
    }

    /// Get a token.
    pub fn get_token<T: Token>(&self) -> TokensFuture<T> {
        let url = self.mkurl(Main {
            action: req::Action::Query(req::Query {
                meta: Some(req::QueryMeta::Tokens { type_: T::types() }.into()),
                ..Default::default()
            }),
            format: req::Format::Json { formatversion: 2 },
        });

        self.client
            .get(url)
            .send_parse()
            .map_ok(|x: api::QueryResponse<api::Tokens<T>>| x.query.tokens)
    }

    /// Perform a query, except returns a `Stream` of results that continues from `continue` parameters
    /// in the responses.
    pub fn query_all(&self, query: req::Query) -> GeneratorStream<QueryAllGenerator<A>> {
        let m = Main::query(query);

        fn clone(_: &Url, _: &reqwest::Client, v: &Main) -> Main {
            v.clone()
        }

        fn response(_: &Url, _: &reqwest::Client, _: &Main, v: Value) -> Result<Vec<Value>> {
            Ok(vec![v])
        }

        QueryAllGenerator::new(self.clone(), m, clone, response).into_stream()
    }
}

/// A structure for bot passwords.
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

impl Client {
    /// Create a new client with anonymous access.
    pub fn new(api_url: &str) -> Result<Self> {
        let url: Url = api_url.parse()?;
        assert!(url.query().is_none());
        let mut client = reqwest::Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        {
            client = client.cookie_store(true).user_agent(UA);
        }
        

        #[cfg(target_arch = "wasm32")] {
            let mut headers = HeaderMap::new();
            headers.insert("Api-User-Agent", HeaderValue::from_static(UA));
            client = client.default_headers(headers);
        }
        
        
        let client = client.build()?;

        Ok(Client {
            client,
            url,
            acc: PhantomData,
        })
    }

    /// Create a client for English Wikipedia (en.wikipedia.org).
    pub fn enwiki() -> Self {
        Client::new("https://en.wikipedia.org/w/api.php").unwrap()
    }

    /// Create a client for Test Wikipedia (test.wikipedia.org).
    pub fn test_wikipedia() -> Self {
        Client::new("https://test.wikipedia.org/w/api.php").unwrap()
    }

    /// Create a client for the public test wiki (publictestwiki.com).
    pub fn test_miraheze() -> Self {
        Client::new("https://publictestwiki.com/w/api.php").unwrap()
    }
}

// doesn't work with serde's `#[from]`
impl From<http_types::Error> for Error {
    fn from(e: http_types::Error) -> Self {
        Self::HttpTypes(e)
    }
}

#[cfg(test)]
mod tests;
