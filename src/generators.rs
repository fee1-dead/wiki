use std::future::Future;
use std::marker::PhantomData;
use std::mem::{replace, take};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::BoxFuture;
use futures_util::{stream, Stream};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::api::{
    BasicSearchResult, MaybeContinue, RecentChangesResult, RequestBuilderExt, Revisions, SlotsMain,
};
use crate::req::rc::ListRc;
use crate::req::{
    self, EnumSet, ListSearch, Main, Query, QueryGenerator, QueryList, QueryProp,
    QueryPropRevisions, RvProp, RvSlot,
};
use crate::{api, Bot, Page};

pub mod rcpatrol;

pub type BoxReqFuture = BoxFuture<'static, reqwest::Result<reqwest::Response>>;
pub type BoxRecvFuture = BoxFuture<'static, reqwest::Result<api::QueryResponse<Revisions<SlotsMain>>>>;

pub type ResponseFuture<G> = Pin<
    Box<
        dyn Future<Output = crate::Result<MaybeContinue<<G as WikiGenerator>::Response>>>
            + Send
            + Sync,
    >,
>;

#[derive(Default)]
#[pin_project::pin_project(project = StateProj)]
pub enum State<G: WikiGenerator> {
    #[default]
    Init,
    Fut(#[pin] ResponseFuture<G>),
    Values(Vec<G::Item>, Option<Value>),
    Cont(Value),
    Done,
}

impl<G: WikiGenerator> State<G> {
    pub fn values(v: Vec<G::Item>, cont: Option<Value>) -> Self {
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

#[pin_project::pin_project]
pub struct GeneratorStream<G: WikiGenerator> {
    pub generator: G,
    #[pin]
    state: State<G>,
}

impl<G: WikiGenerator> Stream for GeneratorStream<G> {
    type Item = crate::Result<G::Item>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        macro_rules! tryit {
            ($e:expr) => {
                match $e {
                    Ok(very_well) => very_well,
                    Err(e) => {
                        this.state.set(State::Done);
                        return Poll::Ready(Some(Err(e.into())));
                    }
                }
            };
        }

        let url = match this.state.as_mut().project() {
            StateProj::Init => {
                let main = this.generator.create_request();
                this.generator.bot().mkurl(main)
            }
            StateProj::Cont(v) => {
                let main = this.generator.create_request();
                tryit!(this.generator.bot().mkurl_with_ext(main, v.take()))
            }
            StateProj::Values(v, cont) => {
                let value = v.pop().expect("must always have value");
                let state = State::values(take(v), take(cont));
                this.state.set(state);
                return Poll::Ready(Some(Ok(value)));
            }
            StateProj::Fut(f) => match f.poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(res) => {
                    let res = tryit!(res);
                    let mut items = tryit!(this.generator.untangle_response(res.inner));
                    if let Some(item) = items.pop() {
                        this.state.set(State::values(items, res.cont));
                        return Poll::Ready(Some(Ok(item)));
                    } else {
                        assert!(res.cont.is_none(), "Cannot continue without return value");
                        return Poll::Ready(None);
                    }
                }
            },
            StateProj::Done => return Poll::Ready(None),
        };

        let req = this.generator.bot().client.get(url).send_parse();

        this.state.set(State::Fut(req));

        self.poll_next(cx)
    }
}

pub trait WikiGenerator {
    type Item: 'static;
    type Response: DeserializeOwned;
    fn bot(&self) -> &Bot;
    fn create_request(&self) -> Main;
    fn untangle_response(&self, res: Self::Response) -> crate::Result<Vec<Self::Item>>;
    fn into_stream(self) -> GeneratorStream<Self>
    where
        Self: Sized,
    {
        GeneratorStream {
            generator: self,
            state: State::Init,
        }
    }
}

/// GENeric GENerator, use this to create your own continuable requests
pub struct GenGen<State, C, U, Response, Item> {
    pub bot: Bot,
    pub state: State,
    create_request: C,
    untangle_response: U,
    _phtm: PhantomData<fn() -> (Response, Item)>,
}

impl<State, C, U, Response, Item> GenGen<State, C, U, Response, Item>
where
    C: Fn(&Bot, &State) -> Main,
    U: Fn(&Bot, &State, Response) -> crate::Result<Vec<Item>>,
    Response: DeserializeOwned,
{
    pub fn new(bot: Bot, state: State, create_request: C, untangle_response: U) -> Self {
        Self {
            bot,
            state,
            create_request,
            untangle_response,
            _phtm: PhantomData,
        }
    }
}

impl<State, C, U, Response, Item> WikiGenerator for GenGen<State, C, U, Response, Item>
where
    C: Fn(&Bot, &State) -> Main,
    U: Fn(&Bot, &State, Response) -> crate::Result<Vec<Item>>,
    Response: DeserializeOwned,
    Item: 'static,
{
    type Item = Item;
    type Response = Response;

    fn bot(&self) -> &Bot {
        &self.bot
    }

    fn create_request(&self) -> Main {
        (self.create_request)(self.bot(), &self.state)
    }

    fn untangle_response(&self, res: Self::Response) -> crate::Result<Vec<Self::Item>> {
        (self.untangle_response)(self.bot(), &self.state, res)
    }
}

pub struct SearchGenerator {
    bot: Bot,
    search: String,
}

impl WikiGenerator for SearchGenerator {
    type Item = BasicSearchResult;
    type Response = api::QueryResponse<api::Search<BasicSearchResult>>;

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

    fn untangle_response(&self, res: Self::Response) -> crate::Result<Vec<Self::Item>> {
        Ok(res.query.search)
    }
}

impl SearchGenerator {
    pub fn new(bot: Bot, search: String) -> Self {
        Self { search, bot }
    }
}

pub struct RecentChangesGenerator {
    bot: Bot,
    rc: ListRc,
}

impl RecentChangesGenerator {
    pub fn new(bot: Bot, rc: ListRc) -> Self {
        Self { bot, rc }
    }
}

impl WikiGenerator for RecentChangesGenerator {
    type Item = RecentChangesResult;
    type Response = api::QueryResponse<api::RecentChanges<RecentChangesResult>>;
    fn bot(&self) -> &Bot {
        &self.bot
    }
    fn create_request(&self) -> Main {
        Main::query(Query {
            list: Some(QueryList::RecentChanges(self.rc.clone()).into()),
            ..Default::default()
        })
    }
    fn untangle_response(&self, res: Self::Response) -> crate::Result<Vec<Self::Item>> {
        Ok(res.query.recent_changes)
    }
}
