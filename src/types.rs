use chrono::Utc;

use crate::url::{TriStr, WriteUrlValue};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MwTimestamp {
    Now,
    Timestamp(chrono::DateTime<Utc>),
}

fn format(time: &chrono::DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl serde::Serialize for MwTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Now => "now".serialize(serializer),
            Self::Timestamp(time) => format(time).serialize(serializer),
        }
    }
}

impl WriteUrlValue for MwTimestamp {
    fn ser<W: crate::macro_support::UrlParamWriter>(
        &self,
        w: crate::macro_support::BufferedName<'_, W>,
    ) -> Result<(), W::E> {
        w.write(match self {
            Self::Now => TriStr::Static("now"),
            Self::Timestamp(time) => format(time).into(),
        })
        .map(|_| {})
    }
}

impl From<chrono::DateTime<Utc>> for MwTimestamp {
    fn from(dt: chrono::DateTime<Utc>) -> Self {
        Self::Timestamp(dt)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    use crate::types::MwTimestamp;

    #[test]
    fn works() -> Result<(), Box<dyn Error>> {
        #[derive(serde::Serialize)]
        pub struct Testing {
            time: MwTimestamp,
        }

        let j = serde_json::to_value(Testing {
            time: MwTimestamp::Now,
        })?;
        assert_eq!(j, serde_json::json!({ "time": "now" }));

        let j = serde_json::to_value(Testing {
            time: MwTimestamp::Timestamp(DateTime::from_utc(
                NaiveDateTime::new(
                    NaiveDate::from_ymd(1337, 1, 3),
                    NaiveTime::from_hms(3, 7, 0),
                ),
                Utc,
            )),
        })?;
        assert_eq!(j, serde_json::json!({ "time": "1337-01-03T03:07:00Z" }));

        Ok(())
    }
}
