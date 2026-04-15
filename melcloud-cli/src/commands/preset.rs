use crate::args::PresetAction;
use crate::json_output::{preset_preview_to_json, status_to_json};
use crate::models::DeviceProfileLocation;
use crate::presets::{
    capture_local_preset_from_state, list_local_preset_names, load_local_preset, local_preset_path,
    normalize_preset_state_key, parse_preset_scalar_value, patch_from_local_preset,
    render_local_preset_yaml, write_local_preset_file,
};
use crate::profile::resolve_device_target;
use crate::render::{format_preset_preview_block, format_status_block};
use crate::verify::wait_for_expected_config;
use melcloud_core::{AtaState, MelcloudClient, MelcloudError};
use serde_json::json;
use std::path::Path;

pub(crate) async fn handle(
    client: &mut MelcloudClient,
    json_output: bool,
    preset_dir: &Path,
    profile_location: &DeviceProfileLocation,
    action: &PresetAction,
) -> Result<Option<String>, MelcloudError> {
    match action {
        PresetAction::List => {
            let names = list_local_preset_names(preset_dir);
            if json_output {
                Ok(Some(serde_json::to_string_pretty(&json!(names))?))
            } else if names.is_empty() {
                Ok(Some("No presets".to_string()))
            } else {
                Ok(Some(names.join("\n")))
            }
        }
        PresetAction::Show { name } => {
            let preset = load_local_preset(preset_dir, name)?;
            if json_output {
                Ok(Some(serde_json::to_string_pretty(&preset)?))
            } else {
                Ok(Some(render_local_preset_yaml(&preset)?))
            }
        }
        PresetAction::Init { name } => {
            let path = local_preset_path(preset_dir, name);
            if path.exists() {
                return Err(MelcloudError::Protocol("preset already exists".to_string()));
            }
            let preset = crate::models::PresetFile::empty(name);
            write_local_preset_file(&path, &preset)?;
            if json_output {
                Ok(Some(serde_json::to_string_pretty(&json!({
                    "status": "created",
                    "path": path,
                }))?))
            } else {
                Ok(Some(format!("created {}", path.display())))
            }
        }
        PresetAction::Capture { name } => {
            let path = local_preset_path(preset_dir, name);
            if path.exists() {
                return Err(MelcloudError::Protocol("preset already exists".to_string()));
            }
            let device = resolve_device_target(client, profile_location, false).await?;
            let state = client.get_device_status(&device).await?;
            let preset = capture_local_preset_from_state(name, &state);
            write_local_preset_file(&path, &preset)?;
            if json_output {
                Ok(Some(serde_json::to_string_pretty(&preset)?))
            } else {
                Ok(Some(format!("captured {}", path.display())))
            }
        }
        PresetAction::Preview { name } => {
            preview_preset(client, json_output, profile_location, preset_dir, name).await
        }
        PresetAction::SetField { name, key, value } => {
            let path = local_preset_path(preset_dir, name);
            let mut preset = load_local_preset(preset_dir, name)?;
            let canonical_key = normalize_preset_state_key(key)?;
            let parsed = parse_preset_scalar_value(value)?;
            preset.state.insert(canonical_key.to_string(), parsed);
            write_local_preset_file(&path, &preset)?;
            Ok(Some("updated".to_string()))
        }
        PresetAction::Apply { name, verify } => {
            apply_preset(
                client,
                json_output,
                profile_location,
                preset_dir,
                name,
                *verify,
            )
            .await
        }
    }
}

async fn preview_preset(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    preset_dir: &Path,
    name: &str,
) -> Result<Option<String>, MelcloudError> {
    let device = resolve_device_target(client, profile_location, false).await?;
    let preset = load_local_preset(preset_dir, name)?;
    let patch = patch_from_local_preset(&preset)?;
    if patch.is_empty() {
        return Err(MelcloudError::Protocol(
            "preset has no settable fields".to_string(),
        ));
    }
    let current = client.get_device_status(&device).await?;
    let preview = current.clone().apply_patch(&patch)?;
    let preview_state = AtaState::from_json(preview.payload.clone());
    if json_output {
        return Ok(Some(serde_json::to_string_pretty(
            &preset_preview_to_json(&device, &preset, &patch, &current, &preview),
        )?));
    }
    Ok(Some(format_preset_preview_block(
        &device,
        &preset,
        &current,
        &preview_state,
        preview.flags,
    )))
}

async fn apply_preset(
    client: &mut MelcloudClient,
    json_output: bool,
    profile_location: &DeviceProfileLocation,
    preset_dir: &Path,
    name: &str,
    verify: bool,
) -> Result<Option<String>, MelcloudError> {
    let device = resolve_device_target(client, profile_location, false).await?;
    let preset = load_local_preset(preset_dir, name)?;
    let patch = patch_from_local_preset(&preset)?;
    if patch.is_empty() {
        return Err(MelcloudError::Protocol(
            "preset has no settable fields".to_string(),
        ));
    }
    let current = client.get_current_config(&device).await?;
    let prepared = client.prepare_config_command(&current, &patch)?;
    let state = client.send_config_command(&device, &prepared).await?;
    if verify {
        let expected = AtaState::from_json(prepared.payload.clone());
        let verified = wait_for_expected_config(client, &device, &expected).await?;
        return render_status_response(json_output, "applied preset", &device, &verified);
    }
    render_status_response(json_output, "applied preset", &device, &state)
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
