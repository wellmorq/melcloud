mod client;
mod discovery;
mod error;
mod json_value;
mod models;
mod parse;
mod patch;
mod remote_preset;
mod session;
mod state;
mod transport;

pub use client::{MelcloudClient, MelcloudConfig};
pub use error::{MelcloudError, Result};
pub use models::{AtaStatusSummary, BoundDevice, DiscoveredAtaDevice, WeatherObservation};
pub use parse::{parse_fan_speed, parse_horizontal_vane, parse_mode_input, parse_vertical_vane};
pub use patch::{
    AtaPatch, DeviceConfigPatch, DeviceConfigSnapshot, PatchResult, PreparedConfigCommand,
    EFFECTIVE_FLAG_PRESET,
};
pub use remote_preset::{
    RemotePreset, RemotePresetRequest, RemotePresetSaveRequest, RemotePresetState,
};
pub use session::Session;
pub use state::AtaState;

#[cfg(test)]
mod tests;
