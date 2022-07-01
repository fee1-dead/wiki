use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::BoxFuture;
use futures_util::Stream;
use serde_json::Value;

use crate::api::{MaybeContinue, RequestBuilderExt, Revisions, SlotsMain, BasicSearchResult};
use crate::req::{
    self, EnumSet, ListSearch, Main, Query, QueryGenerator, QueryList, QueryProp,
    QueryPropRevisions, RvProp, RvSlot,
};
use crate::{api, Bot, Page};

pub type BoxReqFuture = BoxFuture<'static, reqwest::Result<reqwest::Response>>;
pub type BoxRecvFuture = BoxFuture<'static, reqwest::Result<api::Query<Revisions<SlotsMain>>>>;

#[derive(PartialEq, Eq, Debug)]
pub enum State {
    Init,
    Values(Vec<BasicSearchResult>, Option<Value>),
    Cont(Value),
    Done,
}

impl State {
    pub fn values(v: Vec<BasicSearchResult>, cont: Option<Value>) -> Self {
        if v.is_empty() {
            if let Some(c) = cont {
                Self::Cont(c)
            } else {
                Self::Done
            }
        } else {
            Self::Values(v, cont)
        }
    }
}

pub struct SearchGenerator {
    search: String,
    bot: Bot,
    state: State,
}

impl SearchGenerator {
    pub fn new(bot: Bot, search: String) -> Self {
        Self {
            search,
            bot,
            state: State::Init,
        }
    }

    pub fn into_stream(self) -> impl Stream<Item = crate::Result<BasicSearchResult>> {
        futures_util::stream::unfold(self, |mut this| async move {
            macro_rules! tryit {
                ($e:expr) => {
                    match $e {
                        Ok(very_well) => very_well,
                        Err(e) => {
                            this.state = State::Done;
                            return Some((Err(e.into()), this));
                        }
                    }
                };
            }
            let make = || Main::query(Query {
                list: Some(
                    QueryList::Search(ListSearch {
                        search: this.search.clone(),
                        limit: req::Limit::Max,
                        prop: None,
                    })
                    .into(),
                ),
                ..Default::default()
            });
            macro_rules! handle {
                ($u:ident) => {{
                    let res: MaybeContinue<api::Query<api::Search<BasicSearchResult>>> = tryit!(this.bot.client.get($u).send_parse().await);

                    let mut items = res.inner.query.search;

                    if let Some(item) = items.pop() {
                        this.state = State::values(items, res.cont);
                        Some((Ok(item), this))
                    } else {
                        assert!(res.cont.is_none(), "Cannot continue without return value");
                        None
                    }
                }};
            }
            match this.state {
                State::Init => {
                    let m = make();
                    let u = this.bot.mkurl(m);
                    handle!(u)
                }
                State::Cont(v) => {
                    let m = make();
                    let u = tryit!(this.bot.mkurl_with_ext(m, v));
                    handle!(u)
                }
                State::Values(mut v, cont) => {
                    let value = v.pop().expect("must always have value");
                    this.state = State::values(v, cont);
                    Some((Ok(value), this))
                }
                State::Done => None,
            }
        })
    }
}
