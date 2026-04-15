use crate::file_store::{read_backup_text, read_text_with_backup, write_with_backup};
use crate::models::PresetFile;
use melcloud_core::{
    parse_fan_speed, parse_horizontal_vane, parse_mode_input, parse_vertical_vane, AtaPatch,
    AtaState, MelcloudError,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const PRESET_META_KEYS: &[&str] = &["name", "description", "state"];
const PRESET_STATE_KEYS: &[&str] = &[
    "power",
    "mode",
    "operation_mode",
    "target_temperature",
    "fan_speed",
    "vane_horizontal",
    "vane_vertical",
];

pub(crate) fn list_local_preset_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .ok()
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter_map(|entry| {
                    let path = entry.path();
                    if !is_yaml_file(&path) {
                        return None;
                    }
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

pub(crate) fn local_preset_path(base: &Path, name: &str) -> PathBuf {
    let trimmed = name.trim();
    let file_name = if trimmed.ends_with(".yaml") || trimmed.ends_with(".yml") {
        trimmed.to_string()
    } else {
        format!("{trimmed}.yaml")
    };
    base.join(file_name)
}

pub(crate) fn load_local_preset(dir: &Path, name: &str) -> Result<PresetFile, MelcloudError> {
    let path = local_preset_path(dir, name);
    let content = read_text_with_backup(&path)?.ok_or_else(|| {
        MelcloudError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("preset missing: {}", path.display()),
        ))
    })?;
    let raw = parse_preset_yaml_value(&content, &path).or_else(|primary_err| {
        if let Some(backup_content) = read_backup_text(&path)? {
            if let Ok(raw) = parse_preset_yaml_value(&backup_content, &path) {
                return Ok(raw);
            }
        }
        Err(primary_err)
    })?;
    local_preset_from_yaml_value(name, &path, raw)
}

pub(crate) fn write_local_preset_file(
    path: &Path,
    preset: &PresetFile,
) -> Result<(), MelcloudError> {
    let raw = render_local_preset_yaml(preset)?;
    write_with_backup(path, raw.as_bytes())
}

pub(crate) fn render_local_preset_yaml(preset: &PresetFile) -> Result<String, MelcloudError> {
    serde_yaml::to_string(preset)
        .map_err(|err| MelcloudError::Protocol(format!("failed to render preset: {err}")))
}

pub(crate) fn capture_local_preset_from_state(name: &str, state: &AtaState) -> PresetFile {
    local_preset_from_patch(
        name,
        Some("Captured from live ATA state".to_string()),
        &state.as_patch(),
    )
}

pub(crate) fn local_preset_from_patch(
    name: &str,
    description: Option<String>,
    patch: &AtaPatch,
) -> PresetFile {
    let mut preset = PresetFile::empty(name);
    preset.description = description;
    preset.state = patch_to_preset_state(patch);
    preset
}

pub(crate) fn patch_from_local_preset(preset: &PresetFile) -> Result<AtaPatch, MelcloudError> {
    let mut patch = AtaPatch::default();
    for (key, value) in &preset.state {
        match key.as_str() {
            "power" => patch.power = Some(value_as_bool(value)?),
            "mode" | "operation_mode" => patch.operation_mode = Some(value_as_mode(value)?),
            "target_temperature" => patch.target_temperature = Some(value_as_f64(value)?),
            "fan_speed" => patch.fan_speed = Some(value_as_fan_speed(value)?),
            "vane_horizontal" => patch.vane_horizontal = Some(value_as_horizontal_vane(value)?),
            "vane_vertical" => patch.vane_vertical = Some(value_as_vertical_vane(value)?),
            other => return Err(unsupported_state_key(other)),
        }
    }
    Ok(patch)
}

pub(crate) fn parse_preset_scalar_value(raw: &str) -> Result<serde_yaml::Value, MelcloudError> {
    if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(raw) {
        if matches!(
            value,
            serde_yaml::Value::Mapping(_) | serde_yaml::Value::Sequence(_)
        ) {
            return Err(MelcloudError::Protocol(
                "unsupported complex value type".to_string(),
            ));
        }
        return Ok(value);
    }
    if raw.eq_ignore_ascii_case("true") {
        return Ok(serde_yaml::Value::Bool(true));
    }
    if raw.eq_ignore_ascii_case("false") {
        return Ok(serde_yaml::Value::Bool(false));
    }
    if let Ok(value) = raw.parse::<i64>() {
        return Ok(serde_yaml::Value::Number(value.into()));
    }
    if let Ok(value) = raw.parse::<f64>() {
        if value.is_finite() {
            return serde_yaml::from_str::<serde_yaml::Value>(&format!("{value}"))
                .map_err(|err| MelcloudError::Protocol(format!("invalid numeric value: {err}")));
        }
    }
    Ok(serde_yaml::Value::String(raw.to_string()))
}

pub(crate) fn normalize_preset_state_key(key: &str) -> Result<&'static str, MelcloudError> {
    canonical_preset_state_key(key).ok_or_else(|| unsupported_state_key(key))
}

