use std::{collections::HashMap, num::NonZeroU16, sync::Arc};

use reqwest::Url;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;

use crate::{
    req::{self, Login, Main, QueryProp, RvProp, RvSlot, TokenType, QueryPropRevisions},
    BotPassword, Result, Simple, WriteUrlParams,
};

#[derive(Deserialize, Debug)]
pub struct Slot {
    #[serde(rename = "contentmodel")]
    pub content_model: String,
    #[serde(rename = "contentformat")]
    pub content_format: String,
    #[serde(rename = "*")]
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub struct SlotsMain {
    pub main: Slot,
}

#[derive(Deserialize, Debug)]
pub struct Revision<S> {
    #[serde(rename = "revid")]
    pub rev_id: usize,
    #[serde(rename = "parentid")]
    pub parent_id: usize,
    pub slots: S,
}

#[derive(Deserialize, Debug)]
pub struct Query<Q> {
    pub query: Q,
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
    pub page_id: usize,
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

impl crate::Site {
    fn mkurl(&self, m: Main) -> Url {
        let mut url = self.url.clone();
        let mut q = Simple::default();
        if let Err(e) = m.ser(&mut q) {
            match e {}
        }
        url.set_query(Some(&q.0));
        println!("{url}");
        url
    }

    pub async fn get_tokens<T: Token>(&self) -> Result<T> {
        let res = self
            .client
            .get(self.mkurl(Main::tokens(T::types())))
            .send()
            .await?;
        let tokens: Query<Tokens<T>> = res.json().await?;
        Ok(tokens.query.tokens)
    }

    /// Returns a page with the latest revision.
    pub async fn fetch(&self, name: &str) -> Result<crate::Page> {
        let name = name.replace(' ', "_");
        let m = Main::query(req::Query {
            prop: QueryProp::Revisions(QueryPropRevisions {
                prop: [RvProp::Ids, RvProp::Content].into(),
                slots: [RvSlot::Main].into(),
                limit: NonZeroU16::new(1).unwrap(),
            })
            .into(),
            titles: vec![name],
            ..Default::default()
        });
        let url = self.mkurl(m);
        let res = self.client.get(url).send().await?;
        let t = res.text().await?;
        dbg!(&t);
        let body: Query<Revisions<SlotsMain>> = serde_json::from_str(&t).unwrap();
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

    pub async fn login(self, password: BotPassword) -> Result<crate::Bot, (Self, crate::Error)> {
        async fn login_(
            this: &crate::Site,
            BotPassword { username, password }: BotPassword,
        ) -> Result<()> {
            let LoginToken { token } = this.get_tokens::<LoginToken>().await?;
            let req = this.client.post(this.url.clone());
            let l = Main::login(Login {
                name: username,
                password,
                token,
            });
            let form = l.build_form();
            let v: Value = req.multipart(form).send().await?.json().await?;
            if v.get("login")
                .and_then(|v| v.get("result"))
                .map_or(false, |v| v == "Success")
            {
                Ok(())
            } else {
                panic!("Vandalism detected. Your actions will be logged at [[WP:LTA/BotAbuser]]")
            }
        }
        let res = login_(&self, password.clone()).await;
        match res {
            Ok(()) => Ok(crate::Bot {
                inn: Arc::new(crate::BotInn {
                    site: self,
                    pass: password,
                }),
            }),
            Err(e) => Err((self, e)),
        }
    }
}

impl crate::Bot {
    pub async fn fetch(&self, name: &str) -> Result<crate::Page> {
        self.inn.site.fetch(name).await.map(|p| crate::Page {
            bot: Some(self.clone()),
            ..p
        })
    }
}

impl crate::Page {
    pub async fn save(&self, newtext: &str, summary: &str) -> Result<()> {
        if let Some(bot) = &self.bot {
            let u = bot.inn.site.url.clone();
            let t = bot.inn.site.get_tokens::<CsrfToken>().await?;
            let m = Main::edit(req::Edit {
                spec: req::PageSpec::Id { pageid: self.id },
                summary: summary.to_owned(),
                text: newtext.to_owned(),
                baserevid: self.latest_revision,
                token: t.token,
            });
            let f = m.build_form();
            let res = bot
                .inn
                .site
                .client
                .post(u)
                .multipart(f)
                .send()
                .await?
                .text()
                .await?;
            dbg!(res);

            Ok(())
        } else {
            panic!("User is not logged in. This action will be logged.")
        }
    }
}
