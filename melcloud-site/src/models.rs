use crate::config::UiLanguage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum FixedPresetId {
    #[serde(rename = "site-heat")]
    SiteHeat,
    #[serde(rename = "site-fan")]
    SiteFan,
    #[serde(rename = "site-cool")]
    SiteCool,
    #[serde(rename = "site-dry")]
    SiteDry,
}

impl FixedPresetId {
    pub(crate) const ALL: [Self; 4] =
        [Self::SiteHeat, Self::SiteFan, Self::SiteCool, Self::SiteDry];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SiteHeat => "site-heat",
            Self::SiteFan => "site-fan",
            Self::SiteCool => "site-cool",
            Self::SiteDry => "site-dry",
        }
    }

    pub(crate) fn mode(self) -> &'static str {
        match self {
            Self::SiteHeat => "heat",
            Self::SiteFan => "fan_only",
            Self::SiteCool => "cool",
            Self::SiteDry => "dry",
        }
    }

    pub(crate) fn icon(self) -> &'static str {
        match self {
            Self::SiteHeat => "preset_heat",
            Self::SiteFan => "preset_fan",
            Self::SiteCool => "preset_cool",
            Self::SiteDry => "preset_dry",
        }
    }
}

impl FromStr for FixedPresetId {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "site-heat" => Ok(Self::SiteHeat),
            "site-fan" => Ok(Self::SiteFan),
            "site-cool" => Ok(Self::SiteCool),
            "site-dry" => Ok(Self::SiteDry),
            _ => Err(format!("unsupported preset id `{raw}`")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeviceSummary {
    pub name: String,
    pub building_id: i64,
    pub device_id: i64,
    pub device_type: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StatusSummary {
    pub power: bool,
    pub operation_mode: Option<String>,
    pub operation_mode_code: Option<i64>,
    pub room_temperature: Option<f64>,
    pub target_temperature: Option<f64>,
    pub fan_speed: Option<String>,
    pub fan_speed_code: Option<i64>,
    pub vane_horizontal: Option<String>,
    pub vane_horizontal_code: Option<i64>,
    pub vane_vertical: Option<String>,
    pub vane_vertical_code: Option<i64>,
    pub last_communication: Option<String>,
    pub next_communication: Option<String>,
    pub offline: bool,
    pub outdoor_temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CliWeatherObservation {
    pub condition_code: Option<i64>,
    pub condition_name: Option<String>,
    pub date: Option<String>,
    pub day: Option<i64>,
    pub humidity: Option<i64>,
    pub icon: Option<String>,
    pub sunrise: Option<String>,
    pub sunset: Option<String>,
    pub temperature: Option<f64>,
    pub weather_type: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CliStatusResponse {
    pub device: DeviceSummary,
    pub status: StatusSummary,
    pub weather: Vec<CliWeatherObservation>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CliDevicesEntry {
    pub device: DeviceSummary,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SitePresetMeta {
    pub id: FixedPresetId,
    pub icon: String,
    pub config: ConfigPatchRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeviceCapabilities {
    pub fan_speeds: Vec<i64>,
    pub supports_fan_auto: bool,
    pub min_temp_auto: Option<f64>,
    pub max_temp_auto: Option<f64>,
    pub min_temp_cool_dry: Option<f64>,
    pub max_temp_cool_dry: Option<f64>,
    pub min_temp_heat: Option<f64>,
    pub max_temp_heat: Option<f64>,
    pub temperature_step: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WeatherCard {
    pub slot: String,
    pub date: Option<String>,
    pub period_key: String,
    pub icon: String,
    pub temperature: Option<f64>,
    pub placeholder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PageSnapshot {
    pub language: UiLanguage,
    pub commit_debounce_ms: u64,
    pub active_preset_id: Option<FixedPresetId>,
    pub device: DeviceSummary,
    pub live_status: StatusSummary,
    pub capabilities: DeviceCapabilities,
    pub presets: Vec<SitePresetMeta>,
    pub weather_cards: Vec<WeatherCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ConfigPatchRequest {
    pub power: Option<bool>,
    pub mode: Option<String>,
    pub target_temperature: Option<f64>,
    pub fan_speed: Option<String>,
    pub vane_horizontal: Option<String>,
    pub vane_vertical: Option<String>,
}
