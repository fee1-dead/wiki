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
