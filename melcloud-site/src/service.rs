use crate::cli::CliRunner;
use crate::config::SiteConfig;
use crate::error::{Result, SiteError};
use crate::models::{
    CliDevicesEntry, CliStatusResponse, ConfigPatchRequest, DeviceCapabilities, FixedPresetId,
    PageSnapshot, SitePresetMeta, StatusSummary,
};
use crate::presets::{
    ensure_fixed_presets, infer_preset_id, load_preset_config, preset_summary,
    validate_preset_exists, write_preset_from_status,
};
use crate::site_state::{load_site_state, write_site_state, SiteStateFile};
use crate::weather::build_weather_cards;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct CachedDiscovery {
    devices: Vec<CliDevicesEntry>,
    capabilities: DeviceCapabilities,
}

pub(crate) struct SiteService {
    config: SiteConfig,
    runner: Arc<dyn CliRunner>,
    op_lock: Mutex<()>,
    discovery_cache: Mutex<Option<CachedDiscovery>>,
}

impl SiteService {
    pub(crate) fn new(config: SiteConfig, runner: Arc<dyn CliRunner>) -> Result<Self> {
        ensure_fixed_presets(&config.preset_dir)?;
        Ok(Self {
            config,
            runner,
            op_lock: Mutex::new(()),
            discovery_cache: Mutex::new(None),
        })
    }

    pub(crate) fn config(&self) -> &SiteConfig {
        &self.config
    }

    pub(crate) async fn snapshot(&self, refresh_discovery: bool) -> Result<PageSnapshot> {
        let _guard = self.op_lock.lock().await;
        let status = self.runner.status_json().await?;
        let discovery = self.load_discovery(refresh_discovery).await?;
        self.build_snapshot(status, discovery, None).await
    }

    pub(crate) async fn refresh(&self) -> Result<PageSnapshot> {
        self.snapshot(true).await
    }

    pub(crate) async fn apply_preset(&self, preset_id: FixedPresetId) -> Result<PageSnapshot> {
        let _guard = self.op_lock.lock().await;
        validate_preset_exists(&self.config.preset_dir, preset_id)?;
        let expected = load_preset_config(&self.config.preset_dir, preset_id)?;
        let _sent = self.runner.set_config(&expected).await?;
        let status = self.runner.status_json().await?;
        verify_status_matches_patch(&status.status, &expected, "preset apply")?;
        write_preset_from_status(&self.config.preset_dir, preset_id, &status.status)?;
        write_site_state(
            &self.config.site_state_path,
            &SiteStateFile {
                active_preset_id: Some(preset_id),
            },
        )?;
        let discovery = self.load_discovery(false).await?;
        self.build_snapshot(status, discovery, Some(preset_id))
            .await
    }

    pub(crate) async fn patch_active_preset(
        &self,
        preset_id: FixedPresetId,
        patch: &ConfigPatchRequest,
    ) -> Result<PageSnapshot> {
        let _guard = self.op_lock.lock().await;
        validate_patch(patch)?;
        let _sent = self.runner.set_config(patch).await?;
        let status = self.runner.status_json().await?;
        verify_status_matches_patch(&status.status, patch, "config write")?;
        write_preset_from_status(&self.config.preset_dir, preset_id, &status.status)?;
        write_site_state(
            &self.config.site_state_path,
            &SiteStateFile {
                active_preset_id: Some(preset_id),
            },
        )?;
        let discovery = self.load_discovery(false).await?;
        self.build_snapshot(status, discovery, Some(preset_id))
            .await
    }

    async fn load_discovery(&self, refresh: bool) -> Result<CachedDiscovery> {
        if !refresh {
            if let Some(cached) = self.discovery_cache.lock().await.clone() {
                return Ok(cached);
            }
        }
        let devices = self.runner.devices_list_json().await?;
        let capabilities = capabilities_from_devices(&devices)?;
        let cached = CachedDiscovery {
            devices,
            capabilities,
        };
        *self.discovery_cache.lock().await = Some(cached.clone());
        Ok(cached)
    }

