use crate::args::PatchArgs;
use crate::json_output::{config_preview_to_json, devices_map_json, status_to_json};
use crate::models::{DeviceProfile, PresetFile};
use crate::patch_input::patch_from_args;
use crate::presets::{
    capture_local_preset_from_state, load_local_preset, normalize_preset_state_key,
    patch_from_local_preset, write_local_preset_file,
};
use crate::profile::{read_device_profile, save_device_profile, select_discovered_device};
use crate::remote_presets::{
    read_remote_preset_backup, remote_preset_backup_path, remote_preset_export_path,
    select_remote_preset, write_remote_preset_backup,
};
use crate::runtime::{
    device_profile_location, parse_language_id, resolve_language_id, session_file_location,
};
use melcloud_core::{AtaState, BoundDevice, DiscoveredAtaDevice, RemotePreset};
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

fn sample_device() -> BoundDevice {
    BoundDevice {
        name: "Main Room".to_string(),
        building_id: 211547,
        device_id: 12066563,
    }
}

fn sample_profile() -> DeviceProfile {
    DeviceProfile {
        name: "Main Room".to_string(),
        building_id: 211547,
        device_id: 12066563,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("melcloud-cli-tests-{name}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn preset_with_mode(name: &str, mode: &str) -> PresetFile {
    PresetFile {
        name: name.to_string(),
        description: None,
        state: BTreeMap::from([(
            "mode".to_string(),
            serde_yaml::Value::String(mode.to_string()),
        )]),
    }
}

fn device_with_id(device_id: i64) -> BoundDevice {
    BoundDevice {
        name: format!("Device {device_id}"),
        building_id: 211547,
        device_id,
    }
}

fn sample_remote_preset() -> RemotePreset {
    RemotePreset {
        name: "Ventilation".to_string(),
        number: Some(2),
        id: Some(502),
        state: melcloud_core::RemotePresetState {
            power: Some(true),
            operation_mode_code: Some(7),
            operation_mode: Some("fan_only".to_string()),
            target_temperature: Some(26.0),
            fan_speed_code: Some(2),
            fan_speed: Some("2".to_string()),
            vane_horizontal_code: Some(8),
            vane_horizontal: Some("split".to_string()),
            vane_vertical_code: Some(0),
            vane_vertical: Some("auto".to_string()),
        },
        raw: json!({
            "ID": 502,
            "Number": 2,
            "Name": "Ventilation"
        }),
    }
}

fn sample_discovered_device() -> DiscoveredAtaDevice {
    DiscoveredAtaDevice {
        device: sample_device(),
        presets: vec![sample_remote_preset()],
        raw: json!({
            "DeviceName": "Main Room",
            "Presets": [
                {
                    "ID": 502,
                    "Number": 2,
                    "Name": "Ventilation"
                }
            ]
        }),
    }
}

fn sample_state() -> AtaState {
    AtaState::from_json(json!({
        "Power": true,
        "OperationMode": 8,
        "SetTemperature": 26.0,
        "RoomTemperature": 27.0,
        "SetFanSpeed": 0,
        "VaneHorizontal": 12,
        "VaneVertical": 7,
        "NumberOfFanSpeeds": 5,
        "HideVaneControls": false,
        "WeatherObservations": [
            {
                "Date": "2026-04-15T15:00:00",
                "Temperature": 13,
                "Humidity": 53,
                "ConditionName": "Partly Cloudy"
            }
        ]
    }))
}

#[test]
fn select_discovered_device_matches_saved_profile() {
    let resolved =
        select_discovered_device(&[sample_discovered_device()], Some(&sample_profile())).unwrap();
    assert_eq!(resolved.device.device_id, 12066563);
}

#[test]
fn select_discovered_device_requires_single_device_without_profile() {
    let err = select_discovered_device(
        &[
            sample_discovered_device(),
            DiscoveredAtaDevice {
                device: BoundDevice {
                    name: "Secondary".to_string(),
                    building_id: 1,
                    device_id: 2,
                },
                presets: Vec::new(),
                raw: json!({}),
            },
        ],
        None,
    )
    .unwrap_err();

    assert!(matches!(err, melcloud_core::MelcloudError::Protocol(_)));
}

#[test]
fn preset_capture_uses_readable_aliases() {
    let preset = capture_local_preset_from_state("captured", &sample_state());

    assert_eq!(
        preset
            .state
            .get("fan_speed")
            .and_then(|value| value.as_str()),
        Some("auto")
    );
    assert_eq!(
        preset
            .state
            .get("vane_horizontal")
            .and_then(|value| value.as_str()),
        Some("swing")
    );
    assert_eq!(
        preset
            .state
            .get("vane_vertical")
            .and_then(|value| value.as_str()),
        Some("swing")
    );
}

#[test]
fn preset_to_patch_accepts_string_aliases() {
    let preset = PresetFile {
        name: "test".to_string(),
        description: None,
        state: BTreeMap::from([
            (
                "mode".to_string(),
                serde_yaml::Value::String("auto".to_string()),
            ),
            (
                "fan_speed".to_string(),
                serde_yaml::Value::String("auto".to_string()),
            ),
            (
                "vane_horizontal".to_string(),
                serde_yaml::Value::String("split".to_string()),
            ),
            (
                "vane_vertical".to_string(),
                serde_yaml::Value::String("swing".to_string()),
            ),
        ]),
    };

    let patch = patch_from_local_preset(&preset).unwrap();

    assert_eq!(patch.operation_mode.as_deref(), Some("auto"));
    assert_eq!(patch.fan_speed, Some(0));
    assert_eq!(patch.vane_horizontal, Some(8));
    assert_eq!(patch.vane_vertical, Some(7));
}

#[test]
fn preset_parser_rejects_unknown_keys() {
    let err = load_local_preset(Path::new("."), "missing").unwrap_err();
    assert!(matches!(err, melcloud_core::MelcloudError::Io(_)));

    let err = normalize_preset_state_key("mystery").unwrap_err();
    assert!(err.to_string().contains("unsupported preset key `mystery`"));
}

#[test]
fn corrupt_local_preset_loads_backup_copy() {
    let dir = temp_dir("preset-backup");
    let path = dir.join("saved.yaml");
    write_local_preset_file(&path, &preset_with_mode("saved", "cool")).unwrap();
    write_local_preset_file(&path, &preset_with_mode("saved", "heat")).unwrap();
    fs::write(&path, "{broken").unwrap();

    let preset = load_local_preset(&dir, "saved").unwrap();
    let patch = patch_from_local_preset(&preset).unwrap();

    assert_eq!(patch.operation_mode.as_deref(), Some("cool"));
}

#[test]
fn failed_local_preset_write_keeps_last_good_file() {
    let dir = temp_dir("preset-write-failure");
    let path = dir.join("saved.yaml");
    write_local_preset_file(&path, &preset_with_mode("saved", "cool")).unwrap();
    let mut temp_raw: OsString = path.as_os_str().to_owned();
    temp_raw.push(".tmp");
    fs::create_dir(PathBuf::from(temp_raw)).unwrap();

    let result = write_local_preset_file(&path, &preset_with_mode("saved", "heat"));

    assert!(result.is_err());
    let preset = load_local_preset(&dir, "saved").unwrap();
    let patch = patch_from_local_preset(&preset).unwrap();
    assert_eq!(patch.operation_mode.as_deref(), Some("cool"));
}

#[test]
fn corrupt_device_profile_loads_backup_copy() {
    let dir = temp_dir("profile-backup");
    let path = dir.join("device.yaml");
    let location = crate::models::DeviceProfileLocation {
        primary: path.clone(),
    };
    save_device_profile(&location, &sample_device()).unwrap();
    save_device_profile(&location, &device_with_id(42)).unwrap();
    fs::write(&path, "{broken").unwrap();

    let profile = read_device_profile(&path).unwrap();

    assert_eq!(profile.device_id, sample_device().device_id);
}

#[test]
fn corrupt_remote_preset_backup_loads_backup_copy() {
    let dir = temp_dir("remote-backup");
    let path = remote_preset_backup_path(&dir, 2);
    let mut replacement = sample_remote_preset();
    replacement.name = "Replacement".to_string();
    write_remote_preset_backup(&path, &sample_remote_preset()).unwrap();
    write_remote_preset_backup(&path, &replacement).unwrap();
    fs::write(&path, "{broken").unwrap();

    let backup = read_remote_preset_backup(&path).unwrap();

    assert_eq!(backup.preset.name, sample_remote_preset().name);
}

#[test]
fn preview_json_contains_diff_and_payload() {
    let preset = PresetFile {
        name: "preview".to_string(),
        description: Some("test".to_string()),
        state: BTreeMap::from([
            (
                "mode".to_string(),
                serde_yaml::Value::String("cool".to_string()),
            ),
            (
                "target_temperature".to_string(),
                serde_yaml::to_value(24.5).unwrap(),
            ),
        ]),
    };
    let patch = patch_from_local_preset(&preset).unwrap();
    let current = sample_state();
    let preview = current.clone().apply_patch(&patch).unwrap();
    let payload = crate::json_output::preset_preview_to_json(
        &sample_device(),
        &preset,
        &patch,
        &current,
        &preview,
    );

    assert_eq!(payload["preset"]["name"], json!("preview"));
    assert_eq!(payload["requested_patch"]["mode"], json!("cool"));
    assert_eq!(payload["payload"]["OperationMode"], json!(3));
    assert_eq!(payload["changes"][0]["field"], json!("mode"));
}

#[test]
fn status_json_contains_summary_and_raw() {
    let payload = status_to_json(&sample_device(), &sample_state());

    assert_eq!(payload["device"]["device_id"], json!(12066563));
    assert_eq!(payload["status"]["operation_mode"], json!("auto"));
    assert_eq!(payload["raw"]["SetTemperature"], json!(26.0));
}

#[test]
fn config_preview_json_contains_diff_and_payload() {
    let current = sample_state();
    let patch = melcloud_core::AtaPatch {
        target_temperature: Some(24.5),
        ..melcloud_core::AtaPatch::default()
    };
    let preview = current.prepare_command(&patch).unwrap();
    let payload = config_preview_to_json(&sample_device(), &patch, &current, &preview);

    assert_eq!(
        payload["requested_patch"]["target_temperature"],
        json!(24.5)
    );
    assert_eq!(payload["payload"]["SetTemperature"], json!(24.5));
    assert_eq!(payload["changes"][0]["field"], json!("target_temperature"));
}

#[test]
fn relative_temperature_uses_current_state() {
    let args = PatchArgs {
        power: None,
        mode: None,
        target_temperature: Some("+1".to_string()),
        fan_speed: None,
        vane_horizontal: None,
        vane_vertical: None,
    };

    let patch = patch_from_args(&args, Some(&sample_state())).unwrap();
    assert_eq!(patch.target_temperature, Some(27.0));
}

#[test]
fn canonical_preset_key_normalizes_operation_mode() {
    assert_eq!(
        normalize_preset_state_key("operation_mode").unwrap(),
        "mode"
    );
}

#[test]
fn remote_preset_export_uses_slugified_default_path() {
    let path = remote_preset_export_path(Path::new("presets"), &sample_remote_preset(), None);
    assert_eq!(path, PathBuf::from("presets").join("ventilation.yaml"));
}

#[test]
fn find_remote_preset_matches_number_and_name() {
    let preset = sample_remote_preset();
    let presets = vec![preset.clone()];

    assert_eq!(select_remote_preset(&presets, "2").unwrap().id, Some(502));
    assert_eq!(
        select_remote_preset(&presets, "ventilation").unwrap().name,
        preset.name
    );
}

#[test]
fn devices_json_includes_remote_presets_and_raw() {
    let payload = devices_map_json(&[sample_discovered_device()]);

    assert_eq!(payload[0]["device"]["device_id"], json!(12066563));
    assert_eq!(payload[0]["remote_preset_count"], json!(1));
    assert_eq!(
        payload[0]["remote_presets"][0]["name"],
        json!("Ventilation")
    );
    assert_eq!(payload[0]["raw"]["DeviceName"], json!("Main Room"));
}

#[test]
fn device_profile_defaults_to_state_directory() {
    let state_dir = PathBuf::from("state");
    let profile = device_profile_location(None, &state_dir);

    assert_eq!(profile.primary, state_dir.join("device.yaml"));
}

#[test]
fn session_file_defaults_to_state_directory() {
    let state_dir = PathBuf::from("state");

    assert_eq!(
        session_file_location(None, &state_dir),
        state_dir.join("session.json")
    );
}

#[test]
fn parse_language_id_accepts_named_site_languages() {
    assert_eq!(parse_language_id("en").unwrap(), 0);
    assert_eq!(parse_language_id("ru").unwrap(), 16);
}

#[test]
fn parse_language_id_accepts_numeric_codes() {
    assert_eq!(parse_language_id("0").unwrap(), 0);
    assert_eq!(parse_language_id("16").unwrap(), 16);
}

#[test]
fn resolve_language_id_prefers_cli_value() {
    let mut env = HashMap::from([("language".to_string(), "ru".to_string())]);
    assert_eq!(resolve_language_id(Some("en"), &mut env).unwrap(), 0);
}
