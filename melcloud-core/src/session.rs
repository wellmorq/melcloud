use crate::error::{MelcloudError, Result};
use crate::json_value::parse_datetime;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

pub(crate) const SESSION_FILE_NAME: &str = "session.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub context_key: String,
    pub obtained_at: DateTime<Utc>,
    pub expiry: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i64>,
    pub login_status: i64,
    pub user_name: Option<String>,
}

impl Session {
    pub fn is_valid(&self) -> bool {
        if let Some(expiry) = self.expiry {
            return Utc::now() < expiry - Duration::seconds(30);
        }
        if let Some(minutes) = self.duration_minutes {
            let ttl = Duration::minutes(minutes);
            return Utc::now() < self.obtained_at + ttl - Duration::seconds(30);
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSession {
    context_key: String,
    obtained_at: String,
    expiry: Option<String>,
    duration_minutes: Option<i64>,
    login_status: i64,
    user_name: Option<String>,
}

impl From<Session> for CachedSession {
    fn from(value: Session) -> Self {
        Self {
            context_key: value.context_key,
            obtained_at: value.obtained_at.to_rfc3339(),
            expiry: value.expiry.map(|value| value.to_rfc3339()),
            duration_minutes: value.duration_minutes,
            login_status: value.login_status,
            user_name: value.user_name,
        }
    }
}

impl TryFrom<CachedSession> for Session {
    type Error = MelcloudError;

    fn try_from(value: CachedSession) -> Result<Self> {
        let obtained_at = parse_datetime(&value.obtained_at).ok_or_else(|| {
            MelcloudError::Protocol("cached session has invalid obtained_at".to_string())
        })?;
        Ok(Self {
            context_key: value.context_key,
            obtained_at,
            expiry: value.expiry.as_deref().and_then(parse_datetime),
            duration_minutes: value.duration_minutes,
            login_status: value.login_status,
            user_name: value.user_name,
        })
    }
}

pub(crate) fn load_session_file(path: Option<&Path>) -> Result<Option<Session>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }

    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return Ok(None),
    };
    let cache: CachedSession = match serde_json::from_str(&raw) {
        Ok(cache) => cache,
        Err(_) => {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
    };
    let session: Session = match cache.try_into() {
        Ok(session) => session,
        Err(_) => {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
    };
    Ok(Some(session))
}

pub(crate) fn write_session_file(path: &Path, session: &Session) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(&CachedSession::from(session.clone()))?;
    fs::write(path, raw)?;
    Ok(())
}

pub(crate) fn clear_session_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

impl Display for Session {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let expiry = self
            .expiry
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string());
        let preview_len = self.context_key.len().min(8);
        write!(
            f,
            "context_key={}; expiry={}",
            &self.context_key[..preview_len],
            expiry
        )
    }
}
