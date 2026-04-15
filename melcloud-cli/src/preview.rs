use melcloud_core::{AtaState, RemotePreset};

#[derive(Debug, Clone)]
pub(crate) struct PreviewChange {
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

pub(crate) fn preview_changes(before: &AtaState, after: &AtaState) -> Vec<PreviewChange> {
    let before_summary = before.summary();
    let after_summary = after.summary();
    let mut changes = Vec::new();

    push_change(
        &mut changes,
        "power",
        bool_to_on_off(before_summary.power).to_string(),
        bool_to_on_off(after_summary.power).to_string(),
    );
    push_change(
        &mut changes,
        "mode",
        before_summary.operation_mode,
        after_summary.operation_mode,
    );
    push_change(
        &mut changes,
        "target_temperature",
        format_optional_f64(before_summary.target_temperature),
        format_optional_f64(after_summary.target_temperature),
    );
    push_change(
        &mut changes,
        "fan_speed",
        before_summary.fan_speed.unwrap_or_else(|| "-".to_string()),
        after_summary.fan_speed.unwrap_or_else(|| "-".to_string()),
    );
    push_change(
        &mut changes,
        "vane_horizontal",
        before_summary
            .vane_horizontal
            .unwrap_or_else(|| "-".to_string()),
        after_summary
            .vane_horizontal
            .unwrap_or_else(|| "-".to_string()),
    );
    push_change(
        &mut changes,
        "vane_vertical",
        before_summary
            .vane_vertical
            .unwrap_or_else(|| "-".to_string()),
        after_summary
            .vane_vertical
            .unwrap_or_else(|| "-".to_string()),
    );
    changes
}

pub(crate) fn bool_to_on_off(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "on",
        Some(false) => "off",
        None => "-",
    }
}

pub(crate) fn format_optional_f64(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "-".to_string())
}

pub(crate) fn compact_remote_preset_signature(preset: &RemotePreset) -> String {
    format!(
        "name={}, power={:?}, mode={:?}, temp={:?}, fan={:?}, vane_h={:?}, vane_v={:?}",
        preset.name,
        preset.state.power,
        preset.state.operation_mode,
        preset.state.target_temperature,
        preset.state.fan_speed_code,
        preset.state.vane_horizontal_code,
        preset.state.vane_vertical_code
    )
}

fn push_change(
    changes: &mut Vec<PreviewChange>,
    field: &'static str,
    before: String,
    after: String,
) {
    if before != after {
        changes.push(PreviewChange {
            field,
            before,
            after,
        });
    }
}
