use chrono::Utc;

use crate::url::{BufferedName, TriStr, UrlParamWriter, WriteUrlValue};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NowableTime {
    Now,
    Timestamp(MwTimestamp),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct MwTimestamp(pub chrono::DateTime<Utc>);

fn format(time: &chrono::DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl serde::Serialize for MwTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format(&self.0).serialize(serializer)
    }
}

impl serde::Serialize for NowableTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Now => "now".serialize(serializer),
            Self::Timestamp(time) => time.serialize(serializer),
        }
    }
}

impl WriteUrlValue for MwTimestamp {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        w.write(format(&self.0).into()).map(|_| {})
    }
}

impl WriteUrlValue for NowableTime {
    fn ser<W: UrlParamWriter>(&self, w: BufferedName<'_, W>) -> Result<(), W::E> {
        match self {
            Self::Now => w.write(TriStr::Static("now")).map(|_| {}),
            Self::Timestamp(time) => time.ser(w),
        }
    }
}

impl From<chrono::DateTime<Utc>> for NowableTime {
    fn from(dt: chrono::DateTime<Utc>) -> Self {
        Self::Timestamp(MwTimestamp(dt))
    }
}

impl From<chrono::DateTime<Utc>> for MwTimestamp {
    fn from(x: chrono::DateTime<Utc>) -> Self {
        Self(x)
    }
}
