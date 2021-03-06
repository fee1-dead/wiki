use std::collections::HashMap;
use std::future::Future;

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures_util::TryStreamExt;
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::{interval, Interval, MissedTickBehavior};
use tracing::{debug, trace};

use crate::generators::{GenGen, WikiGenerator};
use crate::jobs::{create_server, JobRunner};
use crate::req::{
    self, Login, Main, MetaUserInfo, PageSpec, QueryMeta, QueryProp,
    QueryPropRevisions, RvProp, RvSlot, TokenType, UserInfoProp,
};
use crate::res::PageResponse;
use crate::url::WriteUrlParams;
use crate::{BotPassword, Result, AccessExt};

#[macro_export]
macro_rules! basic {
    (@handle( $i:ident { $name:ident: $ty:ty } )) => {
        #[derive(Deserialize, Debug)]
        pub struct $i {
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
    ($($ty:ident { $($tt:tt)* })*) => {
        $(basic!(@handle($ty { $($tt)* }));)*
    };
}

basic! {
    SlotsMain { main: Slot }
    QueryResponse { query }
    AbuseLog { T => abuse_log["abuselog"]: Vec<T> }
    Search { T => search: Vec<T> }
    RecentChanges { T => recent_changes["recentchanges"]: Vec<T> }
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
            fn types() -> &'static [TokenType] { &[$($t),+] }
        }
    };
}

token!(LoginToken = "logintoken" = [TokenType::Login] + token);
token!(CsrfToken = "csrftoken" = [TokenType::Csrf] + token);

pub trait Token: DeserializeOwned {
    fn types() -> &'static [TokenType];
}

pub trait TokenExt: Token {
    const LEN: usize;
    const QUERY: &'static str;
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
    url.set_query(Some(&q.0));
    debug!(%url, "GET");
    Ok(url)
}

