use crate::client::{MelcloudClient, MelcloudConfig};
use crate::discovery::flatten_ata_devices;
use crate::patch::{AtaPatch, EFFECTIVE_FLAG_PRESET};
use crate::remote_preset::{RemotePresetRequest, RemotePresetSaveRequest};
use crate::session::load_session_file;
use crate::{AtaState, BoundDevice, MelcloudError, Session};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_list_devices() -> Vec<Value> {
    serde_json::from_str(include_str!("../tests/fixtures/list_devices.json")).unwrap()
}

fn fixture_state() -> AtaState {
    AtaState::from_json(
        serde_json::from_str(include_str!("../tests/fixtures/device_get.json")).unwrap(),
    )
}

fn sample_device_tree() -> Vec<Value> {
    vec![json!({
        "Structure": {
            "BuildingID": 211547,
            "Devices": [
                {
                    "Presets": [
                        {
                            "ID": 1,
                            "Number": 1,
                            "Name": "Heating",
                            "Power": true,
                            "OperationMode": 1,
                            "SetTemperature": 26.5,
                            "FanSpeed": 2,
                            "VaneHorizontal": 0,
                            "VaneVertical": 7
                        }
                    ],
                    "Device": {
                        "DeviceID": 12066563,
                        "DeviceType": 0,
                        "DeviceName": "Main Room",
                        "BuildingID": 211547
                    }
                },
                {
                    "Device": {
                        "DeviceID": 999,
                        "DeviceType": 1,
                        "DeviceName": "ATW",
                        "BuildingID": 211547
                    }
                }
            ]
        }
    })]
}

fn sample_state() -> AtaState {
    AtaState::from_json(json!({
        "Power": true,
        "OperationMode": 8,
        "SetTemperature": 26.0,
        "RoomTemperature": 27.0,
        "SetFanSpeed": 3,
        "VaneHorizontal": 12,
        "VaneVertical": 7,
        "NumberOfFanSpeeds": 5,
        "HideVaneControls": false,
        "TemperatureIncrementOverride": 2,
        "Offline": false,
        "LastCommunication": "2026-04-15T12:00:51.35",
        "NextCommunication": "2026-04-15T12:01:51.35",
        "WeatherObservations": [
            {
                "Condition": 116,
                "ConditionName": "Partly Cloudy",
                "Date": "2026-04-15T15:00:00",
                "Day": 3,
                "Humidity": 53,
                "Icon": "wsymbol_0002_sunny_intervals",
                "Sunrise": "2026-04-15T05:40:00",
                "Sunset": "2026-04-15T19:10:00",
                "Temperature": 13,
                "WeatherType": 0
            }
        ]
    }))
}

#[test]
fn session_validity_uses_expiry_threshold() {
    let valid = Session {
        context_key: "abcdef".to_string(),
        obtained_at: Utc::now(),
        expiry: Some(Utc::now() + Duration::seconds(90)),
        duration_minutes: None,
        login_status: 0,
        user_name: None,
    };
    let expired = Session {
        expiry: Some(Utc::now() + Duration::seconds(10)),
        ..valid.clone()
    };

    assert!(valid.is_valid());
    assert!(!expired.is_valid());
}

#[test]
fn flatten_ata_devices_keeps_only_ata_devices_and_presets() {
    let devices = flatten_ata_devices(sample_device_tree());

    assert_eq!(devices.len(), 1);
    assert_eq!(
        devices[0].device,
        BoundDevice {
            name: "Main Room".to_string(),
            building_id: 211547,
            device_id: 12066563,
        }
    );
    assert_eq!(devices[0].presets.len(), 1);
    assert_eq!(devices[0].presets[0].name, "Heating");
    assert_eq!(
        devices[0].presets[0].state.operation_mode.as_deref(),
        Some("heat")
    );
}

