use crate::models::PresetFile;
use crate::preview::{bool_to_on_off, format_optional_f64, preview_changes};
use crate::remote_presets::remote_preset_label;
use melcloud_core::{AtaState, BoundDevice, RemotePreset};

pub(crate) fn format_preset_preview_block(
    device: &BoundDevice,
    preset: &PresetFile,
    current: &AtaState,
    preview_state: &AtaState,
    flags: u32,
) -> String {
    let changes = preview_changes(current, preview_state);
    let mut out = String::new();
    out.push_str(&format!(
        "preset preview: {} -> {} ({})\n",
        preset.name, device.name, device.device_id
    ));
    if let Some(description) = preset.description.as_deref() {
        out.push_str(&format!("description: {description}\n"));
    }
    out.push_str(&format!("effective flags: {} (0x{:x})\n", flags, flags));
    if changes.is_empty() {
        out.push_str("changes: none (preset already matches live state)");
        return out;
    }
    out.push_str("changes:\n");
    for change in changes {
        out.push_str(&format!(
            "{}: {} -> {}\n",
            change.field, change.before, change.after
        ));
    }
    out.trim_end().to_string()
}

pub(crate) fn format_config_preview_block(
    device: &BoundDevice,
    title: &str,
    current: &AtaState,
    preview_state: &AtaState,
    flags: u32,
) -> String {
    let changes = preview_changes(current, preview_state);
    let mut out = String::new();
    out.push_str(&format!(
        "{title}: {} ({})\n",
        device.name, device.device_id
    ));
    out.push_str(&format!("effective flags: {} (0x{:x})\n", flags, flags));
    if changes.is_empty() {
        out.push_str("changes: none (request already matches live state)");
        return out;
    }
    out.push_str("changes:\n");
    for change in changes {
        out.push_str(&format!(
            "{}: {} -> {}\n",
            change.field, change.before, change.after
        ));
    }
    out.trim_end().to_string()
}

pub(crate) fn format_status_block(prefix: &str, state: &AtaState, device: &BoundDevice) -> String {
    let summary = state.summary();
    let mut out = String::new();
    out.push_str(&format!(
        "{prefix}: {} ({})\n",
        device.name, device.device_id
    ));
    out.push_str(&format!("power: {}\n", bool_to_on_off(summary.power)));
    out.push_str(&format!(
        "mode: {} ({})\n",
        summary.operation_mode,
        summary
            .operation_mode_code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "target temp: {}\n",
        format_optional_f64(summary.target_temperature)
    ));
    out.push_str(&format!(
        "room temp: {}\n",
        format_optional_f64(summary.room_temperature)
    ));
    out.push_str(&format!(
        "outdoor temp: {}\n",
        format_optional_f64(summary.outdoor_temperature)
    ));
    out.push_str(&format!(
        "fan speed: {}\n",
        summary.fan_speed.unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "vane horizontal: {}\n",
        summary.vane_horizontal.unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "vane vertical: {}\n",
        summary.vane_vertical.unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "offline: {}\n",
        summary
            .offline
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "last communication: {}\n",
        summary
            .last_communication
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!(
        "next communication: {}",
        summary
            .next_communication
            .unwrap_or_else(|| "-".to_string())
    ));
    out
}

pub(crate) fn format_weather_block(state: &AtaState, device: &BoundDevice) -> String {
    let weather = state.weather_observations();
    if weather.is_empty() {
        return format!(
            "weather: {} ({})\nno weather observations",
            device.name, device.device_id
        );
    }
    let mut out = String::new();
    out.push_str(&format!(
        "weather: {} ({})\n",
        device.name, device.device_id
    ));
    for observation in weather {
        let date = observation.date.unwrap_or_else(|| "-".to_string());
        let condition = observation
            .condition_name
            .unwrap_or_else(|| "unknown".to_string());
        let temperature = observation
            .temperature
            .map(|value| format!("{value:.1}"))
            .unwrap_or_else(|| "-".to_string());
        let humidity = observation
            .humidity
            .map(|value| format!("{value}%"))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "{date} | {temperature} C | {condition} | humidity {humidity}\n"
        ));
    }
    out.trim_end().to_string()
}

pub(crate) fn format_remote_preset_list_block(
    device: &BoundDevice,
    presets: &[RemotePreset],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "remote presets: {} ({})\n",
        device.name, device.device_id
    ));
    for preset in presets {
        out.push_str(&format!(
            "{} | mode={} | temp={} | fan={}\n",
            remote_preset_label(preset),
            preset.state.operation_mode.as_deref().unwrap_or("-"),
            format_optional_f64(preset.state.target_temperature),
            preset.state.fan_speed.as_deref().unwrap_or("-")
        ));
    }
    out.trim_end().to_string()
}

pub(crate) fn format_remote_preset_block(device: &BoundDevice, preset: &RemotePreset) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "remote preset: {} -> {} ({})\n",
        remote_preset_label(preset),
        device.name,
        device.device_id
    ));
    out.push_str(&format!(
        "id: {}\n",
        preset
            .id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    out.push_str(&format!("power: {}\n", bool_to_on_off(preset.state.power)));
    out.push_str(&format!(
        "mode: {}\n",
        preset.state.operation_mode.as_deref().unwrap_or("-")
    ));
    out.push_str(&format!(
        "target temp: {}\n",
        format_optional_f64(preset.state.target_temperature)
    ));
    out.push_str(&format!(
        "fan speed: {}\n",
        preset.state.fan_speed.as_deref().unwrap_or("-")
    ));
    out.push_str(&format!(
        "vane horizontal: {}\n",
        preset.state.vane_horizontal.as_deref().unwrap_or("-")
    ));
    out.push_str(&format!(
        "vane vertical: {}",
        preset.state.vane_vertical.as_deref().unwrap_or("-")
    ));
    out
}
