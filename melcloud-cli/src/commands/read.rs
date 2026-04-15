use crate::json_output::{status_to_json, weather_to_json};
use crate::models::DeviceProfileLocation;
use crate::profile::resolve_device_target;
use crate::render::{format_status_block, format_weather_block};
use melcloud_core::{MelcloudClient, MelcloudError};

pub(crate) async fn handle_status(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
) -> Result<Option<String>, MelcloudError> {
    let device = resolve_device_target(client, profile_location, false).await?;
    let state = client.get_device_status(&device).await?;
    if json_output {
        return Ok(Some(serde_json::to_string_pretty(&status_to_json(
            &device, &state,
        ))?));
    }
    Ok(Some(format_status_block("status", &state, &device)))
}

pub(crate) async fn handle_weather(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
) -> Result<Option<String>, MelcloudError> {
    let device = resolve_device_target(client, profile_location, false).await?;
    let state = client.get_device_status(&device).await?;
    if json_output {
        return Ok(Some(serde_json::to_string_pretty(&weather_to_json(
            &device, &state,
        ))?));
    }
    Ok(Some(format_weather_block(&state, &device)))
}
