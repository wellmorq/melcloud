use crate::client::{default_local_session_path, MelcloudClient};
use crate::error::{MelcloudError, Result};
use crate::json_value::{as_i64, as_object, as_str, parse_datetime, str_from_json};
use crate::session::{clear_session_file, load_session_file, write_session_file};
use crate::Session;
use chrono::{Duration, Utc};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::path::PathBuf;

pub(crate) const BASE_URL: &str = "https://app.melcloud.com/Mitsubishi.Wifi.Client";
pub(crate) const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/73.0";

impl MelcloudClient {
    pub async fn login(&mut self) -> Result<Session> {
        let email = self
            .config
            .email
            .clone()
            .ok_or(MelcloudError::MissingCredentials)?;
        let password = self
            .config
            .password
            .clone()
            .ok_or(MelcloudError::MissingCredentials)?;
        let payload = json!({
            "Email": email,
            "Password": password,
            "Language": self.config.language_id,
            "AppVersion": "1.34.13.0",
            "Persist": true,
            "CaptchaChallenge": "",
            "CaptchaResponse": null
        });

        let mut last_error = None;
        let mut value = None;
        for route in ["ClientLogin3", "ClientLogin"] {
            let url = format!("{BASE_URL}/Login/{route}");
            let response = match self.http.post(&url).json(&payload).send().await {
                Ok(response) => response,
                Err(err) => {
                    last_error = Some(format!("{route}: {err}"));
                    continue;
                }
            };
            if !response.status().is_success() {
                last_error = Some(format!("{route}: status {}", response.status()));
                continue;
            }
            value = Some(response.json().await?);
            break;
        }

        let value = value.ok_or_else(|| {
            MelcloudError::Auth(last_error.unwrap_or_else(|| "login request failed".to_string()))
        })?;
        let session = session_from_login_response(value)?;
        self.session = Some(session.clone());
        self.save_session()?;
        Ok(session)
    }

    pub(crate) async fn request_json<T, F>(&mut self, mut make_request: F) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        F: FnMut() -> reqwest::RequestBuilder,
    {
        let mut auth_retried = false;
        let mut transport_retried = false;

        loop {
            self.ensure_session().await?;
            let response = match make_request()
                .header(
                    "X-MitsContextKey",
                    self.session.as_ref().unwrap().context_key.clone(),
                )
                .header("X-Requested-With", "XMLHttpRequest")
                .header("Accept", "application/json, text/javascript, */*; q=0.01")
                .header("Cookie", "policyaccepted=true")
                .send()
                .await
            {
                Ok(response) => response,
                Err(err) if !transport_retried && is_retryable_transport_error(&err) => {
                    transport_retried = true;
                    continue;
                }
                Err(err) => return Err(err.into()),
            };

            let status = response.status();
            if (status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN)
                && self.config.email.is_some()
                && self.config.password.is_some()
                && !auth_retried
            {
                auth_retried = true;
                self.session = None;
                self.clear_session_cache()?;
                continue;
            }

            if !status.is_success() {
                let suffix = if auth_retried { " after re-login" } else { "" };
                return Err(MelcloudError::Rejected(format!(
                    "request failed{suffix}: {status}"
                )));
            }

            return Ok(response.json::<T>().await?);
        }
    }

    pub(crate) async fn request_remote_preset_save_json(
        &mut self,
        payload: Value,
    ) -> Result<Value> {
        let http = self.http.clone();
        self.request_json(move || {
            let url = format!("{BASE_URL}/Device/SetAtaPreset");
            http.post(&url).json(&payload)
        })
        .await
    }

    pub(crate) async fn request_remote_preset_save_form(
        &mut self,
        form_fields: Vec<(String, String)>,
    ) -> Result<Value> {
        let http = self.http.clone();
        self.request_json(move || {
            let url = format!("{BASE_URL}/Device/SetAtaPreset");
            http.post(&url).form(&form_fields)
        })
        .await
    }

    pub(crate) fn clear_session_cache(&self) -> Result<()> {
        clear_session_file(&self.resolve_session_path())
    }

    async fn ensure_session(&mut self) -> Result<()> {
        if self.session.as_ref().is_some_and(Session::is_valid) {
            return Ok(());
        }
        if let Some(session) = load_session_file(self.config.session_file.as_deref())? {
            if session.is_valid() {
                self.session = Some(session);
                return Ok(());
            }
        }
        let session = self.login().await?;
        if !session.is_valid() {
            return Err(MelcloudError::Auth("received invalid session".to_string()));
        }
        Ok(())
    }

    fn save_session(&self) -> Result<()> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| MelcloudError::Protocol("No session available to cache".to_string()))?;
        write_session_file(&self.resolve_session_path(), session)
    }

    fn resolve_session_path(&self) -> PathBuf {
        self.config
            .session_file
            .clone()
            .unwrap_or_else(default_local_session_path)
    }
}

fn session_from_login_response(value: Value) -> Result<Session> {
    let login_status = value.get("LoginStatus").and_then(as_i64).unwrap_or(1);
    if login_status != 0 {
        let msg = value
            .get("ErrorMessage")
            .and_then(as_str)
            .unwrap_or("unknown login error")
            .to_string();
        return Err(MelcloudError::Auth(msg));
    }

    let login_data = value
        .get("LoginData")
        .and_then(as_object)
        .ok_or_else(|| MelcloudError::Protocol("Missing LoginData in response".to_string()))?;
    let context_key = str_from_json(login_data.get("ContextKey"))
        .ok_or_else(|| MelcloudError::Protocol("Missing ContextKey".to_string()))?
        .to_string();
    let duration_minutes = login_data.get("Duration").and_then(as_i64);
    let expiry = login_data
        .get("Expiry")
        .and_then(as_str)
        .and_then(parse_datetime)
        .or_else(|| duration_minutes.map(|minutes| Utc::now() + Duration::minutes(minutes)));

    Ok(Session {
        context_key,
        obtained_at: Utc::now(),
        expiry,
        duration_minutes,
        login_status,
        user_name: str_from_json(value.get("UserName")).map(str::to_string),
    })
}

fn is_retryable_transport_error(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_connect() {
        return true;
    }
    let message = error.to_string().to_ascii_lowercase();
    message.contains("broken pipe")
        || message.contains("connection reset")
        || message.contains("connection closed")
}
