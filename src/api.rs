use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use futures_util::TryStreamExt;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use tracing::debug;

use crate::generators::GenGen;
use crate::req::{self, Main, PageSpec, TokenType};
use crate::res::PageResponse;
use crate::sealed::Access;
use crate::url::WriteUrlParams;
#[cfg(target_arch = "wasm32")]
use crate::url::{TriStr, UrlParamWriter};
use crate::Result;

#[macro_export]
macro_rules! build_response_type {
    (@handle( $(#[$meta:meta])*  $i:ident { $name:ident$([$lit:literal])?: $ty:ty } )) => {
        #[derive(Deserialize, Debug)]
        $(#[$meta])*
        pub struct $i {
            $(#[serde(rename = $lit)])?
            pub $name: $ty,
        }
    };
    (@handle( $i:ident { $name:ident$([$lit:literal])? } )) => {
        #[derive(Deserialize, Debug)]
        pub struct $i<__Inner> {
            $(#[serde(rename = $lit)])?
            pub $name: __Inner,
        }
    };
    (@handle( $i:ident { $T:ident => $name:ident $([$lit:literal])? : $($rest:tt)* } )) => {
        #[derive(Deserialize, Debug)]
        pub struct $i<$T> {
            $(#[serde(rename = $lit)])?
            pub $name: $($rest)*,
        }
    };
    ($( $(#[$meta:meta])* $ty:ident { $($tt:tt)* })*) => {
        $(build_response_type!(@handle( $(#[$meta])* $ty { $($tt)* }));)*
    };
}

build_response_type! {
    RevisionSlots { slots: SlotsMain }
    SlotsMain { main: Slot }
    QueryResponse { query }
    Pages { T => pages: Vec<T> }
    AbuseLog { T => abuse_log["abuselog"]: Vec<T> }
    AbuseFilters { T => abuse_filters["abusefilters"]: Vec<T> }
    Pattern { pattern: String }
    Search { T => search: Vec<T> }
    RecentChanges { T => recent_changes["recentchanges"]: Vec<T> }
    RevisionsList { T => revisions: Vec<T> }
    AbuseFilterCheckMatchResult { result: bool }
    AbuseFilterCheckMatchResponse { inner["abusefiltercheckmatch"]: AbuseFilterCheckMatchResult }
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
pub struct BasicSearchResult {
    pub ns: u16,
    pub title: String,
    #[serde(rename = "pageid")]
    pub page_id: usize,
}

#[derive(Deserialize, Debug)]
pub struct RecentChangesResult {
    #[serde(rename = "type")]
    pub type_: String,
    pub ns: Option<u16>,
    pub title: Option<String>,
    pub pageid: Option<usize>,
    pub revid: Option<usize>,
    pub old_revid: Option<usize>,
    pub rcid: Option<usize>,
    pub user: Option<String>,
    pub userid: Option<usize>,
    pub oldlen: Option<usize>,
    pub newlen: Option<usize>,
    pub timestamp: Option<String>,
    pub comment: Option<String>,
    pub parsedcomment: Option<String>,
    pub redirect: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub sha1: Option<String>,
    pub oresscores: Option<Value>, // TODO more precise
}

#[derive(Deserialize, Debug)]
pub struct Slot {
    #[serde(rename = "contentmodel")]
    pub content_model: String,
    #[serde(rename = "contentformat")]
    pub content_format: String,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub struct Revision<S> {
    #[serde(rename = "revid")]
    pub rev_id: u32,
    #[serde(rename = "parentid")]
    pub parent_id: u32,
    pub slots: S,
}

#[derive(Deserialize, Debug)]
pub struct MaybeContinue<T> {
    #[serde(rename = "continue", default)]
    pub cont: Option<Value>,
    #[serde(flatten)]
    pub inner: T,
}

#[derive(Deserialize, Debug)]
pub struct Q2<A, B> {
    #[serde(flatten)]
    pub a: A,
    #[serde(flatten)]
    pub b: B,
}

macro_rules! token {
    ($Name:ident = $field:literal = [$($t:expr),+$(,)?] + $token:ident) => {
        #[derive(Deserialize, Debug)]
        pub struct $Name {
            #[serde(rename = $field)]
            pub $token: String,
        }
        impl Token for $Name {
            fn types() -> TokenType { $($t)|* }
        }
    };
}

token!(LoginToken = "logintoken" = [TokenType::LOGIN] + token);
token!(CsrfToken = "csrftoken" = [TokenType::CSRF] + token);

pub trait Token: DeserializeOwned {
    fn types() -> TokenType;
}

#[derive(Deserialize, Debug)]
pub struct Page<S> {
    #[serde(rename = "pageid")]
    pub page_id: u32,
    pub ns: u8,
    pub title: String,
    pub revisions: Vec<Revision<S>>,
}

#[derive(Deserialize, Debug)]
pub struct Tokens<T> {
    pub tokens: T,
}

#[derive(Deserialize, Debug)]
pub struct Revisions<S> {
    pub pages: HashMap<usize, Page<S>>,
}

pub enum PageRef<'a> {
    Title(&'a str),
    Id(u32),
}

#[derive(Deserialize, Debug)]
pub struct UserInfo<E> {
    pub userinfo: UserInfoInner<E>,
}

#[derive(Deserialize, Debug)]
pub struct UserInfoInner<E> {
    pub id: usize,
    pub name: String,
    #[serde(flatten)]
    pub extra: E,
}

#[derive(Deserialize, Debug)]
pub struct UserInfoRights {
    pub rights: Vec<String>,
}

pub fn mkurl(mut url: Url, m: Main) -> Url {
    let mut q = crate::url::Simple::default();
    if let Err(e) = m.ser(&mut q) {
        match e {}
    }
    // todo wasi does not need this
    #[cfg(target_arch = "wasm32")]
    {
        q.add(TriStr::Static("origin"), TriStr::Static("*"));
    }
    url.set_query(Some(&q.0));
    debug!(%url, "GET");
    url
}

pub fn mkurl_with_ext(
    mut url: Url,
    m: Main,
    ext: Value,
) -> Result<Url, serde_urlencoded::ser::Error> {
    let mut q = crate::url::Simple::default();
    if let Err(e) = m.ser(&mut q) {
        match e {}
    }
    q.add_serde(ext)?;
    // todo wasi does not need this
    #[cfg(target_arch = "wasm32")]
    {
        q.add(TriStr::Static("origin"), TriStr::Static("*"));
    }
    url.set_query(Some(&q.0));
    debug!(%url, "GET");
    Ok(url)
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for reqwest::RequestBuilder {}
}

pub trait RequestBuilderExt: Sized + sealed::Sealed {
    fn send_and_report_err(self) -> BoxFuture<crate::Result<Value>>;
    fn send_parse<D: DeserializeOwned>(self) -> BoxFuture<crate::Result<D>>
    where
        Self: Send + Sync + 'static,
    {
        Box::pin(async move {
            let v = self.send_and_report_err().await?;
            Ok(serde_json::from_value(v)?)
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync>>;

#[cfg(target_arch = "wasm32")]
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T>>>;

impl RequestBuilderExt for reqwest::RequestBuilder {
    fn send_and_report_err(self) -> BoxFuture<crate::Result<Value>> {
        Box::pin(async {
            let r = self.send().await?;
            let mut v = r.json::<Value>().await?;
            if let Some(v) = v.get_mut("error") {
                Err(crate::Error::MediaWiki(v.take()))
            } else {
                Ok(v)
            }
        })
    }
}

impl<A: Access> crate::Client<A> {
    pub async fn get_tokens<T: Token>(&self) -> Result<T> {
        let res = self
            .client
            .get(mkurl(self.url.clone(), Main::tokens(T::types())))
            .send()
            .await?;
        let tokens: QueryResponse<Tokens<T>> = res.json().await?;
        Ok(tokens.query.tokens)
    }
}

pub type QueryAllGenerator<A> = GenGen<
    A,
    Main,
    fn(&Url, &reqwest::Client, &Main) -> Main,
    fn(&Url, &reqwest::Client, &Main, Value) -> Result<Vec<Value>>,
    Value,
    Value,
>;

impl crate::Bot {
    pub async fn page_info_all<E: DeserializeOwned>(
        &self,
        mut query: req::Query,
        spec: PageSpec,
    ) -> Result<PageResponse<E>> {
        query.pageids = None;
        query.titles = None;
        match spec {
            PageSpec::PageId(id) => query.pageids = Some(vec![id]),
            PageSpec::Title(title) => query.titles = Some(vec![title]),
        }

        self.query_all(query)
            .try_fold(Value::Null, |mut acc, new| async {
                crate::util::merge_values(&mut acc, new);
                Ok(acc)
            })
            .await
            .and_then(|v| Ok(serde_json::from_value(v)?))
    }
}
