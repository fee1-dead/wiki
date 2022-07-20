//! EventStreams for WikiMedia projects.

use std::collections::HashMap;
use std::io;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::pin::Pin;
use std::task::Poll;

use async_sse::{Decoder, Event};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::stream::{IntoAsyncRead, MapErr, MapOk};
use futures_util::{Stream, TryStreamExt};
use serde::Deserialize;
use serde_json::Value;

type Tr = fn(reqwest::Error) -> io::Error;
type TrOk = fn(Event) -> crate::Result<serde_json::Value>;
type ReqStream = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>>>>;
type ReqwestSseDecoder = MapOk<Decoder<IntoAsyncRead<MapErr<ReqStream, Tr>>>, TrOk>;

pub struct ReqwestSseStream<C> {
    pub decoder: ReqwestSseDecoder,
    pub _content: PhantomData<fn() -> C>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EventMeta {
    #[serde(with = "crate::util::dt")]
    pub dt: DateTime<Utc>,
    pub stream: String,
    pub domain: Option<String>,
    pub request_id: Option<String>,
    pub uri: Option<String>,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OldNew {
    pub old: Option<u64>,
    pub new: Option<u64>,
}

/// https://schema.wikimedia.org/repositories/primary/jsonschema/mediawiki/recentchange/latest.json
#[derive(Deserialize, Debug, Clone)]
pub struct RecentChangeEvent {
    pub meta: EventMeta,
    pub id: Option<usize>,
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub title: Option<String>,
    pub namespace: Option<i64>,
    pub comment: Option<String>,
    pub parsedcomment: Option<String>,
    pub timestamp: Option<i64>,
    pub user: Option<String>,
    pub bot: bool,
    pub server_url: Option<String>,
    pub server_script_path: Option<String>,
    pub wiki: Option<String>,
    pub minor: bool,
    pub patrolled: Option<bool>,
    pub length: Option<OldNew>,
    pub revision: Option<OldNew>,
    pub log_id: Option<u64>,
    pub log_type: Option<String>,
    pub log_action: Option<String>,
    pub log_params: Option<Value>,
    pub log_action_comment: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RevisionScoreEvent {
    pub database: String,
    pub meta: EventMeta,
    pub page_id: NonZeroU64,
    pub page_title: String,
    pub page_namespace: i64,
    pub page_is_redirect: bool,
    pub rev_id: u64,
    pub rev_parent_id: Option<u64>,
    #[serde(with = "crate::util::dt")]
    pub rev_timestamp: DateTime<Utc>,
    #[serde(default)]
    pub scores: HashMap<String, OresScores>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OresScores {
    pub model_name: String,
    pub model_version: String,
    pub prediction: Vec<String>,
    pub probability: HashMap<String, f64>,
}

impl ReqwestSseStream<RecentChangeEvent> {
    pub async fn recent_changes() -> crate::Result<Self> {
        Self::new("https://stream.wikimedia.org/v2/stream/recentchange").await
    }
}

impl ReqwestSseStream<RevisionScoreEvent> {
    pub async fn revision_scores() -> crate::Result<Self> {
        Self::new("https://stream.wikimedia.org/v2/stream/revision-score").await
    }
}

impl<C> ReqwestSseStream<C> {
    pub async fn new(url: &str) -> crate::Result<Self> {
        let res = reqwest::get(url).await?;
        let f: Tr = |e| io::Error::new(io::ErrorKind::Other, e);
        let o: TrOk = |e| match e {
            Event::Message(m) => Ok(serde_json::from_slice(m.data())?),
            _ => panic!("what?"),
        };
        let s: ReqStream = Box::pin(res.bytes_stream());
        let decoder = async_sse::decode(s.map_err(f).into_async_read()).map_ok(o);

        Ok(Self {
            decoder,
            _content: PhantomData,
        })
    }
}

impl<C: serde::de::DeserializeOwned> Stream for ReqwestSseStream<C> {
    type Item = crate::Result<C>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        match Pin::new(&mut this.decoder).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(res) => {
                Poll::Ready(res.map(|res| (|| Ok(serde_json::from_value(res??)?))()))
            }
        }
    }
}
