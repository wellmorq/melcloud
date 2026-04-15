use crate::file_store::{read_backup_text, read_text_with_backup, write_with_backup};
use crate::models::{DeviceProfileLocation, RemotePresetBackup};
use crate::presets::local_preset_path;
use melcloud_core::{MelcloudError, RemotePreset};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn select_remote_preset<'a>(
    presets: &'a [RemotePreset],
    selector: &str,
) -> Result<&'a RemotePreset, MelcloudError> {
    let trimmed = selector.trim();
    if let Ok(number) = trimmed.parse::<i64>() {
        if let Some(preset) = presets.iter().find(|preset| preset.number == Some(number)) {
            return Ok(preset);
        }
    }

    let folded = trimmed.to_lowercase();
    presets
        .iter()
        .find(|preset| preset.name.to_lowercase() == folded)
        .ok_or_else(|| {
            MelcloudError::Protocol(format!(
                "remote preset `{trimmed}` not found. run `remote-preset list`"
            ))
        })
}

pub(crate) fn remote_preset_export_path(
    preset_dir: &Path,
    preset: &RemotePreset,
    explicit_output: Option<&PathBuf>,
) -> PathBuf {
    explicit_output
        .cloned()
        .unwrap_or_else(|| local_preset_path(preset_dir, &slugify_name(&preset.name)))
}

pub(crate) fn remote_preset_export_description(preset: &RemotePreset) -> String {
    match preset.number {
        Some(number) => format!("Exported from MELCloud server preset #{number}"),
        None => "Exported from MELCloud server preset".to_string(),
    }
}

pub(crate) fn remote_preset_label(preset: &RemotePreset) -> String {
    match preset.number {
        Some(number) => format!("{} (#{number})", preset.name),
        None => preset.name.clone(),
    }
}

pub(crate) fn remote_preset_backup_dir(
    profile_location: &DeviceProfileLocation,
) -> Result<PathBuf, MelcloudError> {
    let base = profile_location
        .primary
        .parent()
        .ok_or_else(|| MelcloudError::Protocol("invalid device profile path".to_string()))?;
    let path = base.join("remote-preset-backups");
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub(crate) fn remote_preset_backup_path(backup_dir: &Path, slot: i64) -> PathBuf {
    backup_dir.join(format!("slot-{slot}.json"))
}

pub(crate) fn write_remote_preset_backup(
    path: &Path,
    preset: &RemotePreset,
) -> Result<(), MelcloudError> {
    let payload = RemotePresetBackup {
        saved_at: chrono::Utc::now().to_rfc3339(),
        preset: preset.clone(),
    };
    let raw = serde_json::to_string_pretty(&payload)?;
    write_with_backup(path, raw.as_bytes())
}

pub(crate) fn read_remote_preset_backup(path: &Path) -> Result<RemotePresetBackup, MelcloudError> {
    let raw = read_text_with_backup(path)?.ok_or_else(|| {
        MelcloudError::Protocol(format!("remote preset backup missing: {}", path.display()))
    })?;
    parse_remote_preset_backup(&raw).or_else(|primary_err| {
        if let Some(backup_raw) = read_backup_text(path)? {
            if let Ok(backup) = parse_remote_preset_backup(&backup_raw) {
                return Ok(backup);
            }
        }
        Err(primary_err)
    })
}

fn slugify_name(input: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;

    for ch in input.trim().chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "remote-preset".to_string()
    } else {
        slug
    }
}

fn parse_remote_preset_backup(raw: &str) -> Result<RemotePresetBackup, MelcloudError> {
    serde_json::from_str(raw).map_err(MelcloudError::from)
}
