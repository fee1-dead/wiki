use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::BoxFuture;
use futures_util::{Stream, stream};
use serde::Deserialize;
use serde::de::DeserializeOwned;
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
pub enum State<I> {
    Init,
    Values(Vec<I>, Option<Value>),
    Cont(Value),
    Done,
}

impl<I> State<I> {
    pub fn values(v: Vec<I>, cont: Option<Value>) -> Self {
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

pub trait WikiGenerator {
    type Item: 'static;
    type Response: DeserializeOwned;
    fn bot(&self) -> &Bot;
    fn create_request(&self) -> Main;
    fn untangle_response(&self, res: Self::Response) -> crate::Result<(Option<Value>, Vec<Self::Item>)>;
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = crate::Result<Self::Item>>>> where Self: Sized + 'static {
        struct Storage<T, I> {
            inner: T,
            state: State<I>,
        }
        Box::pin(stream::unfold(Storage { inner: self, state: State::Init }, |mut st| async move {
            macro_rules! tryit {
                ($e:expr) => {
                    match $e {
                        Ok(very_well) => very_well,
                        Err(e) => {
                            st.state = State::Done;
                            return Some((Err(e.into()), st));
                        }
                    }
                };
            }

            let url = match st.state {
                State::Init => {
                    let main = st.inner.create_request();
                    st.inner.bot().mkurl(main)
                }
                State::Cont(v) => {
                    let main = st.inner.create_request();
                    tryit!(st.inner.bot().mkurl_with_ext(main, v))
                }
                State::Values(mut v, cont) => {
                    let value = v.pop().expect("must always have value");
                    st.state = State::values(v, cont);
                    return Some((Ok(value), st))
                }
                State::Done => return None,
            };
            
            let res = tryit!(st.inner.bot().client.get(url).send_parse().await);
            let (cont, mut items) = tryit!(st.inner.untangle_response(res));
            if let Some(item) = items.pop() {
                st.state = State::values(items, cont);
                Some((Ok(item), st))
            } else {
                assert!(cont.is_none(), "Cannot continue without return value");
                None
            }
        }))
    }
}

pub struct SearchGenerator {
    bot: Bot,
    search: String,
}

impl WikiGenerator for SearchGenerator {
    type Item = BasicSearchResult;
    type Response = MaybeContinue<api::Query<api::Search<BasicSearchResult>>>;

    fn bot(&self) -> &Bot {
        &self.bot
    }

    fn create_request(&self) -> Main {
        Main::query(Query {
            list: Some(
                QueryList::Search(ListSearch {
                    search: self.search.clone(),
                    limit: req::Limit::Max,
                    prop: None,
                })
                .into(),
            ),
            ..Default::default()
        })
    }

    fn untangle_response(&self, res: Self::Response) -> crate::Result<(Option<Value>, Vec<Self::Item>)> {
        Ok((res.cont, res.inner.query.search))
    }
}


impl SearchGenerator {
    pub fn new(bot: Bot, search: String) -> Self {
        Self {
            search,
            bot,
        }
    }
}
