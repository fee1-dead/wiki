use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::future::BoxFuture;
use futures_util::Stream;

use crate::api::{Revisions, SlotsMain, RequestBuilderExt};
use crate::req::{
    self, EnumSet, Main, Query, QueryGenerator, QueryProp, QueryPropRevisions, RvProp, RvSlot,
};
use crate::{api, Bot, Page};

pub type BoxReqFuture = BoxFuture<'static, reqwest::Result<reqwest::Response>>;
pub type BoxRecvFuture = BoxFuture<'static, reqwest::Result<api::Query<Revisions<SlotsMain>>>>;

pub enum State {
    Init,
    Req(BoxReqFuture),
    Recv(BoxRecvFuture),
    Errored,
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
    pub fn into_stream(self) -> impl Stream<Item = crate::Result<Page>> {

        futures_util::stream::unfold(self, |mut this| async move {
            macro_rules! tryit {
                ($e:expr) => {
                    match $e {
                        Ok(very_well) => very_well,
                        Err(e) => {
                            this.state = State::Errored;
                            return Some((Err(e.into()), this));
                        }
                    }
                };
            }
            if let State::Init = this.state {
                let m = Main::query(Query {
                    prop: Some(
                        QueryProp::Revisions(QueryPropRevisions {
                            prop: [RvProp::Content, RvProp::Ids].into(),
                            slots: RvSlot::Main.into(),
                            limit: None,
                        })
                        .into(),
                    ),
                    generator: Some(QueryGenerator::Search(req::SearchGenerator {
                        search: this.search.to_owned(),
                        limit: 500,
                        offset: None,
                    })),
                    ..Default::default()
                });
                let u = this.bot.mkurl(m);
                let r = tryit!(this.bot.client.get(u).send_and_report_err().await);
                let res: api::Query<Revisions<SlotsMain>> = tryit!(serde_json::from_value(r));
            }
            todo!()
        })
    }
}