    async fn build_snapshot(
        &self,
        status: CliStatusResponse,
        discovery: CachedDiscovery,
        forced_active_preset: Option<FixedPresetId>,
    ) -> Result<PageSnapshot> {
        let active_preset_id = resolve_active_preset(
            &self.config.site_state_path,
            &self.config.preset_dir,
            &status,
            forced_active_preset,
        )?;
        let presets = preset_summary()
            .into_iter()
            .map(|(id, icon)| {
                Ok(SitePresetMeta {
                    id,
                    icon,
                    config: load_preset_config(&self.config.preset_dir, id)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(PageSnapshot {
            language: self.config.ui_language,
            commit_debounce_ms: self.config.commit_debounce_ms,
            active_preset_id,
            device: status.device.clone(),
            live_status: status.status.clone(),
            capabilities: discovery.capabilities,
            presets,
            weather_cards: build_weather_cards(
                &status,
                &discovery.devices,
                &self.config.weather_icon_cache_dir,
                self.config.weather_icon_timeout_ms,
            )
            .await?,
        })
    }
}

fn resolve_active_preset(
    state_path: &std::path::Path,
    preset_dir: &std::path::Path,
    status: &CliStatusResponse,
    forced: Option<FixedPresetId>,
) -> Result<Option<FixedPresetId>> {
    if forced.is_some() {
        return Ok(forced);
    }
    let current = load_site_state(state_path)?;
    if let Some(preset_id) = current.active_preset_id {
        if preset_matches_live_mode(preset_dir, preset_id, &status.status)? {
            return Ok(Some(preset_id));
        }
    }
    Ok(infer_preset_id(&status.status))
}

fn preset_matches_live_mode(
    preset_dir: &std::path::Path,
    preset_id: FixedPresetId,
    status: &StatusSummary,
) -> Result<bool> {
    let live_mode = match status.operation_mode.as_deref() {
        Some(mode) => mode,
        None => return Ok(true),
    };
    let preset = load_preset_config(preset_dir, preset_id)?;
    Ok(preset.mode.as_deref().unwrap_or(preset_id.mode()) == live_mode)
}

fn validate_patch(patch: &ConfigPatchRequest) -> Result<()> {
    if patch.power.is_none()
        && patch.mode.is_none()
        && patch.target_temperature.is_none()
        && patch.fan_speed.is_none()
        && patch.vane_horizontal.is_none()
        && patch.vane_vertical.is_none()
    {
        return Err(SiteError::Protocol(
            "config patch request has no settable fields".to_string(),
        ));
    }
    Ok(())
}

fn verify_status_matches_patch(
    status: &StatusSummary,
    expected: &ConfigPatchRequest,
    context: &str,
) -> Result<()> {
    let mut mismatches = Vec::new();
    compare_bool(&mut mismatches, "power", expected.power, status.power);
    compare_text(
        &mut mismatches,
        "mode",
        expected.mode.as_deref(),
        status.operation_mode.as_deref(),
    );
    compare_temperature(
        &mut mismatches,
        "target_temperature",
        expected.target_temperature,
        status.target_temperature,
    );
    compare_text(
        &mut mismatches,
        "fan_speed",
        expected.fan_speed.as_deref(),
        status.fan_speed.as_deref(),
    );
    compare_text(
        &mut mismatches,
        "vane_horizontal",
        expected.vane_horizontal.as_deref(),
        status.vane_horizontal.as_deref(),
    );
    compare_text(
        &mut mismatches,
        "vane_vertical",
        expected.vane_vertical.as_deref(),
        status.vane_vertical.as_deref(),
    );
    if mismatches.is_empty() {
        return Ok(());
    }
    Err(SiteError::WriteNotConfirmed(format!(
        "{context} was sent but read-back did not confirm it: {}",
        mismatches.join(", ")
    )))
}

fn compare_bool(mismatches: &mut Vec<String>, field: &str, expected: Option<bool>, actual: bool) {
    if let Some(expected) = expected {
        if expected != actual {
            mismatches.push(format!("{field} expected {expected}, got {actual}"));
        }
    }
}

fn compare_text(
    mismatches: &mut Vec<String>,
    field: &str,
    expected: Option<&str>,
    actual: Option<&str>,
) {
    if let Some(expected) = expected {
        if Some(expected) != actual {
            mismatches.push(format!(
                "{field} expected {expected}, got {}",
                actual.unwrap_or("<none>")
            ));
        }
    }
}

fn compare_temperature(
    mismatches: &mut Vec<String>,
    field: &str,
    expected: Option<f64>,
    actual: Option<f64>,
) {
    if let Some(expected) = expected {
        let confirmed = actual
            .map(|actual| (expected - actual).abs() <= 0.05)
            .unwrap_or(false);
        if !confirmed {
            mismatches.push(format!(
                "{field} expected {expected:.1}, got {}",
                actual
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "<none>".to_string())
            ));
        }
    }
}

pub(crate) fn capabilities_from_devices(devices: &[CliDevicesEntry]) -> Result<DeviceCapabilities> {
    let device = devices
        .first()
        .and_then(|entry| entry.raw.get("Device"))
        .ok_or_else(|| {
            SiteError::Protocol("device discovery payload is missing Device".to_string())
        })?;
    let speed_count = value_i64(device, "NumberOfFanSpeeds").unwrap_or(5);
    let step = value_f64(device, "TemperatureIncrement").unwrap_or(0.5);
    Ok(DeviceCapabilities {
        fan_speeds: (1..=speed_count.max(1)).collect(),
        supports_fan_auto: value_bool(device, "HasAutomaticFanSpeed").unwrap_or(true),
        min_temp_auto: value_f64(device, "MinTempAutomatic"),
        max_temp_auto: value_f64(device, "MaxTempAutomatic"),
        min_temp_cool_dry: value_f64(device, "MinTempCoolDry"),
        max_temp_cool_dry: value_f64(device, "MaxTempCoolDry"),
        min_temp_heat: value_f64(device, "MinTempHeat"),
        max_temp_heat: value_f64(device, "MaxTempHeat"),
        temperature_step: step,
    })
}

fn value_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn value_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn value_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}
