use std::borrow::Cow;
use std::convert::Infallible;
use std::ops::{BitAnd, BitOr, BitOrAssign};

use crate::req;

pub enum TriStr<'a> {
    Shared(&'a str),
    Owned(String),
    Static(&'static str),
}

pub trait UrlParamWriter {
    type E;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), Self::E>;
    fn fork<'a>(&'a mut self, name: TriStr<'a>) -> BufferedName<'a, Self> {
        BufferedName { s: self, name }
    }
}

pub trait WriteUrlParams {
    fn ser<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E>;
}

pub trait WriteUrlValue {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E>;
    /// only write the extra values, excluding names.
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

pub trait NamedEnum {
    fn variant_name(&self) -> &'static str;
}

pub trait BitflaggedEnum {
    type Bitflag: Copy
        + BitAnd<Output = Self::Bitflag>
        + BitOr<Output = Self::Bitflag>
        + BitOrAssign
        + Default
        + Eq;
    fn flag(&self) -> Self::Bitflag;
}

#[derive(Default)]
pub struct Simple(pub String);

impl Simple {
    pub fn add_serde<T: serde::Serialize>(
        &mut self,
        x: T,
    ) -> Result<(), serde_urlencoded::ser::Error> {
        let s = serde_urlencoded::to_string(x)?;
        if !s.is_empty() {
            if !self.0.is_empty() {
                self.0.push('&');
            }
            self.0.push_str(&s);
        }
        Ok(())
    }
}

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

impl WriteUrlValue for String {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        w.write(TriStr::Shared(self)).map(|_| ())
    }
}

impl WriteUrlValue for Cow<'static, str> {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        w.write(match self {
            Self::Borrowed(s) => TriStr::Static(s),
            Self::Owned(s) => TriStr::Shared(s),
        })
        .map(|_| ())
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

impl<T: WriteUrlParams> WriteUrlParams for Option<T> {
    fn ser<W: UrlParamWriter>(&self, w: &mut W) -> Result<(), W::E> {
        if let Some(this) = self {
            this.ser(w)?;
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

impl<T: WriteUrlValue + req::HasValue> WriteUrlValue for Vec<T> {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        if self.is_empty() {
            return Ok(());
        }
        let s = req::encode_multivalue(self);
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

pub struct PrependAdaptor<'a, T> {
    inner: T,
    prep: &'a str,
}

impl<T: UrlParamWriter> UrlParamWriter for PrependAdaptor<'_, T> {
    type E = T::E;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), Self::E> {
        let p = self.prep;
        self.inner.add(format!("{p}{name}").into(), value)
    }
}

impl<'a, T> PrependAdaptor<'a, T> {
    pub fn new(inner: T, prep: &'a str) -> Self {
        PrependAdaptor { inner, prep }
    }
}
