use crate::args::PatchArgs;
use crate::json_output::{config_preview_to_json, status_to_json};
use crate::models::DeviceProfileLocation;
use crate::patch_input::patch_from_args;
use crate::profile::resolve_device_target;
use crate::render::{format_config_preview_block, format_status_block};
use crate::verify::wait_for_expected_config;
use melcloud_core::{AtaState, MelcloudClient, MelcloudError};

pub(crate) async fn handle(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    patch_args: &PatchArgs,
    preview: bool,
    verify: bool,
) -> Result<Option<String>, MelcloudError> {
    let device = resolve_device_target(client, profile_location, false).await?;
    let current = client.get_current_config(&device).await?;
    let patch = patch_from_args(patch_args, Some(&current))?;
    if patch.is_empty() {
        return Err(MelcloudError::Protocol(
            "set command has no parameters".to_string(),
        ));
    }

    let prepared = client.prepare_config_command(&current, &patch)?;
    if preview {
        let preview_state = AtaState::from_json(prepared.payload.clone());
        if json_output {
            return Ok(Some(serde_json::to_string_pretty(
                &config_preview_to_json(&device, &patch, &current, &prepared),
            )?));
        }
        return Ok(Some(format_config_preview_block(
            &device,
            "config preview",
            &current,
            &preview_state,
            prepared.flags,
        )));
    }

    let state = client.send_config_command(&device, &prepared).await?;
    if verify {
        let expected = AtaState::from_json(prepared.payload.clone());
        let verified = wait_for_expected_config(client, &device, &expected).await?;
        return finish_status(json_output, "updated", &device, &verified);
    }
    finish_status(json_output, "set", &device, &state)
}

fn finish_status(
    json_output: bool,
    label: &str,
    device: &melcloud_core::BoundDevice,
    state: &melcloud_core::AtaState,
) -> Result<Option<String>, MelcloudError> {
    if json_output {
        Ok(Some(serde_json::to_string_pretty(&status_to_json(
            device, state,
        ))?))
    } else {
        Ok(Some(format_status_block(label, state, device)))
    }
}
