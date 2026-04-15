use crate::discovery::flatten_ata_devices;
use crate::error::{MelcloudError, Result};
use crate::models::{BoundDevice, DiscoveredAtaDevice};
use crate::patch::{AtaPatch, PatchResult};
use crate::remote_preset::{
    remote_presets_from_response_value, RemotePreset, RemotePresetSaveRequest,
};
use crate::state::AtaState;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration as StdDuration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelcloudConfig {
    pub email: Option<String>,
    pub password: Option<String>,
    pub session_file: Option<PathBuf>,
    #[serde(default = "default_language_id")]
    pub language_id: i64,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct MelcloudClient {
    pub(crate) config: MelcloudConfig,
    pub(crate) http: Client,
    pub(crate) session: Option<crate::Session>,
}

impl Default for MelcloudConfig {
    fn default() -> Self {
        Self {
            email: None,
            password: None,
            session_file: None,
            language_id: default_language_id(),
            timeout_seconds: default_timeout_seconds(),
        }
    }
}

impl MelcloudClient {
    pub fn new(config: MelcloudConfig) -> Result<Self> {
        let http = Client::builder()
            .user_agent(crate::transport::USER_AGENT)
            .timeout(StdDuration::from_secs(config.timeout_seconds))
            .build()?;
        Ok(Self {
            config,
            http,
            session: None,
        })
    }

    pub fn with_default_session_path(mut self, path: PathBuf) -> Self {
        self.config.session_file = Some(path);
        self
    }

    pub async fn list_device_nodes(&mut self) -> Result<Vec<DiscoveredAtaDevice>> {
        let http = self.http.clone();
        let response: Vec<Value> = self
            .request_json(|| {
                let url = format!("{}/User/ListDevices", crate::transport::BASE_URL);
                http.get(&url)
            })
            .await?;
        let devices = flatten_ata_devices(response);
        if devices.is_empty() {
            return Err(MelcloudError::NoDevices);
        }
        Ok(devices)
    }

    pub async fn list_devices(&mut self) -> Result<Vec<BoundDevice>> {
        Ok(self
            .list_device_nodes()
            .await?
            .into_iter()
            .map(|device| device.device)
            .collect())
    }

    pub async fn discover_device(&mut self) -> Result<BoundDevice> {
        let devices = self.list_device_nodes().await?;
        match devices.len() {
            1 => Ok(devices[0].device.clone()),
            count => Err(MelcloudError::Protocol(format!(
                "expected exactly one ATA device, found {count}"
            ))),
        }
    }

    pub async fn discover_device_node(&mut self) -> Result<DiscoveredAtaDevice> {
        let devices = self.list_device_nodes().await?;
        match devices.len() {
            1 => Ok(devices[0].clone()),
            count => Err(MelcloudError::Protocol(format!(
                "expected exactly one ATA device, found {count}"
            ))),
        }
    }

    pub async fn get_current_config(&mut self, device: &BoundDevice) -> Result<AtaState> {
        self.get_device_status(device).await
    }

    pub async fn get_device_status(&mut self, device: &BoundDevice) -> Result<AtaState> {
        let url = format!(
            "{}/Device/Get?id={}&buildingID={}",
            crate::transport::BASE_URL,
            device.device_id,
            device.building_id
        );
        let http = self.http.clone();
        let state: Value = self.request_json(|| http.get(&url)).await?;
        Ok(AtaState::from_json(state))
    }

    pub fn prepare_config_command(
        &self,
        current: &AtaState,
        patch: &AtaPatch,
    ) -> Result<PatchResult> {
        current.prepare_command(patch)
    }

    pub async fn send_config_command(
        &mut self,
        _device: &BoundDevice,
        prepared: &PatchResult,
    ) -> Result<AtaState> {
        let http = self.http.clone();
        let payload = prepared.payload.clone();
        let response: Value = self
            .request_json(move || {
                let url = format!("{}/Device/SetAta", crate::transport::BASE_URL);
                http.post(&url).json(&payload)
            })
            .await?;
        Ok(AtaState::from_json(response))
    }

    pub async fn apply_config_patch(
        &mut self,
        device: &BoundDevice,
        patch: &AtaPatch,
    ) -> Result<AtaState> {
        let current = self.get_current_config(device).await?;
        let prepared = self.prepare_config_command(&current, patch)?;
        self.send_config_command(device, &prepared).await
    }

    pub async fn apply_patch(&mut self, device: &BoundDevice, patch: AtaPatch) -> Result<AtaState> {
        self.apply_config_patch(device, &patch).await
    }

    pub async fn list_remote_presets(&mut self) -> Result<Vec<RemotePreset>> {
        Ok(self.discover_device_node().await?.presets)
    }

    pub async fn apply_remote_preset(
        &mut self,
        device: &BoundDevice,
        preset: &RemotePreset,
    ) -> Result<AtaState> {
        self.apply_config_patch(device, &preset.as_patch()).await
    }

    pub async fn save_remote_preset(
        &mut self,
        request: &RemotePresetSaveRequest,
    ) -> Result<Vec<RemotePreset>> {
        let attempts = [
            (
                "official-json",
                self.request_remote_preset_save_json(request.to_wire_payload())
                    .await,
            ),
            (
                "official-form",
                self.request_remote_preset_save_form(request.to_form_fields())
                    .await,
            ),
        ];

        let mut failures = Vec::new();
        for (label, result) in attempts {
            match result {
                Ok(response) => return Ok(remote_presets_from_response_value(response)),
                Err(err) => failures.push(format!("{label}={err}")),
            }
        }

        Err(MelcloudError::Rejected(format!(
            "remote preset save failed via all known payload shapes: {}",
            failures.join("; ")
        )))
    }

    pub async fn test_connection(&mut self) -> Result<crate::Session> {
        self.login().await
    }
}

fn default_timeout_seconds() -> u64 {
    30
}

pub(crate) fn default_language_id() -> i64 {
    0
}

pub(crate) fn default_local_session_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("presets")
        .join(crate::session::SESSION_FILE_NAME)
}
