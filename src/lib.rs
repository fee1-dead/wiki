use std::{borrow::Cow, convert::Infallible, num::NonZeroU16, ops::Deref, sync::Arc};

use req::{encode_multivalue, HasValue};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client, Url,
};
use serde::Deserialize;

pub mod api;
pub mod req;

pub trait WriteUrlParams {
    fn ser<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E>;
}

pub trait WriteUrlValue {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E>;
    fn ser_additional_only<W: UrlParamWriter>(&self, _w: &mut W) -> Result<(), W::E> {
        Ok(())
    }
}

pub struct BufferedName<'a, T: ?Sized> {
    s: &'a mut T,
    name: TriStr<'a>,
}

impl<'a, T: UrlParamWriter> BufferedName<'a, T> {
    pub fn write(self, value: TriStr<'_>) -> Result<&'a mut T, T::E> {
        self.s.add(self.name, value)?;
        Ok(self.s)
    }
}

pub enum TriStr<'a> {
    Shared(&'a str),
    Owned(String),
    Static(&'static str),
}

impl Deref for TriStr<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(s) => s,
            Self::Static(s) => s,
            Self::Shared(s) => s,
        }
    }
}

impl From<TriStr<'_>> for Cow<'static, str> {
    fn from(s: TriStr<'_>) -> Self {
        match s {
            TriStr::Shared(s) => Self::Owned(s.to_owned()),
            TriStr::Owned(s) => Self::Owned(s),
            TriStr::Static(s) => Self::Borrowed(s),
        }
    }
}

pub trait UrlParamWriter {
    type E;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), Self::E>;
    fn fork<'a>(&'a mut self, name: TriStr<'a>) -> BufferedName<'a, Self> {
        BufferedName { s: self, name }
    }
}

pub trait NamedEnum {
    fn variant_name(&self) -> &'static str;
}

#[derive(Default)]
pub struct Simple(pub String);

impl UrlParamWriter for Simple {
    type E = Infallible;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), Self::E> {
        if !self.0.is_empty() {
            self.0.push('&');
        }
        self.0.push_str(&urlencoding::encode(&*name));
        self.0.push('=');
        self.0.push_str(&urlencoding::encode(&*value));
        Ok(())
    }
}

impl UrlParamWriter for reqwest::multipart::Form {
    type E = Infallible;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), Self::E> {
        *self = std::mem::take(self).text(name, value);
        Ok(())
    }
}

pub struct SerdeAdaptor<T>(pub T);

impl<T: serde::ser::SerializeSeq> UrlParamWriter for SerdeAdaptor<T> {
    type E = T::Error;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), T::Error> {
        self.0.serialize_element(&(&*name, &*value))
    }
}

macro_rules! display_impls {
    ($($ty:ty),*$(,)?) => {$(
        impl WriteUrlValue for $ty {
            fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
                w.write(TriStr::Owned(self.to_string()))?;
                Ok(())
            }
        }
    )*};
}

display_impls! {
    u32,
    usize,
    NonZeroU16,
}

impl WriteUrlValue for String {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        w.write(TriStr::Shared(self))?;
        Ok(())
    }
}

impl<T: WriteUrlValue> WriteUrlValue for Option<T> {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        if let Some(this) = self {
            this.ser(w)?;
        }
        Ok(())
    }
    fn ser_additional_only<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E> {
        if let Some(this) = self {
            this.ser_additional_only(w)?;
        }
        Ok(())
    }
}

impl WriteUrlValue for bool {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        if *self {
            w.write(TriStr::Static(""))?;
        }
        Ok(())
    }
}

impl<T: WriteUrlValue + HasValue> WriteUrlValue for Vec<T> {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        if self.is_empty() {
            return Ok(());
        }
        let s = encode_multivalue(self);
        let w = w.write(TriStr::Owned(s))?;
        self.ser_additional_only(w)
    }

    fn ser_additional_only<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E> {
        for v in self {
            v.ser_additional_only(w)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Site {
    client: reqwest::Client,
    url: Url,
}

#[derive(Deserialize, Debug)]
pub struct QueryResponse<Q> {
    pub query: Q,
}

pub struct Page {
    content: String,
    id: usize,
    latest_revision: usize,
    changed: bool,
    bot: Option<Bot>,
}

impl Page {
    pub fn content_mut(&mut self) -> &mut String {
        self.changed = true;
        &mut self.content
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// BotInn TM
pub struct BotInn {
    site: Site,
    #[allow(unused)]
    pass: BotPassword,
}

/// A bot that is logged in.
#[derive(Clone)]
pub struct Bot {
    inn: Arc<BotInn>,
}

#[derive(Clone)]
pub struct BotPassword {
    username: String,
    password: String,
}

impl BotPassword {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

const UA: &str = concat!(
    "wiki.rs",
    "/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/fee1-dead/wiki.rs ent3rm4n@gmail.com)"
);

impl Site {
    pub fn new(api_url: &'static str) -> Result<Self> {
        let url: Url = api_url.parse()?;
        assert!(url.query().is_none());
        let mut client = Client::builder();
        client = client;
        #[cfg(feature = "default")]
        {
            client = client.cookie_store(true).user_agent(UA);
        }
        let client = client.build()?;
        let mut headers = HeaderMap::new();
        headers = headers;
        headers.insert("Api-User-Agent", HeaderValue::from_static(UA));

        Ok(Site { client, url })
    }

    pub fn enwiki() -> Self {
        Site::new("https://en.wikipedia.org/w/api.php").unwrap()
    }

    pub fn testwiki() -> Self {
        Site::new("https://test.wikipedia.org/w/api.php").unwrap()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