pub(crate) fn allowed_local_preset_keys() -> String {
    PRESET_META_KEYS
        .iter()
        .chain(PRESET_STATE_KEYS.iter())
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

fn patch_to_preset_state(patch: &AtaPatch) -> BTreeMap<String, serde_yaml::Value> {
    let mut state = BTreeMap::new();
    if let Some(power) = patch.power {
        state.insert("power".to_string(), serde_yaml::Value::Bool(power));
    }
    if let Some(mode) = patch.operation_mode.as_ref() {
        state.insert("mode".to_string(), serde_yaml::Value::String(mode.clone()));
    }
    if let Some(temp) = patch.target_temperature {
        state.insert(
            "target_temperature".to_string(),
            serde_yaml::to_value(temp)
                .unwrap_or_else(|_| serde_yaml::Value::String(temp.to_string())),
        );
    }
    if let Some(fan) = patch.fan_speed {
        state.insert("fan_speed".to_string(), fan_speed_yaml_value(fan));
    }
    if let Some(vane_h) = patch.vane_horizontal {
        state.insert(
            "vane_horizontal".to_string(),
            horizontal_vane_yaml_value(vane_h),
        );
    }
    if let Some(vane_v) = patch.vane_vertical {
        state.insert(
            "vane_vertical".to_string(),
            vertical_vane_yaml_value(vane_v),
        );
    }
    state
}

fn local_preset_from_yaml_value(
    default_name: &str,
    path: &Path,
    raw: serde_yaml::Value,
) -> Result<PresetFile, MelcloudError> {
    let mapping = raw.as_mapping().ok_or_else(|| {
        MelcloudError::Protocol(format!(
            "invalid preset format {}: expected yaml mapping",
            path.display()
        ))
    })?;

    let mut preset = PresetFile::empty(default_name);
    for (key, value) in mapping {
        let key = key.as_str().ok_or_else(|| {
            MelcloudError::Protocol(format!(
                "invalid preset format {}: key must be string",
                path.display()
            ))
        })?;
        match key {
            "name" => {
                if let Some(raw_name) = value.as_str() {
                    preset.name = raw_name.to_string();
                }
            }
            "description" => {
                preset.description = value.as_str().map(str::to_string);
            }
            "state" => insert_nested_state(&mut preset, path, value)?,
            other if normalize_preset_state_key(other).is_ok() => {
                insert_normalized_preset_state_value(&mut preset, other, value.clone())?;
            }
            other => {
                return Err(MelcloudError::Protocol(format!(
                    "unsupported preset key `{other}` in {}. allowed keys: {}",
                    path.display(),
                    allowed_local_preset_keys()
                )));
            }
        }
    }
    Ok(preset)
}

fn parse_preset_yaml_value(content: &str, path: &Path) -> Result<serde_yaml::Value, MelcloudError> {
    serde_yaml::from_str::<serde_yaml::Value>(content).map_err(|err| {
        MelcloudError::Protocol(format!("invalid preset format {}: {}", path.display(), err))
    })
}

fn insert_nested_state(
    preset: &mut PresetFile,
    path: &Path,
    value: &serde_yaml::Value,
) -> Result<(), MelcloudError> {
    let state = value.as_mapping().ok_or_else(|| {
        MelcloudError::Protocol(format!(
            "invalid preset format {}: state must be mapping",
            path.display()
        ))
    })?;
    for (state_key, state_value) in state {
        let state_key = state_key.as_str().ok_or_else(|| {
            MelcloudError::Protocol(format!(
                "invalid preset format {}: state key must be string",
                path.display()
            ))
        })?;
        insert_normalized_preset_state_value(preset, state_key, state_value.clone())?;
    }
    Ok(())
}

fn insert_normalized_preset_state_value(
    preset: &mut PresetFile,
    key: &str,
    value: serde_yaml::Value,
) -> Result<(), MelcloudError> {
    let canonical = normalize_preset_state_key(key)?;
    preset.state.insert(canonical.to_string(), value);
    Ok(())
}

fn is_yaml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "yaml" | "yml"))
        .unwrap_or(false)
}

