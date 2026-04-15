use crate::args::PatchArgs;
use crate::json_output::{patch_to_json, status_to_json};
use crate::models::DeviceProfileLocation;
use crate::profile::resolve_discovered_device;
use crate::remote_presets::{
    read_remote_preset_backup, remote_preset_backup_dir, remote_preset_backup_path,
    select_remote_preset, write_remote_preset_backup,
};
use crate::render::{format_config_preview_block, format_status_block};
use crate::verify::{wait_for_expected_config, wait_for_remote_preset};
use melcloud_core::{AtaState, MelcloudClient, MelcloudError, PatchResult};
use serde_json::json;
use std::fs;

pub(crate) async fn handle_apply(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    selector: &str,
    verify: bool,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let preset = select_remote_preset(&discovered.presets, selector)?;
    let current = client.get_current_config(&discovered.device).await?;
    let prepared = client.prepare_config_command(&current, &preset.as_patch())?;
    let state = client
        .send_config_command(&discovered.device, &prepared)
        .await?;

    if verify {
        let expected = AtaState::from_json(prepared.payload.clone());
        let verified = wait_for_expected_config(client, &discovered.device, &expected).await?;
        return render_status_response(
            json_output,
            "applied remote preset",
            &discovered.device,
            &verified,
        );
    }
    render_status_response(
        json_output,
        "applied remote preset",
        &discovered.device,
        &state,
    )
}

pub(crate) async fn handle_save(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    slot: i64,
    name: &str,
    patch_args: &PatchArgs,
    preview: bool,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let backup_dir = remote_preset_backup_dir(profile_location)?;
    let current = client.get_current_config(&discovered.device).await?;
    let patch = crate::patch_input::patch_from_args(patch_args, Some(&current))?;
    let prepared = if patch.is_empty() {
        PatchResult {
            payload: current.raw().clone(),
            flags: 0,
        }
    } else {
        client.prepare_config_command(&current, &patch)?
    };
    let target_state = AtaState::from_json(prepared.payload.clone());
    let existing = discovered
        .presets
        .iter()
        .find(|preset| preset.number == Some(slot))
        .cloned();

    if preview {
        if json_output {
            return Ok(Some(serde_json::to_string_pretty(&json!({
                "device": crate::json_output::device_json(&discovered.device),
                "slot": slot,
                "name": name,
                "existing_remote_preset": existing,
                "requested_patch": patch_to_json(&patch),
                "target_config": target_state.summary(),
                "payload": prepared.payload,
            }))?));
        }
        return Ok(Some(format!(
            "remote preset save preview: slot #{slot} -> {name}\n{}",
            format_config_preview_block(
                &discovered.device,
                "target config",
                &current,
                &target_state,
                prepared.flags,
            )
        )));
    }

    let existing = existing.ok_or_else(|| {
        MelcloudError::Protocol(format!(
            "remote preset slot #{slot} not found on this MELCloud account"
        ))
    })?;
    let backup_path = remote_preset_backup_path(&backup_dir, slot);
    write_remote_preset_backup(&backup_path, &existing)?;
    let request = existing.to_save_request(name.to_string(), &target_state)?;
    client.save_remote_preset(&request).await?;
    let saved = wait_for_remote_preset(client, slot, name, &target_state.as_patch()).await?;

    if json_output {
        Ok(Some(serde_json::to_string_pretty(&json!({
            "status": "saved",
            "slot": slot,
            "backup_path": backup_path,
            "remote_preset": saved,
        }))?))
    } else {
        Ok(Some(format!(
            "saved remote preset slot #{slot} as `{name}`\nbackup: {}",
            backup_path.display()
        )))
    }
}

pub(crate) async fn handle_delete(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    selector: &str,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let backup_dir = remote_preset_backup_dir(profile_location)?;
    let preset = select_remote_preset(&discovered.presets, selector)?;
    let slot = preset.number.ok_or_else(|| {
        MelcloudError::Protocol("remote preset slot number is missing".to_string())
    })?;
    let backup_path = remote_preset_backup_path(&backup_dir, slot);
    let backup = read_remote_preset_backup(&backup_path)?;
    let current = client.get_current_config(&discovered.device).await?;
    let restored = client.prepare_config_command(&current, &backup.preset.as_patch())?;
    let restored_state = AtaState::from_json(restored.payload.clone());
    let request = backup
        .preset
        .to_save_request(backup.preset.name.clone(), &restored_state)?;
    client.save_remote_preset(&request).await?;
    let saved = wait_for_remote_preset(
        client,
        slot,
        backup.preset.name.as_str(),
        &backup.preset.as_patch(),
    )
    .await?;
    let _ = fs::remove_file(&backup_path);

    if json_output {
        Ok(Some(serde_json::to_string_pretty(&json!({
            "status": "restored",
            "slot": slot,
            "remote_preset": saved,
        }))?))
    } else {
        Ok(Some(format!(
            "restored remote preset slot #{slot} from backup"
        )))
    }
}

fn render_status_response(
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
