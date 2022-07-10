//! This is really boring. Move on.

use std::borrow::Cow;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroUsize};
use std::ops::Deref;

use crate::req::PageSpec;
use crate::url::{BufferedName, TriStr, UrlParamWriter, WriteUrlValue};

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

impl fmt::Display for TriStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl From<String> for TriStr<'_> {
    fn from(s: String) -> Self {
        Self::Owned(s)
    }
}

impl<'w, T: UrlParamWriter> UrlParamWriter for &'w mut T {
    type E = T::E;
    fn add(&mut self, name: TriStr<'_>, value: TriStr<'_>) -> Result<(), Self::E> {
        (*self).add(name, value)
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
    u8,
    u16,
    u32,
    u64,
    usize,
    NonZeroU16,
    NonZeroU32,
    NonZeroU64,
    NonZeroUsize,
}

impl From<&'_ str> for PageSpec {
    fn from(s: &'_ str) -> Self {
        Self::Title {
            title: s.to_owned(),
        }
    }
}
