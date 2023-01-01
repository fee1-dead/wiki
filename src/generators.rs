use std::future::Future;
use std::marker::PhantomData;
use std::mem::take;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::Stream;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::{trace, trace_span};

use crate::api::{
    BasicSearchResult, BoxFuture, MaybeContinue, RecentChangesResult, RequestBuilderExt, Revisions,
    SlotsMain,
};
use crate::req::rc::ListRc;
use crate::req::search::{ListSearch, SearchInfo, SearchProp};
use crate::req::{self, Main, Query, QueryList};
use crate::sealed::Access;
use crate::{api, Site};

pub type BoxReqFuture = BoxFuture<reqwest::Result<reqwest::Response>>;
pub type BoxRecvFuture = BoxFuture<reqwest::Result<api::QueryResponse<Revisions<SlotsMain>>>>;

pub type ResponseFuture<G> =
    BoxFuture<crate::Result<MaybeContinue<<G as WikiGenerator>::Response>>>;

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
    span: tracing::span::Span,
}

impl<G: WikiGenerator> Stream for GeneratorStream<G> {
    type Item = crate::Result<G::Item>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.as_mut().project();
        let entered = this.span.enter();
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
                trace!("created request");
                let u = crate::api::mkurl(this.generator.url().clone(), main);
                trace!("created url");
                u
            }
            StateProj::Cont(v) => {
                let main = this.generator.create_request();
                trace!("created request");
                let u = tryit!(crate::api::mkurl_with_ext(
                    this.generator.url().clone(),
                    main,
                    v.take()
                ));
                trace!("created url");
                u
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
                    trace!("received request");
                    let res = tryit!(res);
                    let mut items = tryit!(this.generator.untangle_response(res.inner));
                    trace!("parsed response");
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

        let req = this.generator.client().get(url).send_parse();
        trace!("sent request");

        drop(entered);
        this.state.set(State::Fut(req));

        self.poll_next(cx)
    }
}

pub trait WikiGenerator {
    type Item: 'static;
    type Response: DeserializeOwned;
    fn url(&self) -> &Url;
    fn client(&self) -> &Client;
    fn create_request(&self) -> Main;
    fn untangle_response(&self, res: Self::Response) -> crate::Result<Vec<Self::Item>>;
    fn into_stream(self) -> GeneratorStream<Self>
    where
        Self: Sized,
    {
        GeneratorStream {
            generator: self,
            state: State::Init,
            span: trace_span!("stream"),
        }
    }
}

/// GENeric GENerator, use this to create your own continuable requests
pub struct GenGen<Access: crate::sealed::Access, State, C, U, Response, Item> {
    pub site: Site<Access>,
    pub state: State,
    create_request: C,
    untangle_response: U,
    _phtm: PhantomData<fn() -> (Response, Item)>,
}

impl<A, State, C, U, Response, Item> GenGen<A, State, C, U, Response, Item>
where
    A: Access,
    C: Fn(&Url, &Client, &State) -> Main,
    U: Fn(&Url, &Client, &State, Response) -> crate::Result<Vec<Item>>,
    Response: DeserializeOwned,
{
    pub fn new(site: Site<A>, state: State, create_request: C, untangle_response: U) -> Self {
        Self {
            site,
            state,
            create_request,
            untangle_response,
            _phtm: PhantomData,
        }
    }
}

impl<A, State, C, U, Response, Item> WikiGenerator for GenGen<A, State, C, U, Response, Item>
where
    A: Access,
    C: Fn(&Url, &Client, &State) -> Main,
    U: Fn(&Url, &Client, &State, Response) -> crate::Result<Vec<Item>>,
    Response: DeserializeOwned,
    Item: 'static,
{
    type Item = Item;
    type Response = Response;

    fn url(&self) -> &Url {
        &self.site.url
    }

    fn client(&self) -> &Client {
        &self.site.client
    }

    fn create_request(&self) -> Main {
        (self.create_request)(self.url(), self.client(), &self.state)
    }

    fn untangle_response(&self, res: Self::Response) -> crate::Result<Vec<Self::Item>> {
        (self.untangle_response)(self.url(), self.client(), &self.state, res)
    }
}

pub struct SearchGenerator<A: Access> {
    site: Site<A>,
    search: String,
}

impl<A: Access> WikiGenerator for SearchGenerator<A> {
    type Item = BasicSearchResult;
    type Response = api::QueryResponse<api::Search<BasicSearchResult>>;

    fn url(&self) -> &Url {
        &self.site.url
    }

    fn client(&self) -> &Client {
        &self.site.client
    }

    fn create_request(&self) -> Main {
        Main::query(Query {
            list: Some(
                QueryList::Search(ListSearch {
                    search: self.search.clone(),
                    limit: req::Limit::Max,
                    prop: SearchProp::empty(),
                    info: SearchInfo::empty(),
                    namespace: None,
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

impl<A: Access> SearchGenerator<A> {
    pub fn new(site: Site<A>, search: String) -> Self {
        Self { site, search }
    }
}

pub struct RecentChangesGenerator<A: Access> {
    site: Site<A>,
    rc: ListRc,
}

impl<A: Access> RecentChangesGenerator<A> {
    pub fn new(site: Site<A>, rc: ListRc) -> Self {
        Self { site, rc }
    }
}

impl<A: Access> WikiGenerator for RecentChangesGenerator<A> {
    type Item = RecentChangesResult;
    type Response = api::QueryResponse<api::RecentChanges<RecentChangesResult>>;
    fn url(&self) -> &Url {
        &self.site.url
    }
    fn client(&self) -> &Client {
        &self.site.client
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
