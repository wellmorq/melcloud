use crate::args::DeviceAction;
use crate::json_output::devices_map_json;
use crate::models::DeviceProfileLocation;
use crate::profile::resolve_discovered_device;
use melcloud_core::{MelcloudClient, MelcloudError};

pub(crate) async fn handle(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    action: &DeviceAction,
) -> Result<Option<String>, MelcloudError> {
    match action {
        DeviceAction::List => {
            let devices = client.list_device_nodes().await?;
            if json_output {
                return Ok(Some(serde_json::to_string_pretty(&devices_map_json(
                    &devices,
                ))?));
            }
            let lines: Vec<String> = devices
                .iter()
                .enumerate()
                .map(|(idx, device)| {
                    format!(
                        "{}) {} | id={} | building={} | remote_presets={}",
                        idx + 1,
                        device.device.name,
                        device.device.device_id,
                        device.device.building_id,
                        device.presets.len()
                    )
                })
                .collect();
            Ok(Some(lines.join("\n")))
        }
        DeviceAction::Sync => {
            let discovered = resolve_discovered_device(client, profile_location, true).await?;
            let device = discovered.device;
            Ok(Some(format!(
                "synced device profile: {} (id={}, building={}) | remote_presets={} -> {}",
                device.name,
                device.device_id,
                device.building_id,
                discovered.presets.len(),
                profile_location.primary.display()
            )))
        }
    }
}
