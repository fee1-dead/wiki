use std::pin::Pin;
use std::task::Poll;

use chrono::{DateTime, Duration, Utc};
use futures_util::{Future, Stream};

use super::{GeneratorStream, RecentChangesGenerator, WikiGenerator};
use crate::api::RecentChangesResult;
use crate::req::rc::{ListRc, RcProp, RcType};
use crate::req::Limit;
use crate::Bot;

#[pin_project::pin_project(project = StateProj)]
pub enum State {
    Stream(#[pin] GeneratorStream<RecentChangesGenerator>),
    Sleep(Pin<Box<tokio::time::Sleep>>),
}

#[pin_project::pin_project]
pub struct RecentChangesPatroller {
    bot: Bot,
    prev_time: DateTime<Utc>,
    #[pin]
    state: State,
    interval: tokio::time::Duration,
    errored: bool,
    prop: RcProp,
    ty: RcType,
}

impl RecentChangesPatroller {
    pub fn new(bot: Bot, interval: tokio::time::Duration, prop: RcProp, ty: RcType) -> Self {
        let prev_time = Self::now();
        let state = State::Sleep(Box::pin(tokio::time::sleep(interval)));
        Self {
            bot,
            prev_time,
            state,
            interval,
            errored: false,
            prop,
            ty,
        }
    }
    fn now() -> DateTime<Utc> {
        Utc::now() - Duration::seconds(1)
    }
}

impl Stream for RecentChangesPatroller {
    type Item = crate::Result<RecentChangesResult>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.as_mut().project();
        if *this.errored {
            return Poll::Ready(None);
        }
        match this.state.project() {
            StateProj::Sleep(s) => match s.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    let timestamp = Self::now();
                    let gen = RecentChangesGenerator::new(
                        self.bot.clone(),
                        ListRc {
                            start: Some(timestamp.into()),
                            end: Some(self.prev_time.into()),
                            limit: Limit::Max,
                            prop: self.prop,
                            ty: self.ty,
                        },
                    );
                    self.prev_time = timestamp;
                    self.state = State::Stream(gen.into_stream());
                    self.poll_next(cx)
                }
            },
            StateProj::Stream(s) => match s.poll_next(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(None) => {
                    self.state = State::Sleep(Box::pin(tokio::time::sleep(self.interval)));
                    self.poll_next(cx)
                }
                Poll::Ready(Some(Err(e))) => {
                    *this.errored = true;
                    Poll::Ready(Some(Err(e)))
                }
                Poll::Ready(Some(Ok(i))) => Poll::Ready(Some(Ok(i))),
            },
        }
    }
}
