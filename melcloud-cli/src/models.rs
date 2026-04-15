use melcloud_core::RemotePreset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct PresetFile {
    pub name: String,
    pub description: Option<String>,
    pub state: BTreeMap<String, serde_yaml::Value>,
}

impl PresetFile {
    pub(crate) fn empty(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            state: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DeviceProfile {
    pub device_id: i64,
    pub building_id: i64,
    pub name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DeviceProfileLocation {
    pub primary: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TemperatureValue {
    Absolute(f64),
    Relative(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct RemotePresetBackup {
    pub saved_at: String,
    pub preset: RemotePreset,
}
