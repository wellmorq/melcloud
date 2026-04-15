use crate::remote_preset::RemotePreset;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundDevice {
    pub name: String,
    pub building_id: i64,
    pub device_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredAtaDevice {
    pub device: BoundDevice,
    pub presets: Vec<RemotePreset>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AtaStatusSummary {
    pub power: Option<bool>,
    pub operation_mode_code: Option<i64>,
    pub operation_mode: String,
    pub target_temperature: Option<f64>,
    pub room_temperature: Option<f64>,
    pub outdoor_temperature: Option<f64>,
    pub fan_speed_code: Option<i64>,
    pub fan_speed: Option<String>,
    pub vane_horizontal_code: Option<i64>,
    pub vane_horizontal: Option<String>,
    pub vane_vertical_code: Option<i64>,
    pub vane_vertical: Option<String>,
    pub offline: Option<bool>,
    pub last_communication: Option<String>,
    pub next_communication: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WeatherObservation {
    pub date: Option<String>,
    pub temperature: Option<f64>,
    pub humidity: Option<i64>,
    pub condition_code: Option<i64>,
    pub condition_name: Option<String>,
    pub icon: Option<String>,
    pub sunrise: Option<String>,
    pub sunset: Option<String>,
    pub weather_type: Option<i64>,
    pub day: Option<i64>,
}
