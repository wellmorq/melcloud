use crate::error::{MelcloudError, Result};
use crate::parse::{format_allowed_ints, parse_operation_mode, round_to_step};
use crate::state::AtaState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const EFFECTIVE_FLAG_POWER: u32 = 0x01;
const EFFECTIVE_FLAG_MODE: u32 = 0x02;
const EFFECTIVE_FLAG_TEMPERATURE: u32 = 0x04;
const EFFECTIVE_FLAG_FAN_SPEED: u32 = 0x08;
const EFFECTIVE_FLAG_VANE_VERTICAL: u32 = 0x10;
const EFFECTIVE_FLAG_VANE_HORIZONTAL: u32 = 0x100;

pub const EFFECTIVE_FLAG_PRESET: u32 = EFFECTIVE_FLAG_POWER
    | EFFECTIVE_FLAG_MODE
    | EFFECTIVE_FLAG_TEMPERATURE
    | EFFECTIVE_FLAG_FAN_SPEED
    | EFFECTIVE_FLAG_VANE_VERTICAL
    | EFFECTIVE_FLAG_VANE_HORIZONTAL;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AtaPatch {
    pub power: Option<bool>,
    pub operation_mode: Option<String>,
    pub target_temperature: Option<f64>,
    pub fan_speed: Option<i64>,
    pub vane_horizontal: Option<i64>,
    pub vane_vertical: Option<i64>,
}

#[derive(Debug)]
pub struct PatchResult {
    pub payload: Value,
    pub flags: u32,
}

pub type PreparedConfigCommand = PatchResult;
pub type DeviceConfigSnapshot = AtaState;
pub type DeviceConfigPatch = AtaPatch;

impl AtaPatch {
    pub fn is_empty(&self) -> bool {
        self.power.is_none()
            && self.operation_mode.is_none()
            && self.target_temperature.is_none()
            && self.fan_speed.is_none()
            && self.vane_horizontal.is_none()
            && self.vane_vertical.is_none()
    }
}

impl AtaState {
    pub fn prepare_command(&self, patch: &AtaPatch) -> Result<PatchResult> {
        let mut flags = 0u32;
        let mut payload = self.raw().clone();

        if let Some(power) = patch.power {
            if self.prohibit_power() {
                return Err(MelcloudError::InvalidPayload(
                    "power changes are prohibited on this device".to_string(),
                ));
            }
            payload["Power"] = json!(power);
            flags |= EFFECTIVE_FLAG_POWER;
        }

        if let Some(mode) = patch.operation_mode.as_deref() {
            let allowed = self.supported_operation_modes();
            if allowed.is_empty() {
                return Err(MelcloudError::InvalidPayload(
                    "operation mode changes are not available on this device".to_string(),
                ));
            }
            if !allowed.iter().any(|candidate| *candidate == mode) {
                return Err(MelcloudError::InvalidPayload(format!(
                    "unsupported operation mode: {mode}. allowed: {}",
                    allowed.join(", ")
                )));
            }
            let code = parse_operation_mode(mode).ok_or_else(|| {
                MelcloudError::InvalidPayload(format!("unsupported operation mode: {mode}"))
            })?;
            payload["OperationMode"] = json!(code);
            flags |= EFFECTIVE_FLAG_MODE;
        }

        if let Some(temp) = patch.target_temperature {
            if self.prohibit_set_temperature() {
                return Err(MelcloudError::InvalidPayload(
                    "temperature changes are prohibited on this device".to_string(),
                ));
            }
            payload["SetTemperature"] = json!(round_to_step(temp, self.temperature_increment()));
            flags |= EFFECTIVE_FLAG_TEMPERATURE;
        }

        if let Some(fan) = patch.fan_speed {
            let allowed = self.supported_fan_speeds();
            if !allowed.contains(&fan) {
                return Err(MelcloudError::InvalidPayload(format!(
                    "unsupported fan speed: {fan}. allowed: {}",
                    format_allowed_ints(&allowed)
                )));
            }
            payload["SetFanSpeed"] = json!(fan);
            flags |= EFFECTIVE_FLAG_FAN_SPEED;
        }

        if let Some(vane_h) = patch.vane_horizontal {
            let allowed = self.supported_horizontal_vanes();
            if allowed.is_empty() {
                return Err(MelcloudError::InvalidPayload(
                    "horizontal vane controls are not available on this device".to_string(),
                ));
            }
            if !allowed.contains(&vane_h) {
                return Err(MelcloudError::InvalidPayload(format!(
                    "unsupported horizontal vane position: {vane_h}. allowed: {}",
                    format_allowed_ints(&allowed)
                )));
            }
            payload["VaneHorizontal"] = json!(vane_h);
            flags |= EFFECTIVE_FLAG_VANE_HORIZONTAL;
        }

        if let Some(vane_v) = patch.vane_vertical {
            let allowed = self.supported_vertical_vanes();
            if allowed.is_empty() {
                return Err(MelcloudError::InvalidPayload(
                    "vertical vane controls are not available on this device".to_string(),
                ));
            }
            if !allowed.contains(&vane_v) {
                return Err(MelcloudError::InvalidPayload(format!(
                    "unsupported vertical vane position: {vane_v}. allowed: {}",
                    format_allowed_ints(&allowed)
                )));
            }
            payload["VaneVertical"] = json!(vane_v);
            flags |= EFFECTIVE_FLAG_VANE_VERTICAL;
        }

        if flags > 0 {
            payload["EffectiveFlags"] = json!(flags);
            payload["HasPendingCommand"] = json!(true);
        }

        Ok(PatchResult { payload, flags })
    }

    pub fn apply_patch(self, patch: &AtaPatch) -> Result<PatchResult> {
        self.prepare_command(patch)
    }
}
