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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    use crate::types::NowableTime;

    #[test]
    fn works() -> Result<(), Box<dyn Error>> {
        #[derive(serde::Serialize)]
        pub struct Testing {
            time: NowableTime,
        }

        let j = serde_json::to_value(Testing {
            time: NowableTime::Now,
        })?;
        assert_eq!(j, serde_json::json!({ "time": "now" }));

        let j = serde_json::to_value(Testing {
            time: DateTime::from_utc(
                NaiveDateTime::new(
                    NaiveDate::from_ymd(1337, 1, 3),
                    NaiveTime::from_hms(3, 7, 0),
                ),
                Utc,
            )
            .into(),
        })?;
        assert_eq!(j, serde_json::json!({ "time": "1337-01-03T03:07:00Z" }));

        Ok(())
    }
}
