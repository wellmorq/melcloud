use crate::error::{MelcloudError, Result};
use crate::json_value::{as_f64, as_i64, as_str};
use crate::parse::{
    fan_speed_to_label, horizontal_vane_to_label, operation_mode_to_cli_value,
    vertical_vane_to_label,
};
use crate::patch::AtaPatch;
use crate::state::AtaState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemotePreset {
    pub name: String,
    pub number: Option<i64>,
    pub id: Option<i64>,
    pub state: RemotePresetState,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemotePresetState {
    pub power: Option<bool>,
    pub operation_mode_code: Option<i64>,
    pub operation_mode: Option<String>,
    pub target_temperature: Option<f64>,
    pub fan_speed_code: Option<i64>,
    pub fan_speed: Option<String>,
    pub vane_horizontal_code: Option<i64>,
    pub vane_horizontal: Option<String>,
    pub vane_vertical_code: Option<i64>,
    pub vane_vertical: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemotePresetSaveRequest {
    pub device_id: i64,
    pub number: i64,
    pub number_as_string: String,
    #[serde(rename = "NumberDescription")]
    pub number_description: String,
    #[serde(rename = "PresetRequest")]
    pub preset_request: RemotePresetRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RemotePresetRequest {
    #[serde(rename = "Power")]
    pub power: bool,
    #[serde(rename = "SetTemperature")]
    pub set_temperature: f64,
    #[serde(rename = "OperationMode")]
    pub operation_mode: i64,
    #[serde(rename = "VaneHorizontal")]
    pub vane_horizontal: i64,
    #[serde(rename = "VaneVertical")]
    pub vane_vertical: i64,
    #[serde(rename = "FanSpeed")]
    pub fan_speed: i64,
}

impl RemotePreset {
    pub fn as_patch(&self) -> AtaPatch {
        AtaPatch {
            power: self.state.power,
            operation_mode: self.state.operation_mode.clone(),
            target_temperature: self.state.target_temperature,
            fan_speed: self.state.fan_speed_code,
            vane_horizontal: self.state.vane_horizontal_code,
            vane_vertical: self.state.vane_vertical_code,
        }
    }

    pub fn client(&self) -> Option<i64> {
        self.raw.get("Client").and_then(as_i64)
    }

    pub fn device_location(&self) -> Option<i64> {
        self.raw.get("DeviceLocation").and_then(as_i64)
    }

    pub fn configuration(&self) -> Option<String> {
        self.raw
            .get("Configuration")
            .and_then(as_str)
            .map(str::to_string)
    }

    pub fn to_save_request(
        &self,
        name: impl Into<String>,
        state: &AtaState,
    ) -> Result<RemotePresetSaveRequest> {
        let number = self.number.ok_or_else(|| {
            MelcloudError::Protocol("remote preset slot number is missing".to_string())
        })?;
        let device_id = self.device_location().ok_or_else(|| {
            MelcloudError::Protocol("remote preset device id is missing".to_string())
        })?;
        RemotePresetSaveRequest::from_state(device_id, number, name.into(), state)
    }
}

impl RemotePresetSaveRequest {
    pub fn from_state(
        device_id: i64,
        number: i64,
        number_description: String,
        state: &AtaState,
    ) -> Result<Self> {
        Ok(Self {
            device_id,
            number,
            number_as_string: number.to_string(),
            number_description,
            preset_request: RemotePresetRequest::from_state(state)?,
        })
    }

    pub fn to_form_fields(&self) -> Vec<(String, String)> {
        vec![
            ("DeviceId".to_string(), self.device_id.to_string()),
            ("Number".to_string(), self.number_as_string.clone()),
            (
                "NumberDescription".to_string(),
                self.number_description.clone(),
            ),
            (
                "PresetRequest.Power".to_string(),
                self.preset_request.power.to_string().to_lowercase(),
            ),
            (
                "PresetRequest.SetTemperature".to_string(),
                self.preset_request.set_temperature.to_string(),
            ),
            (
                "PresetRequest.OperationMode".to_string(),
                self.preset_request.operation_mode.to_string(),
            ),
            (
                "PresetRequest.VaneHorizontal".to_string(),
                self.preset_request.vane_horizontal.to_string(),
            ),
            (
                "PresetRequest.VaneVertical".to_string(),
                self.preset_request.vane_vertical.to_string(),
            ),
            (
                "PresetRequest.FanSpeed".to_string(),
                self.preset_request.fan_speed.to_string(),
            ),
        ]
    }

    pub fn to_wire_payload(&self) -> Value {
        json!({
            "DeviceId": self.device_id,
            "Number": self.number_as_string,
            "NumberDescription": self.number_description,
            "PresetRequest": {
                "Power": self.preset_request.power,
                "SetTemperature": self.preset_request.set_temperature,
                "OperationMode": self.preset_request.operation_mode,
                "VaneHorizontal": self.preset_request.vane_horizontal,
                "VaneVertical": self.preset_request.vane_vertical,
                "FanSpeed": self.preset_request.fan_speed,
            }
        })
    }
}

impl RemotePresetRequest {
    pub fn from_state(state: &AtaState) -> Result<Self> {
        Ok(Self {
            power: state
                .power()
                .ok_or_else(|| MelcloudError::Protocol("missing live power state".to_string()))?,
            set_temperature: state.target_temperature().ok_or_else(|| {
                MelcloudError::Protocol("missing live target temperature".to_string())
            })?,
            operation_mode: state.operation_mode_raw().ok_or_else(|| {
                MelcloudError::Protocol("missing live operation mode".to_string())
            })?,
            vane_horizontal: state.vane_horizontal_raw().ok_or_else(|| {
                MelcloudError::Protocol("missing live horizontal vane state".to_string())
            })?,
            vane_vertical: state.vane_vertical_raw().ok_or_else(|| {
                MelcloudError::Protocol("missing live vertical vane state".to_string())
            })?,
            fan_speed: state.fan_speed_raw().ok_or_else(|| {
                MelcloudError::Protocol("missing live fan speed state".to_string())
            })?,
        })
    }
}

pub(crate) fn remote_presets_from_node(node: &Value) -> Vec<RemotePreset> {
    node.get("Presets")
        .and_then(Value::as_array)
        .map(|presets| {
            presets
                .iter()
                .filter_map(remote_preset_from_value)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn remote_presets_from_response_value(value: Value) -> Vec<RemotePreset> {
    match value {
        Value::Array(items) => items.iter().filter_map(remote_preset_from_value).collect(),
        Value::Object(_) => remote_preset_from_value(&value).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn remote_preset_from_value(value: &Value) -> Option<RemotePreset> {
    let name = value
        .get("Name")
        .and_then(as_str)
        .or_else(|| value.get("NumberDescription").and_then(as_str))
        .or_else(|| value.get("DeviceLocation").and_then(as_str))
        .unwrap_or("Unnamed preset")
        .to_string();
    let operation_mode_code = value.get("OperationMode").and_then(as_i64);
    let fan_speed_code = value.get("FanSpeed").and_then(as_i64);
    let vane_horizontal_code = value.get("VaneHorizontal").and_then(as_i64);
    let vane_vertical_code = value.get("VaneVertical").and_then(as_i64);

    Some(RemotePreset {
        name,
        number: value.get("Number").and_then(as_i64),
        id: value.get("ID").and_then(as_i64),
        state: RemotePresetState {
            power: value.get("Power").and_then(Value::as_bool),
            operation_mode_code,
            operation_mode: operation_mode_code.map(operation_mode_to_cli_value),
            target_temperature: value.get("SetTemperature").and_then(as_f64),
            fan_speed_code,
            fan_speed: fan_speed_code.map(fan_speed_to_label),
            vane_horizontal_code,
            vane_horizontal: vane_horizontal_code.map(horizontal_vane_to_label),
            vane_vertical_code,
            vane_vertical: vane_vertical_code.map(vertical_vane_to_label),
        },
        raw: value.clone(),
    })
}