fn canonical_preset_state_key(key: &str) -> Option<&'static str> {
    match key {
        "power" => Some("power"),
        "mode" | "operation_mode" => Some("mode"),
        "target_temperature" => Some("target_temperature"),
        "fan_speed" => Some("fan_speed"),
        "vane_horizontal" => Some("vane_horizontal"),
        "vane_vertical" => Some("vane_vertical"),
        _ => None,
    }
}

fn unsupported_state_key(key: &str) -> MelcloudError {
    MelcloudError::Protocol(format!(
        "unsupported preset key `{key}`. allowed keys: {}",
        PRESET_STATE_KEYS.join(", ")
    ))
}

fn value_as_bool(value: &serde_yaml::Value) -> Result<bool, MelcloudError> {
    value
        .as_bool()
        .ok_or_else(|| MelcloudError::Protocol("preset key expects bool".to_string()))
}

fn value_as_f64(value: &serde_yaml::Value) -> Result<f64, MelcloudError> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|raw| raw as f64))
        .ok_or_else(|| MelcloudError::Protocol("preset key expects number".to_string()))
}

fn value_as_mode(value: &serde_yaml::Value) -> Result<String, MelcloudError> {
    let mode = if let Some(raw) = value.as_str() {
        raw.to_string()
    } else if let Some(raw) = value.as_i64() {
        raw.to_string()
    } else {
        return Err(MelcloudError::Protocol(
            "mode expects string or integer".to_string(),
        ));
    };
    parse_mode_input(&mode).ok_or_else(|| {
        MelcloudError::Protocol(format!(
            "invalid mode value: {mode}. allowed: off, heat, dry, cool, fan_only, auto"
        ))
    })
}

fn value_as_fan_speed(value: &serde_yaml::Value) -> Result<i64, MelcloudError> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(parse_fan_speed))
        .ok_or_else(|| MelcloudError::Protocol("fan_speed expects integer or alias".to_string()))
}

fn value_as_horizontal_vane(value: &serde_yaml::Value) -> Result<i64, MelcloudError> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(parse_horizontal_vane))
        .ok_or_else(|| {
            MelcloudError::Protocol("vane_horizontal expects integer or alias".to_string())
        })
}

fn value_as_vertical_vane(value: &serde_yaml::Value) -> Result<i64, MelcloudError> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(parse_vertical_vane))
        .ok_or_else(|| {
            MelcloudError::Protocol("vane_vertical expects integer or alias".to_string())
        })
}

fn fan_speed_yaml_value(value: i64) -> serde_yaml::Value {
    if value == 0 {
        serde_yaml::Value::String("auto".to_string())
    } else {
        serde_yaml::to_value(value).unwrap_or_else(|_| serde_yaml::Value::String(value.to_string()))
    }
}

fn horizontal_vane_yaml_value(value: i64) -> serde_yaml::Value {
    match value {
        0 => serde_yaml::Value::String("auto".to_string()),
        8 => serde_yaml::Value::String("split".to_string()),
        12 => serde_yaml::Value::String("swing".to_string()),
        other => serde_yaml::to_value(other)
            .unwrap_or_else(|_| serde_yaml::Value::String(other.to_string())),
    }
}

fn vertical_vane_yaml_value(value: i64) -> serde_yaml::Value {
    match value {
        0 => serde_yaml::Value::String("auto".to_string()),
        7 => serde_yaml::Value::String("swing".to_string()),
        other => serde_yaml::to_value(other)
            .unwrap_or_else(|_| serde_yaml::Value::String(other.to_string())),
    }
}