#[test]
fn apply_patch_sets_flags_and_rounds_temperature() {
    let patch = AtaPatch {
        power: Some(false),
        target_temperature: Some(23.2),
        ..AtaPatch::default()
    };
    let result = sample_state().apply_patch(&patch).unwrap();

    assert_eq!(result.flags, 0x01 | 0x04);
    assert_eq!(result.payload["Power"], json!(false));
    assert_eq!(result.payload["SetTemperature"], json!(23.0));
    assert_eq!(result.payload["HasPendingCommand"], json!(true));
}

#[test]
fn apply_patch_rejects_fan_speed_out_of_range() {
    let patch = AtaPatch {
        fan_speed: Some(6),
        ..AtaPatch::default()
    };
    assert!(matches!(
        sample_state().apply_patch(&patch).unwrap_err(),
        MelcloudError::InvalidPayload(_)
    ));
}

#[test]
fn weather_observations_are_normalized() {
    let weather = sample_state().weather_observations();

    assert_eq!(weather.len(), 1);
    assert_eq!(weather[0].condition_name.as_deref(), Some("Partly Cloudy"));
    assert_eq!(weather[0].temperature, Some(13.0));
}

#[test]
fn fixture_list_devices_binds_single_ata_device() {
    let devices = flatten_ata_devices(fixture_list_devices());

    assert_eq!(devices.len(), 1);
    assert_eq!(
        devices[0].device,
        BoundDevice {
            name: "Primary ATA".to_string(),
            building_id: 100001,
            device_id: 200001,
        }
    );
    assert_eq!(devices[0].presets.len(), 3);
    assert_eq!(devices[0].presets[0].name, "Heating");
    assert_eq!(
        devices[0].presets[1].state.operation_mode.as_deref(),
        Some("fan_only")
    );
    assert_eq!(devices[0].presets[2].state.target_temperature, Some(27.0));
}

#[test]
fn fixture_device_state_preserves_live_shaped_summary() {
    let state = fixture_state();
    let summary = state.summary();

    assert_eq!(summary.operation_mode, "fan_only");
    assert_eq!(summary.operation_mode_code, Some(7));
    assert_eq!(summary.fan_speed_code, Some(3));
    assert_eq!(summary.vane_horizontal_code, Some(5));
    assert_eq!(summary.vane_vertical_code, Some(1));
    assert_eq!(state.supported_fan_speeds(), vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(
        state.supported_operation_modes(),
        vec!["off", "heat", "dry", "cool", "fan_only", "auto"]
    );
}

#[test]
fn apply_patch_rejects_unknown_vane_positions() {
    let patch = AtaPatch {
        vane_horizontal: Some(6),
        ..AtaPatch::default()
    };
    let err = fixture_state().apply_patch(&patch).unwrap_err();

    assert!(matches!(err, MelcloudError::InvalidPayload(_)));
    assert!(err.to_string().contains("allowed: 0, 1, 2, 3, 4, 5, 8, 12"));
}

#[test]
fn state_can_be_captured_back_into_patch() {
    let patch = sample_state().as_patch();

    assert_eq!(patch.power, Some(true));
    assert_eq!(patch.operation_mode.as_deref(), Some("auto"));
    assert_eq!(patch.vane_horizontal, Some(12));
    assert_eq!(patch.vane_vertical, Some(7));
}

#[test]
fn invalid_session_cache_is_ignored() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("melcloud-invalid-session-{unique}.json"));
    fs::write(&path, "{broken").unwrap();

    let loaded = load_session_file(Some(&path)).unwrap();
    assert!(loaded.is_none());
    assert!(!path.exists());
}

#[test]
fn remote_preset_can_be_turned_into_patch() {
    let preset = flatten_ata_devices(fixture_list_devices())
        .remove(0)
        .presets
        .remove(1);
    let patch = preset.as_patch();

    assert_eq!(patch.power, Some(true));
    assert_eq!(patch.operation_mode.as_deref(), Some("fan_only"));
    assert_eq!(patch.target_temperature, Some(26.0));
    assert_eq!(patch.fan_speed, Some(2));
}

