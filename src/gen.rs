use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;

use crate::req::{Main, Query};
use crate::{Bot, Page};

pub struct SearchGenerator {
    pub search: String,
    bot: Bot,
}

impl SearchGenerator {
    pub fn new(bot: Bot, search: String) -> Self {
        Self { search, bot }
    }
}

impl Stream for SearchGenerator {
    type Item = Page;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let m = Main::query(Query {
            ..Default::default()
        });
        todo!()
    }
}
