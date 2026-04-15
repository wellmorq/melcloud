use crate::json_output::device_json;
use crate::models::DeviceProfileLocation;
use crate::presets::{local_preset_from_patch, write_local_preset_file};
use crate::profile::resolve_discovered_device;
use crate::remote_presets::{
    remote_preset_export_description, remote_preset_export_path, remote_preset_label,
    select_remote_preset,
};
use crate::render::{
    format_config_preview_block, format_remote_preset_block, format_remote_preset_list_block,
};
use melcloud_core::{AtaState, MelcloudClient, MelcloudError};
use serde_json::json;
use std::path::{Path, PathBuf};

pub(crate) async fn handle_list(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let mut presets = discovered.presets.clone();
    presets.sort_by_key(|preset| (preset.number.unwrap_or(i64::MAX), preset.name.clone()));

    if json_output {
        return Ok(Some(serde_json::to_string_pretty(&json!({
            "device": device_json(&discovered.device),
            "remote_presets": presets,
            "raw": discovered.raw,
        }))?));
    }
    if presets.is_empty() {
        Ok(Some(format!(
            "remote presets: {} ({})\nnone",
            discovered.device.name, discovered.device.device_id
        )))
    } else {
        Ok(Some(format_remote_preset_list_block(
            &discovered.device,
            &presets,
        )))
    }
}

pub(crate) async fn handle_show(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    selector: &str,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let preset = select_remote_preset(&discovered.presets, selector)?;
    if json_output {
        Ok(Some(serde_json::to_string_pretty(&json!({
            "device": device_json(&discovered.device),
            "remote_preset": preset,
        }))?))
    } else {
        Ok(Some(format_remote_preset_block(&discovered.device, preset)))
    }
}

pub(crate) async fn handle_preview(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    selector: &str,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let preset = select_remote_preset(&discovered.presets, selector)?;
    let current = client.get_current_config(&discovered.device).await?;
    let patch = preset.as_patch();
    let preview = client.prepare_config_command(&current, &patch)?;
    let preview_state = AtaState::from_json(preview.payload.clone());

    if json_output {
        return Ok(Some(serde_json::to_string_pretty(&json!({
            "device": device_json(&discovered.device),
            "remote_preset": preset,
            "current_status": current.summary(),
            "requested_patch": crate::json_output::patch_to_json(&patch),
            "changes": crate::preview::preview_changes(&current, &preview_state).into_iter().map(|change| json!({
                "field": change.field,
                "before": change.before,
                "after": change.after,
            })).collect::<Vec<_>>(),
            "effective_flags": {
                "decimal": preview.flags,
                "hex": format!("0x{:x}", preview.flags),
            },
            "payload": preview.payload,
        }))?));
    }
    Ok(Some(format_config_preview_block(
        &discovered.device,
        &format!("remote preset preview: {}", remote_preset_label(preset)),
        &current,
        &preview_state,
        preview.flags,
    )))
}

pub(crate) async fn handle_export(
    client: &mut MelcloudClient,
    json_output: bool,
    preset_dir: &Path,
    profile_location: &DeviceProfileLocation,
    selector: &str,
    output: Option<&PathBuf>,
) -> Result<Option<String>, MelcloudError> {
    let discovered = resolve_discovered_device(client, profile_location, false).await?;
    let preset = select_remote_preset(&discovered.presets, selector)?;
    let path = remote_preset_export_path(preset_dir, preset, output);
    if path.exists() {
        return Err(MelcloudError::Protocol(format!(
            "export target already exists: {}",
            path.display()
        )));
    }

    let local = local_preset_from_patch(
        &preset.name,
        Some(remote_preset_export_description(preset)),
        &preset.as_patch(),
    );
    write_local_preset_file(&path, &local)?;

    if json_output {
        Ok(Some(serde_json::to_string_pretty(&json!({
            "status": "exported",
            "path": path,
            "device": device_json(&discovered.device),
            "remote_preset": preset,
            "local_preset": local,
        }))?))
    } else {
        Ok(Some(format!(
            "exported remote preset {} -> {}",
            remote_preset_label(preset),
            path.display()
        )))
    }
}