#[test]
fn prepare_command_borrows_state_without_consuming_it() {
    let state = sample_state();
    let patch = AtaPatch {
        target_temperature: Some(24.4),
        ..AtaPatch::default()
    };
    let prepared = state.prepare_command(&patch).unwrap();

    assert_eq!(prepared.payload["SetTemperature"], json!(24.5));
    assert_eq!(state.target_temperature(), Some(26.0));
}

#[test]
fn remote_preset_save_request_uses_live_state_shape() {
    let preset = flatten_ata_devices(fixture_list_devices())
        .remove(0)
        .presets
        .remove(2);
    let state = fixture_state();
    let request = preset.to_save_request("Temp preset", &state).unwrap();

    assert_eq!(request.device_id, 200001);
    assert_eq!(request.number, 3);
    assert_eq!(request.number_as_string, "3");
    assert_eq!(request.number_description, "Temp preset");
    assert_eq!(request.preset_request.operation_mode, 7);
    assert_eq!(request.preset_request.vane_horizontal, 5);
    assert_eq!(request.preset_request.vane_vertical, 1);
    assert_eq!(request.preset_request.fan_speed, 3);
}

#[test]
fn remote_preset_form_fields_use_official_shape() {
    let request = RemotePresetSaveRequest {
        device_id: 456,
        number: 3,
        number_as_string: "3".to_string(),
        number_description: "Cooling".to_string(),
        preset_request: RemotePresetRequest {
            power: true,
            set_temperature: 27.0,
            operation_mode: 3,
            vane_horizontal: 5,
            vane_vertical: 1,
            fan_speed: 1,
        },
    };
    let form = request.to_form_fields();

    assert!(form.contains(&("DeviceId".to_string(), "456".to_string())));
    assert!(form.contains(&("Number".to_string(), "3".to_string())));
    assert!(form.contains(&("PresetRequest.OperationMode".to_string(), "3".to_string())));
    assert!(form.contains(&("PresetRequest.FanSpeed".to_string(), "1".to_string())));
    assert!(!form.iter().any(|(key, _)| key == "Configuration"));
}

#[test]
fn remote_preset_wire_payload_matches_official_request_shape() {
    let request = RemotePresetSaveRequest {
        device_id: 456,
        number: 3,
        number_as_string: "3".to_string(),
        number_description: "Cooling".to_string(),
        preset_request: RemotePresetRequest {
            power: true,
            set_temperature: 27.0,
            operation_mode: 3,
            vane_horizontal: 5,
            vane_vertical: 1,
            fan_speed: 1,
        },
    };
    let payload = request.to_wire_payload();

    assert_eq!(payload["DeviceId"], json!(456));
    assert_eq!(payload["Number"], json!("3"));
    assert_eq!(payload["NumberDescription"], json!("Cooling"));
    assert_eq!(payload["PresetRequest"]["Power"], json!(true));
    assert_eq!(payload["PresetRequest"]["OperationMode"], json!(3));
    assert_eq!(payload["PresetRequest"]["VaneHorizontal"], json!(5));
    assert_eq!(payload["PresetRequest"]["VaneVertical"], json!(1));
    assert_eq!(payload["PresetRequest"]["FanSpeed"], json!(1));
}

#[test]
fn preset_patch_prepares_full_effective_flags() {
    let preset = flatten_ata_devices(fixture_list_devices())
        .remove(0)
        .presets
        .remove(2);
    let state = fixture_state();
    let prepared = state.prepare_command(&preset.as_patch()).unwrap();

    assert_eq!(prepared.flags, EFFECTIVE_FLAG_PRESET);
}

#[test]
fn clear_session_cache_removes_cached_file() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("melcloud-session-clear-{unique}.json"));
    fs::write(&path, "{}").unwrap();

    let client = MelcloudClient::new(MelcloudConfig {
        session_file: Some(path.clone()),
        ..MelcloudConfig::default()
    })
    .unwrap();

    client.clear_session_cache().unwrap();
    assert!(!path.exists());
}
