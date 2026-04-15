use crate::models::PresetFile;
use crate::preview::preview_changes;
use melcloud_core::{AtaPatch, AtaState, BoundDevice, DiscoveredAtaDevice, PatchResult};
use serde_json::json;

pub(crate) fn devices_map_json(devices: &[DiscoveredAtaDevice]) -> Vec<serde_json::Value> {
    devices
        .iter()
        .map(|device| {
            json!({
                "device": device_json(&device.device),
                "remote_preset_count": device.presets.len(),
                "remote_presets": &device.presets,
                "raw": &device.raw,
            })
        })
        .collect()
}

pub(crate) fn status_to_json(device: &BoundDevice, state: &AtaState) -> serde_json::Value {
    json!({
        "device": device_json(device),
        "status": state.summary(),
        "weather": state.weather_observations(),
        "raw": state.raw(),
    })
}

pub(crate) fn weather_to_json(device: &BoundDevice, state: &AtaState) -> serde_json::Value {
    json!({
        "device": device_json(device),
        "weather": state.weather_observations(),
        "raw": state.raw().get("WeatherObservations").cloned().unwrap_or_else(|| json!([])),
    })
}

pub(crate) fn device_json(device: &BoundDevice) -> serde_json::Value {
    json!({
        "name": device.name,
        "device_id": device.device_id,
        "building_id": device.building_id,
        "device_type": 0,
    })
}

pub(crate) fn preset_preview_to_json(
    device: &BoundDevice,
    preset: &PresetFile,
    patch: &AtaPatch,
    current: &AtaState,
    preview: &PatchResult,
) -> serde_json::Value {
    let preview_state = AtaState::from_json(preview.payload.clone());
    json!({
        "device": device_json(device),
        "preset": preset,
        "current_status": current.summary(),
        "requested_patch": patch_to_json(patch),
        "changes": preview_changes(current, &preview_state).into_iter().map(|change| json!({
            "field": change.field,
            "before": change.before,
            "after": change.after,
        })).collect::<Vec<_>>(),
        "effective_flags": {
            "decimal": preview.flags,
            "hex": format!("0x{:x}", preview.flags),
        },
        "payload": preview.payload,
    })
}

pub(crate) fn config_preview_to_json(
    device: &BoundDevice,
    patch: &AtaPatch,
    current: &AtaState,
    preview: &PatchResult,
) -> serde_json::Value {
    let preview_state = AtaState::from_json(preview.payload.clone());
    json!({
        "device": device_json(device),
        "current_status": current.summary(),
        "requested_patch": patch_to_json(patch),
        "changes": preview_changes(current, &preview_state).into_iter().map(|change| json!({
            "field": change.field,
            "before": change.before,
            "after": change.after,
        })).collect::<Vec<_>>(),
        "effective_flags": {
            "decimal": preview.flags,
            "hex": format!("0x{:x}", preview.flags),
        },
        "payload": preview.payload,
    })
}

pub(crate) fn patch_to_json(patch: &AtaPatch) -> serde_json::Value {
    let mut payload = serde_json::Map::new();
    if let Some(power) = patch.power {
        payload.insert("power".to_string(), json!(power));
    }
    if let Some(mode) = patch.operation_mode.as_ref() {
        payload.insert("mode".to_string(), json!(mode));
    }
    if let Some(value) = patch.target_temperature {
        payload.insert("target_temperature".to_string(), json!(value));
    }
    if let Some(value) = patch.fan_speed {
        payload.insert("fan_speed".to_string(), json!(value));
    }
    if let Some(value) = patch.vane_horizontal {
        payload.insert("vane_horizontal".to_string(), json!(value));
    }
    if let Some(value) = patch.vane_vertical {
        payload.insert("vane_vertical".to_string(), json!(value));
    }
    serde_json::Value::Object(payload)
}