pub trait RequestBuilderExt: Sized {
    fn send_and_report_err(
        self,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Value>> + Send + Sync>>;
    fn send_parse<D: DeserializeOwned>(
        self,
    ) -> Pin<Box<dyn Future<Output = crate::Result<D>> + Send + Sync>>
    where
        Self: Send + Sync + 'static,
    {
        Box::pin(async move {
            let v = self.send_and_report_err().await?;
            Ok(serde_json::from_value(v)?)
        })
    }
}

impl RequestBuilderExt for reqwest::RequestBuilder {
    fn send_and_report_err(
        self,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Value>> + Send + Sync>> {
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

pub async fn fetch(client: &Client, url: Url, spec: PageSpec) -> Result<crate::Page> {
    let mut q = req::Query {
        prop: Some(
            QueryProp::Revisions(QueryPropRevisions {
                prop: [RvProp::Ids, RvProp::Content].into(),
                slots: [RvSlot::Main].into(),
                limit: req::Limit::Value(1),
            })
            .into(),
        ),
        ..Default::default()
    };
    match spec {
        PageSpec::Title { title } => q.titles = vec![title].into(),
        PageSpec::Id { pageid } => q.pageids = vec![pageid].into(),
    };

    let m = Main::query(q);

    let url = mkurl(url, m);
    let res = client.get(url).send_and_report_err().await?;
    trace!("{res}");
    let body: QueryResponse<Revisions<SlotsMain>> = serde_json::from_value(res).unwrap();
    let mut pages = body.query.pages.into_iter();
    let (_, page) = pages.next().expect("page to exist");
    assert!(pages.next().is_none());

    let [rev]: [_; 1] = page.revisions.try_into().unwrap();

    Ok(crate::Page {
        content: rev.slots.main.content,
        latest_revision: rev.rev_id,
        id: page.page_id,
        changed: false,
        bot: None,
    })
}

pub async fn get_tokens<T: Token>(url: Url, client: &Client) -> Result<T> {
    let res = client
        .get(mkurl(url, Main::tokens(T::types())))
        .send()
        .await?;
    let tokens: QueryResponse<Tokens<T>> = res.json().await?;
    Ok(tokens.query.tokens)
}

pub struct BotOptions {
    highlimits: bool,
}

impl crate::Site {
    pub fn mkurl(&self, m: Main) -> Url {
        mkurl(self.url.clone(), m)
    }

    pub async fn get_tokens<T: Token>(&self) -> Result<T> {
        get_tokens(self.url.clone(), &self.client).await
    }

    /// Returns a page with the latest revision.
    pub async fn fetch(&self, spec: PageSpec) -> Result<crate::Page> {
        fetch(&self.client, self.url.clone(), spec).await
    }

    pub async fn login(
        self,
        password: BotPassword,
        editdelay: Duration,
    ) -> Result<(crate::Bot, JobRunner), (Self, crate::Error)> {
        async fn login_(
            this: &crate::Site,
            BotPassword { username, password }: BotPassword,
        ) -> Result<BotOptions> {
            let LoginToken { token } = this.get_tokens::<LoginToken>().await?;
            let req = this.client.post(this.url.clone());
            let l = Main::login(Login {
                name: username,
                password,
                token,
            });
            let form = l.build_form();
            let v: Value = req.multipart(form).send_and_report_err().await?;
            debug!("{v}");
            if v.get("login")
                .and_then(|v| v.get("result"))
                .map_or(false, |v| v == "Success")
            {
                let url = this.mkurl(Main::query(req::Query {
                    meta: Some(
                        QueryMeta::UserInfo(MetaUserInfo {
                            prop: UserInfoProp::Rights.into(),
                        })
                        .into(),
                    ),
                    ..Default::default()
                }));
                let res: QueryResponse<UserInfo<UserInfoRights>> =
                    this.client.get(url).send_parse().await?;

                // TODO does highlimits need to be here
                Ok(BotOptions {
                    highlimits: res
                        .query
                        .userinfo
                        .extra
                        .rights
                        .iter()
                        .any(|s| s == "apihighlimits"),
                })
            } else {
                panic!("Vandalism detected. Your actions will be logged at [[WP:LTA/BotAbuser]]")
            }
        }
        let res = login_(&self, password.clone()).await;
        let mut interval = interval(editdelay);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        match res {
            Ok(options) => {
                let (queue, r) = create_server(self.client.clone());
                Ok((
                    crate::Bot {
                        inn: Arc::new(crate::BotInn {
                            pass: password,
                            control: Mutex::new(interval),
                            url: self.url,
                            options,
                        }),
                        queue,
                        client: self.client,
                    },
                    r,
                ))
            }
            Err(e) => Err((self, e)),
        }
    }
}

impl crate::Access for crate::Site {
    fn client(&self) -> &Client {
        &self.client
    }
    fn url(&self) -> &Url {
        &self.url
    }
}

pub type QueryAllGenerator<A> = GenGen<
    A,
    Main,
    fn(&Url, &Client, &Main) -> Main,
    fn(&Url, &Client, &Main, Value) -> Result<Vec<Value>>,
    Value,
    Value,
>;

impl crate::Bot {
    pub async fn fetch(&self, spec: PageSpec) -> Result<crate::Page> {
        fetch(&self.client, self.inn.url.clone(), spec)
            .await
            .map(|p| crate::Page {
                bot: Some(self.clone()),
                ..p
            })
    }

    pub async fn page_info_all<E: DeserializeOwned>(
        &self,
        mut query: req::Query,
        spec: PageSpec,
    ) -> Result<PageResponse<E>> {
        query.pageids = None;
        query.titles = None;
        match spec {
            PageSpec::Id { pageid } => query.pageids = Some(vec![pageid]),
            PageSpec::Title { title } => query.titles = Some(vec![title]),
        }

        self.query_all(query)
            .try_fold(Value::Null, |mut acc, new| async {
                crate::util::merge_values(&mut acc, new);
                Ok(acc)
            })
            .await
            .and_then(|v| Ok(serde_json::from_value(v)?))
    }

    pub async fn control(&self) -> MutexGuard<'_, Interval> {
        self.inn.control.lock().await
    }

    pub fn options(&self) -> &BotOptions {
        &self.inn.options
    }
}

impl crate::Page {
    pub async fn save(&self, summary: &str) -> Result<()> {
        if let Some(bot) = &self.bot {
            if !self.changed {
                return Ok(());
            }

            let u = bot.inn.url.clone();
            let t = get_tokens::<CsrfToken>(bot.inn.url.clone(), &bot.client).await?;
            let m = Main::edit(req::Edit {
                spec: req::PageSpec::Id { pageid: self.id },
                summary: summary.to_owned(),
                text: self.content.to_owned(),
                baserevid: self.latest_revision,
                token: t.token,
            });
            let f = m.build_form();
            let res = bot.client.post(u).multipart(f).send().await?.text().await?;
            dbg!(res);

            bot.control().await.tick().await;

            Ok(())
        } else {
            panic!("User is not logged in. This action will be logged.")
        }
    }

    pub async fn refetch(&mut self) -> Result<()> {
        if let Some(bot) = &self.bot {
            let f = bot.fetch(PageSpec::Id { pageid: self.id }).await?;
            *self = f;
            Ok(())
        } else {
            panic!("I will come up with better panic messages next time")
        }
    }
}
