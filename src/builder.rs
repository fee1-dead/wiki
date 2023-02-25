use std::borrow::Cow;
use std::marker::PhantomData;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use http_types::Url;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::ClientBuilder;
use serde_json::Value;
use tracing::{debug, info};

use crate::api::{LoginToken, QueryResponse, RequestBuilderExt, UserInfo, UserInfoInner};
use crate::req::{self, Login, Main};
use crate::sealed::Access;
use crate::{AnonymousAccess, AuthorizedAccess, BotPassword, Client, Result, UA};

pub struct SiteBuilder<A: Access> {
    url: String,
    client: ClientBuilder,
    user_agent: Option<Cow<'static, str>>,
    oauth: Option<String>,
    password: Option<BotPassword>,
    _ph: PhantomData<A>,
}

impl<A: Access> SiteBuilder<A> {
    pub fn user_agent(mut self, ua: impl Into<Cow<'static, str>>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }
}

impl SiteBuilder<AnonymousAccess> {
    // creation of new sites. Only get anonymous access since not logged in.
    pub fn new(api_url: &str) -> Self {
        let client = reqwest::Client::builder();

        Self {
            client,
            url: api_url.to_owned(),
            user_agent: None,
            oauth: None,
            password: None,
            _ph: PhantomData,
        }
    }

    pub fn enwiki() -> Self {
        Self::new("https://en.wikipedia.org/w/api.php")
    }

    pub fn test_wikipedia() -> Self {
        Self::new("https://test.wikipedia.org/w/api.php")
    }

    pub fn test_miraheze() -> Self {
        Self::new("https://publictestwiki.com/w/api.php")
    }

    pub fn password(self, pass: BotPassword) -> SiteBuilder<AuthorizedAccess> {
        SiteBuilder {
            url: self.url,
            client: self.client,
            user_agent: self.user_agent,
            oauth: None,
            password: Some(pass),
            _ph: PhantomData,
        }
    }

    /// to login via oauth, go to
    /// https://meta.wikimedia.org/wiki/Special:OAuthConsumerRegistration/propose/oauth2
    /// and create an owner-only application.
    pub fn oauth(self, token: impl Into<String>) -> SiteBuilder<AuthorizedAccess> {
        SiteBuilder {
            url: self.url,
            client: self.client,
            user_agent: self.user_agent,
            oauth: Some(token.into()),
            password: None,
            _ph: PhantomData,
        }
    }

    /// build anonymous access
    pub fn build(mut self) -> Result<Client<AnonymousAccess>> {
        let url: Url = self.url.parse()?;
        assert!(url.query().is_none());
        let ua = self.user_agent.as_deref().unwrap_or(UA);

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.client = self.client.cookie_store(true).user_agent(ua);
        }

        // TODO add cookie store support once lands in reqwest
        // https://github.com/seanmonstar/reqwest/pull/1449
        #[cfg(target_arch = "wasm32")]
        {
            let mut headers = HeaderMap::new();
            headers.insert("Api-User-Agent", HeaderValue::from_str(ua)?);
            self.client = self.client.default_headers(headers);
        }

        Ok(Client {
            client: self.client.build()?,
            url,
            acc: PhantomData,
        })
    }
}

impl SiteBuilder<AuthorizedAccess> {
    /// build by logging in.
    pub async fn build(mut self) -> Result<Client<AuthorizedAccess>> {
        let url: Url = self.url.parse()?;
        assert!(url.query().is_none());
        let ua = self.user_agent.as_deref().unwrap_or(UA);

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.client = self.client.cookie_store(true).user_agent(ua);
        }

        let mut headers = HeaderMap::new();

        #[cfg(target_arch = "wasm32")]
        {
            headers.insert("Api-User-Agent", HeaderValue::from_str(ua)?);
        }

        if let Some(token) = self.oauth {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))?,
            );
        }

        self.client = self.client.default_headers(headers);

        let site = Client {
            client: self.client.build()?,
            url,
            acc: PhantomData,
        };

        if let Some(pass) = self.password {
            let LoginToken { token } = site.get_tokens::<LoginToken>().await?;
            let req = site.client.post(site.url.clone());
            let l = Main::login(Login {
                name: pass.username,
                password: pass.password,
                token,
            });
            let form = l.build_form();
            let v: Value = req.multipart(form).send_and_report_err().await?;
            debug!("{v}");
            if !v
                .get("login")
                .and_then(|v| v.get("result"))
                .map_or(false, |v| v == "Success")
            {
                panic!("Vandalism detected. Your actions will be logged at [[WP:LTA/BotAbuser]]")
            }
        }

        // we have built the site, now we need to check that we are actually logged in.
        let QueryResponse {
            query:
                UserInfo {
                    userinfo:
                        UserInfoInner {
                            id,
                            name,
                            extra: (),
                        },
                },
        } = site
            .client
            .execute(
                site.get(req::Action::Query(req::Query {
                    meta: Some(req::QueryMeta::UserInfo(req::MetaUserInfo { prop: None }).into()),
                    ..Default::default()
                }))
                .build()?,
            )
            .await?
            .json()
            .await?;

        info!("Logged in as \"{name}\" (id {id})");

        // if we are an IP, then we are definitely not logged in.
        if Ipv4Addr::from_str(&name).is_ok() || Ipv6Addr::from_str(&name).is_ok() {
            return Err(crate::Error::Unauthorized);
        }

        Ok(site)
    }
}
