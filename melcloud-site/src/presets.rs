use crate::error::{Result, SiteError};
use crate::file_store::{
    read_backup_text, read_text_with_backup, restore_backup_if_missing, write_with_backup,
};
use crate::models::{ConfigPatchRequest, FixedPresetId, StatusSummary};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LocalPresetFile {
    pub name: String,
    pub description: Option<String>,
    pub state: BTreeMap<String, Value>,
}

pub(crate) fn ensure_fixed_presets(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    for preset_id in FixedPresetId::ALL {
        let path = preset_path(dir, preset_id);
        if restore_backup_if_missing(&path)? {
            continue;
        }
        if path.exists() {
            continue;
        }
        write_preset(&path, &seed_preset(preset_id))?;
    }
    Ok(())
}

pub(crate) fn preset_path(dir: &Path, preset_id: FixedPresetId) -> PathBuf {
    dir.join(format!("{}.yaml", preset_id.as_str()))
}

pub(crate) fn write_preset_from_status(
    dir: &Path,
    preset_id: FixedPresetId,
    status: &StatusSummary,
) -> Result<()> {
    write_preset(
        &preset_path(dir, preset_id),
        &preset_from_status(preset_id, status),
    )
}

pub(crate) fn infer_preset_id(status: &StatusSummary) -> Option<FixedPresetId> {
    match status.operation_mode.as_deref() {
        Some("heat") => Some(FixedPresetId::SiteHeat),
        Some("fan_only") => Some(FixedPresetId::SiteFan),
        Some("cool") => Some(FixedPresetId::SiteCool),
        Some("dry") => Some(FixedPresetId::SiteDry),
        _ => None,
    }
}

fn seed_preset(preset_id: FixedPresetId) -> LocalPresetFile {
    let mut state = BTreeMap::new();
    state.insert("power".to_string(), Value::Bool(true));
    state.insert(
        "mode".to_string(),
        Value::String(preset_id.mode().to_string()),
    );
    LocalPresetFile {
        name: preset_id.as_str().to_string(),
        description: Some(format!("Site preset for {}", preset_id.mode())),
        state,
    }
}

fn preset_from_status(preset_id: FixedPresetId, status: &StatusSummary) -> LocalPresetFile {
    let mut state = BTreeMap::new();
    state.insert("power".to_string(), Value::Bool(status.power));
    if let Some(mode) = status.operation_mode.as_ref() {
        state.insert("mode".to_string(), Value::String(mode.clone()));
    }
    if let Some(value) = status.target_temperature {
        if let Ok(raw) = serde_yaml::to_value(value) {
            state.insert("target_temperature".to_string(), raw);
        }
    }
    if let Some(value) = status.fan_speed.as_ref() {
        state.insert("fan_speed".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = status.vane_horizontal.as_ref() {
        state.insert("vane_horizontal".to_string(), Value::String(value.clone()));
    }
    if let Some(value) = status.vane_vertical.as_ref() {
        state.insert("vane_vertical".to_string(), Value::String(value.clone()));
    }
    LocalPresetFile {
        name: preset_id.as_str().to_string(),
        description: Some("Saved from melcloud-site live device state".to_string()),
        state,
    }
}

fn write_preset(path: &Path, preset: &LocalPresetFile) -> Result<()> {
    let raw = serde_yaml::to_string(preset)?;
    write_with_backup(path, raw.as_bytes())
}

pub(crate) fn preset_summary() -> Vec<(FixedPresetId, String)> {
    FixedPresetId::ALL
        .iter()
        .copied()
        .map(|preset_id| (preset_id, preset_id.icon().to_string()))
        .collect()
}

pub(crate) fn load_preset_config(
    dir: &Path,
    preset_id: FixedPresetId,
) -> Result<ConfigPatchRequest> {
    let path = preset_path(dir, preset_id);
    let raw = read_text_with_backup(&path)?.ok_or_else(|| {
        SiteError::Protocol(format!("site preset is missing: {}", path.display()))
    })?;
    let preset: LocalPresetFile = match serde_yaml::from_str(&raw) {
        Ok(preset) => preset,
        Err(primary_err) => {
            if let Some(backup_raw) = read_backup_text(&path)? {
                if let Ok(preset) = serde_yaml::from_str(&backup_raw) {
                    preset
                } else {
                    return Err(primary_err.into());
                }
            } else {
                return Err(primary_err.into());
            }
        }
    };
    Ok(ConfigPatchRequest {
        power: preset.state.get("power").and_then(value_as_bool),
        mode: preset.state.get("mode").and_then(value_as_string),
        target_temperature: preset
            .state
            .get("target_temperature")
            .and_then(value_as_f64),
        fan_speed: preset.state.get("fan_speed").and_then(value_as_string),
        vane_horizontal: preset
            .state
            .get("vane_horizontal")
            .and_then(value_as_string),
        vane_vertical: preset.state.get("vane_vertical").and_then(value_as_string),
    })
}

pub(crate) fn validate_preset_exists(dir: &Path, preset_id: FixedPresetId) -> Result<()> {
    let path = preset_path(dir, preset_id);
    if path.exists() || crate::file_store::backup_path(&path).exists() {
        Ok(())
    } else {
        Err(SiteError::Protocol(format!(
            "site preset is missing: {}",
            path.display()
        )))
    }
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(raw) => Some(*raw),
        _ => None,
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(raw) => raw.as_f64(),
        Value::String(raw) => raw.parse::<f64>().ok(),
        _ => None,
    }
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => Some(raw.clone()),
        Value::Number(raw) => Some(raw.to_string()),
        _ => None,
    }
}
