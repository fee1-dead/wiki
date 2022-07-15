use std::fmt;

use chrono::{DateTime, Utc};
use serde::Deserializer;
use serde_json::Value;

pub fn merge_values(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge_values(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (Value::Array(a), Value::Array(b)) => a.extend_from_slice(&b),
        (a, b) => *a = b,
    }
}

pub fn de_dt<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    pub struct Visitor;
    impl<'a> serde::de::Visitor<'a> for Visitor {
        type Value = DateTime<Utc>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.pad("a string")
        }
        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.into())
                .map_err(|e| E::custom(e))
        }
    }

    d.deserialize_str(Visitor)
}
