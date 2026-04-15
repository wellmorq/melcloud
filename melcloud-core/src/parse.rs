const HORIZONTAL_VANE_POSITIONS: &[i64] = &[0, 1, 2, 3, 4, 5, 8, 12];
const VERTICAL_VANE_POSITIONS: &[i64] = &[0, 1, 2, 3, 4, 5, 7];
const CLASSIC_ATA_MODES: &[(&str, i64)] = &[
    ("off", 0),
    ("heat", 1),
    ("dry", 2),
    ("cool", 3),
    ("fan_only", 7),
    ("auto", 8),
];

pub fn parse_mode_input(input: &str) -> Option<String> {
    let normalized = input.trim().to_lowercase();
    parse_operation_mode(&normalized)?;
    Some(normalized)
}

pub fn parse_fan_speed(input: &str) -> Option<i64> {
    let value = input.trim().to_lowercase();
    match value.as_str() {
        "auto" => Some(0),
        _ => value
            .parse::<i64>()
            .ok()
            .filter(|candidate| *candidate >= 0),
    }
}

pub fn parse_horizontal_vane(input: &str) -> Option<i64> {
    let value = input.trim().to_lowercase();
    match value.as_str() {
        "auto" => Some(0),
        "split" => Some(8),
        "swing" => Some(12),
        _ => value
            .parse::<i64>()
            .ok()
            .filter(|candidate| *candidate >= 0),
    }
}

pub fn parse_vertical_vane(input: &str) -> Option<i64> {
    let value = input.trim().to_lowercase();
    match value.as_str() {
        "auto" => Some(0),
        "swing" => Some(7),
        _ => value
            .parse::<i64>()
            .ok()
            .filter(|candidate| *candidate >= 0),
    }
}

pub(crate) fn classic_ata_modes() -> &'static [(&'static str, i64)] {
    CLASSIC_ATA_MODES
}

pub(crate) fn horizontal_vane_positions() -> &'static [i64] {
    HORIZONTAL_VANE_POSITIONS
}

pub(crate) fn vertical_vane_positions() -> &'static [i64] {
    VERTICAL_VANE_POSITIONS
}

pub(crate) fn parse_operation_mode(mode: &str) -> Option<i64> {
    match mode.to_lowercase().as_str() {
        "heat" => Some(1),
        "dry" => Some(2),
        "cool" => Some(3),
        "fan" | "fan_only" | "fanonly" => Some(7),
        "auto" => Some(8),
        "off" | "eco" => Some(0),
        "0" => Some(0),
        "1" => Some(1),
        "2" => Some(2),
        "3" => Some(3),
        "7" => Some(7),
        "8" => Some(8),
        _ => None,
    }
}

pub(crate) fn operation_mode_to_label(mode: i64) -> &'static str {
    match mode {
        0 => "off",
        1 => "heat",
        2 => "dry",
        3 => "cool",
        5 | 7 => "fan_only",
        8 => "auto",
        _ => "other",
    }
}

pub(crate) fn operation_mode_to_cli_value(mode: i64) -> String {
    operation_mode_to_label(mode).to_string()
}

pub(crate) fn fan_speed_to_label(speed: i64) -> String {
    if speed == 0 {
        "auto".to_string()
    } else {
        speed.to_string()
    }
}

pub(crate) fn horizontal_vane_to_label(position: i64) -> String {
    match position {
        0 => "auto".to_string(),
        8 => "split".to_string(),
        12 => "swing".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn vertical_vane_to_label(position: i64) -> String {
    match position {
        0 => "auto".to_string(),
        7 => "swing".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn format_allowed_ints(values: &[i64]) -> String {
    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn temperature_increment_from_override(value: i64) -> Option<f64> {
    match value {
        1 => Some(1.0),
        2 => Some(0.5),
        _ => None,
    }
}

pub(crate) fn round_to_step(value: f64, step: f64) -> f64 {
    if !step.is_finite() || step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}
