use crate::json_value::{as_f64, as_i64, as_str};
use crate::models::{AtaStatusSummary, WeatherObservation};
use crate::parse::{
    classic_ata_modes, fan_speed_to_label, horizontal_vane_positions, horizontal_vane_to_label,
    operation_mode_to_cli_value, operation_mode_to_label, temperature_increment_from_override,
    vertical_vane_positions, vertical_vane_to_label,
};
use crate::patch::AtaPatch;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct AtaState {
    raw: Value,
}

impl AtaState {
    pub fn from_json(raw: Value) -> Self {
        Self { raw }
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }

    pub fn power(&self) -> Option<bool> {
        self.raw.get("Power").and_then(Value::as_bool)
    }

    pub fn operation_mode_raw(&self) -> Option<i64> {
        self.raw.get("OperationMode").and_then(as_i64)
    }

    pub fn operation_mode_label(&self) -> &'static str {
        operation_mode_to_label(self.operation_mode_raw().unwrap_or(-1))
    }

    pub fn target_temperature(&self) -> Option<f64> {
        self.raw.get("SetTemperature").and_then(as_f64)
    }

    pub fn room_temperature(&self) -> Option<f64> {
        self.raw.get("RoomTemperature").and_then(as_f64)
    }

    pub fn outdoor_temperature(&self) -> Option<f64> {
        self.raw.get("OutdoorTemperature").and_then(as_f64)
    }

    pub fn fan_speed_raw(&self) -> Option<i64> {
        self.raw.get("SetFanSpeed").and_then(as_i64)
    }

    pub fn fan_speed_label(&self) -> Option<String> {
        self.fan_speed_raw().map(fan_speed_to_label)
    }

    pub fn vane_horizontal_raw(&self) -> Option<i64> {
        self.raw.get("VaneHorizontal").and_then(as_i64)
    }

    pub fn vane_horizontal_label(&self) -> Option<String> {
        self.vane_horizontal_raw().map(horizontal_vane_to_label)
    }

    pub fn vane_vertical_raw(&self) -> Option<i64> {
        self.raw.get("VaneVertical").and_then(as_i64)
    }

    pub fn vane_vertical_label(&self) -> Option<String> {
        self.vane_vertical_raw().map(vertical_vane_to_label)
    }

    pub fn offline(&self) -> Option<bool> {
        self.raw.get("Offline").and_then(Value::as_bool)
    }

    pub fn last_communication(&self) -> Option<String> {
        self.raw
            .get("LastCommunication")
            .and_then(as_str)
            .map(str::to_string)
    }

    pub fn next_communication(&self) -> Option<String> {
        self.raw
            .get("NextCommunication")
            .and_then(as_str)
            .map(str::to_string)
    }

    pub fn temperature_increment(&self) -> f64 {
        self.raw
            .get("TemperatureIncrement")
            .and_then(as_f64)
            .filter(|value| value.is_finite() && *value > 0.0)
            .or_else(|| {
                self.raw
                    .get("TemperatureIncrementOverride")
                    .and_then(as_i64)
                    .and_then(temperature_increment_from_override)
            })
            .unwrap_or(0.5)
    }

    pub fn max_fan_speed(&self) -> Option<i64> {
        self.raw
            .get("NumberOfFanSpeeds")
            .and_then(as_i64)
            .filter(|value| *value > 0)
    }

    pub fn hide_vane_controls(&self) -> bool {
        self.raw
            .get("HideVaneControls")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn hide_dry_mode_control(&self) -> bool {
        self.raw
            .get("HideDryModeControl")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn prohibit_power(&self) -> bool {
        self.raw
            .get("ProhibitPower")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn prohibit_operation_mode(&self) -> bool {
        self.raw
            .get("ProhibitOperationMode")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn prohibit_set_temperature(&self) -> bool {
        self.raw
            .get("ProhibitSetTemperature")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn supported_operation_modes(&self) -> Vec<&'static str> {
        if self.prohibit_operation_mode() {
            return Vec::new();
        }
        classic_ata_modes()
            .iter()
            .filter_map(|(label, _)| {
                if *label == "dry" && self.hide_dry_mode_control() {
                    None
                } else {
                    Some(*label)
                }
            })
            .collect()
    }

    pub fn supported_fan_speeds(&self) -> Vec<i64> {
        let max = self.max_fan_speed().unwrap_or(0);
        (0..=max).collect()
    }

    pub fn supported_horizontal_vanes(&self) -> Vec<i64> {
        if self.hide_vane_controls() {
            return Vec::new();
        }
        horizontal_vane_positions().to_vec()
    }

    pub fn supported_vertical_vanes(&self) -> Vec<i64> {
        if self.hide_vane_controls() {
            return Vec::new();
        }
        vertical_vane_positions().to_vec()
    }

    pub fn summary(&self) -> AtaStatusSummary {
        AtaStatusSummary {
            power: self.power(),
            operation_mode_code: self.operation_mode_raw(),
            operation_mode: self.operation_mode_label().to_string(),
            target_temperature: self.target_temperature(),
            room_temperature: self.room_temperature(),
            outdoor_temperature: self.outdoor_temperature(),
            fan_speed_code: self.fan_speed_raw(),
            fan_speed: self.fan_speed_label(),
            vane_horizontal_code: self.vane_horizontal_raw(),
            vane_horizontal: self.vane_horizontal_label(),
            vane_vertical_code: self.vane_vertical_raw(),
            vane_vertical: self.vane_vertical_label(),
            offline: self.offline(),
            last_communication: self.last_communication(),
            next_communication: self.next_communication(),
        }
    }

    pub fn weather_observations(&self) -> Vec<WeatherObservation> {
        self.raw
            .get("WeatherObservations")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(WeatherObservation::from_value).collect())
            .unwrap_or_default()
    }

    pub fn as_patch(&self) -> AtaPatch {
        AtaPatch {
            power: self.power(),
            operation_mode: self.operation_mode_raw().map(operation_mode_to_cli_value),
            target_temperature: self.target_temperature(),
            fan_speed: self.fan_speed_raw(),
            vane_horizontal: self.vane_horizontal_raw(),
            vane_vertical: self.vane_vertical_raw(),
        }
    }
}

impl WeatherObservation {
    pub(crate) fn from_value(value: &Value) -> Self {
        Self {
            date: value.get("Date").and_then(as_str).map(str::to_string),
            temperature: value.get("Temperature").and_then(as_f64),
            humidity: value.get("Humidity").and_then(as_i64),
            condition_code: value.get("Condition").and_then(as_i64),
            condition_name: value
                .get("ConditionName")
                .and_then(as_str)
                .map(str::to_string),
            icon: value.get("Icon").and_then(as_str).map(str::to_string),
            sunrise: value.get("Sunrise").and_then(as_str).map(str::to_string),
            sunset: value.get("Sunset").and_then(as_str).map(str::to_string),
            weather_type: value.get("WeatherType").and_then(as_i64),
            day: value.get("Day").and_then(as_i64),
        }
    }
}
