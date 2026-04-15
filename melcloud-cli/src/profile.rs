use crate::file_store::{backup_path, read_backup_text, read_text_with_backup, write_with_backup};
use crate::models::{DeviceProfile, DeviceProfileLocation};
use melcloud_core::{BoundDevice, DiscoveredAtaDevice, MelcloudClient, MelcloudError};
use std::path::{Path, PathBuf};

pub(crate) fn read_device_profile(path: &Path) -> Result<DeviceProfile, MelcloudError> {
    let raw = read_text_with_backup(path)?.ok_or_else(|| {
        MelcloudError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("device profile missing: {}", path.display()),
        ))
    })?;
    parse_device_profile(&raw, path).or_else(|primary_err| {
        if let Some(backup_raw) = read_backup_text(path)? {
            if let Ok(profile) = parse_device_profile(&backup_raw, path) {
                return Ok(profile);
            }
        }
        Err(primary_err)
    })
}

pub(crate) fn load_device_profile(
    location: &DeviceProfileLocation,
) -> Result<Option<(DeviceProfile, PathBuf)>, MelcloudError> {
    let path = &location.primary;
    if !path.exists() && !backup_path(path).exists() {
        return Ok(None);
    }
    Ok(Some((read_device_profile(path)?, path.clone())))
}

pub(crate) fn save_device_profile(
    location: &DeviceProfileLocation,
    device: &BoundDevice,
) -> Result<(), MelcloudError> {
    let profile = DeviceProfile {
        device_id: device.device_id,
        building_id: device.building_id,
        name: device.name.clone(),
    };
    let raw = serde_yaml::to_string(&profile).map_err(|err| {
        MelcloudError::Protocol(format!("serialize device profile failed: {err}"))
    })?;
    write_with_backup(&location.primary, raw.as_bytes())
}

pub(crate) async fn resolve_device_target(
    client: &mut MelcloudClient,
    profile_location: &DeviceProfileLocation,
    force_sync: bool,
) -> Result<BoundDevice, MelcloudError> {
    Ok(
        resolve_discovered_device(client, profile_location, force_sync)
            .await?
            .device,
    )
}

pub(crate) async fn resolve_discovered_device(
    client: &mut MelcloudClient,
    profile_location: &DeviceProfileLocation,
    force_sync: bool,
) -> Result<DiscoveredAtaDevice, MelcloudError> {
    let devices = client.list_device_nodes().await?;
    let profile = if force_sync {
        None
    } else {
        load_device_profile(profile_location)?
    };

    let selected =
        select_discovered_device(&devices, profile.as_ref().map(|(profile, _)| profile))?;
    if force_sync || profile.is_none() {
        save_device_profile(profile_location, &selected.device)?;
    }
    Ok(selected)
}

pub(crate) fn select_discovered_device(
    devices: &[DiscoveredAtaDevice],
    profile: Option<&DeviceProfile>,
) -> Result<DiscoveredAtaDevice, MelcloudError> {
    let index = select_device_index(devices, profile)?;
    Ok(devices[index].clone())
}

fn select_device_index(
    devices: &[DiscoveredAtaDevice],
    profile: Option<&DeviceProfile>,
) -> Result<usize, MelcloudError> {
    if let Some(profile) = profile {
        return devices
            .iter()
            .position(|device| {
                device.device.device_id == profile.device_id
                    && device.device.building_id == profile.building_id
            })
            .ok_or_else(|| {
                MelcloudError::Protocol(format!(
                    "saved device {} is no longer in the ATA device list. run `devices sync`",
                    profile.device_id
                ))
            });
    }

    match devices.len() {
        0 => Err(MelcloudError::NoDevices),
        1 => Ok(0),
        count => Err(MelcloudError::Protocol(format!(
            "expected exactly one ATA device, found {count}. run `devices list` for diagnostics"
        ))),
    }
}

fn parse_device_profile(raw: &str, path: &Path) -> Result<DeviceProfile, MelcloudError> {
    serde_yaml::from_str::<DeviceProfile>(raw).map_err(|err| {
        MelcloudError::Protocol(format!(
            "invalid device profile {}: {}",
            path.display(),
            err
        ))
    })
}
