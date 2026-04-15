use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

pub(crate) fn str_from_json(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}

pub(crate) fn as_str(value: &Value) -> Option<&str> {
    value.as_str()
}

pub(crate) fn as_object(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value.as_object()
}

pub(crate) fn as_i64(value: &Value) -> Option<i64> {
    value.as_i64()
}

pub(crate) fn as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|raw| raw as f64))
}

pub(crate) fn parse_datetime(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.3f") {
        return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    None
}
