use crate::cli::{CliRunner, ProcessCliRunner};
use crate::config::{SiteConfig, UiLanguage};
use crate::error::{Result, SiteError};
use crate::file_store::backup_path;
use crate::models::{
    CliDevicesEntry, CliStatusResponse, CliWeatherObservation, ConfigPatchRequest, DeviceSummary,
    FixedPresetId, StatusSummary,
};
use crate::presets::{
    ensure_fixed_presets, infer_preset_id, load_preset_config, preset_path,
    write_preset_from_status,
};
use crate::service::capabilities_from_devices;
use crate::service::SiteService;
use crate::site_state::{load_site_state, write_site_state, SiteStateFile};
use crate::weather::{build_weather_cards, build_weather_cards_without_download};
use async_trait::async_trait;
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn sample_status(mode: &str) -> CliStatusResponse {
    CliStatusResponse {
        device: DeviceSummary {
            name: "Hall".to_string(),
            building_id: 211547,
            device_id: 12066563,
            device_type: 0,
        },
        status: StatusSummary {
            power: true,
            operation_mode: Some(mode.to_string()),
            operation_mode_code: Some(7),
            room_temperature: Some(26.0),
            target_temperature: Some(25.5),
            fan_speed: Some("3".to_string()),
            fan_speed_code: Some(3),
            vane_horizontal: Some("5".to_string()),
            vane_horizontal_code: Some(5),
            vane_vertical: Some("1".to_string()),
            vane_vertical_code: Some(1),
            last_communication: None,
            next_communication: None,
            offline: true,
            outdoor_temperature: Some(13.0),
        },
        weather: vec![
            CliWeatherObservation {
                condition_code: Some(119),
                condition_name: Some("Cloudy".to_string()),
                date: Some("2026-04-22T15:00:00".to_string()),
                day: Some(3),
                humidity: Some(45),
                icon: Some("wsymbol_0003_white_cloud".to_string()),
                sunrise: None,
                sunset: None,
                temperature: Some(13.0),
                weather_type: Some(0),
            },
            CliWeatherObservation {
                condition_code: Some(113),
                condition_name: Some("Clear/Sunny".to_string()),
                date: Some("2026-04-23T03:00:00".to_string()),
                day: Some(4),
                humidity: Some(67),
                icon: Some("wsymbol_0008_clear_sky_night".to_string()),
                sunrise: None,
                sunset: None,
                temperature: Some(5.0),
                weather_type: Some(2),
            },
            CliWeatherObservation {
                condition_code: Some(176),
                condition_name: Some("Patchy rain nearby".to_string()),
                date: Some("2026-04-23T15:00:00".to_string()),
                day: Some(4),
                humidity: Some(44),
                icon: Some("wsymbol_0009_light_rain_showers".to_string()),
                sunrise: None,
                sunset: None,
                temperature: Some(14.0),
                weather_type: Some(1),
            },
        ],
        raw: json!({}),
    }
}

fn sample_devices() -> Vec<CliDevicesEntry> {
    vec![CliDevicesEntry {
        device: sample_status("fan_only").device,
        raw: json!({
            "Device": {
                "NumberOfFanSpeeds": 5,
                "HasAutomaticFanSpeed": true,
                "TemperatureIncrement": 0.5,
                "MinTempAutomatic": 16.0,
                "MaxTempAutomatic": 31.0,
                "MinTempCoolDry": 16.0,
                "MaxTempCoolDry": 31.0,
                "MinTempHeat": 10.0,
                "MaxTempHeat": 31.0,
                "WeatherForecast": []
            }
        }),
    }]
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("melcloud-site-tests-{name}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

#[derive(Clone)]
struct FakeRunner {
    calls: Arc<Mutex<Vec<String>>>,
    fail_status: bool,
    fail_set: bool,
    persist_writes: bool,
    status: Arc<Mutex<CliStatusResponse>>,
    sleep_ms: u64,
    in_flight: Arc<AtomicUsize>,
    max_in_flight: Arc<AtomicUsize>,
}

impl Default for FakeRunner {
    fn default() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_status: false,
            fail_set: false,
            persist_writes: true,
            status: Arc::new(Mutex::new(sample_status("cool"))),
            sleep_ms: 0,
            in_flight: Arc::new(AtomicUsize::new(0)),
            max_in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl FakeRunner {
    fn with_status(mode: &str) -> Self {
        Self {
            status: Arc::new(Mutex::new(sample_status(mode))),
            ..Self::default()
        }
    }

    fn with_set_failure() -> Self {
        Self {
            fail_set: true,
            ..Self::default()
        }
    }

    fn with_delay(ms: u64) -> Self {
        Self {
            sleep_ms: ms,
            ..Self::default()
        }
    }

    fn with_unconfirmed_writes() -> Self {
        Self {
            persist_writes: false,
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn max_in_flight(&self) -> usize {
        self.max_in_flight.load(Ordering::SeqCst)
    }

    fn enter(&self, call: String) {
        self.calls.lock().unwrap().push(call);
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        let mut max = self.max_in_flight.load(Ordering::SeqCst);
        while now > max {
            match self
                .max_in_flight
                .compare_exchange(max, now, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break,
                Err(next) => max = next,
            }
        }
        if self.sleep_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.sleep_ms));
        }
    }

    fn leave(&self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl CliRunner for FakeRunner {
    async fn status_json(&self) -> Result<CliStatusResponse> {
        self.enter("status".to_string());
        let result = if self.fail_status {
            Err(SiteError::Cli("fake status failure".to_string()))
        } else {
            Ok(self.status.lock().unwrap().clone())
        };
        self.leave();
        result
    }

    async fn devices_list_json(&self) -> Result<Vec<CliDevicesEntry>> {
        self.enter("devices".to_string());
        let result = Ok(sample_devices());
        self.leave();
        result
    }

    async fn set_config(&self, patch: &ConfigPatchRequest) -> Result<CliStatusResponse> {
        self.enter(format!("set:{}", serde_json::to_string(patch).unwrap()));
        let result = if self.fail_set {
            Err(SiteError::Cli("fake set failure".to_string()))
        } else {
            let mut status = sample_status(patch.mode.as_deref().unwrap_or("cool"));
            if let Some(value) = patch.power {
                status.status.power = value;
            }
            if let Some(value) = patch.target_temperature {
                status.status.target_temperature = Some(value);
            }
            if let Some(value) = patch.fan_speed.as_ref() {
                status.status.fan_speed = Some(value.clone());
            }
            if let Some(value) = patch.vane_horizontal.as_ref() {
                status.status.vane_horizontal = Some(value.clone());
            }
            if let Some(value) = patch.vane_vertical.as_ref() {
                status.status.vane_vertical = Some(value.clone());
            }
            if self.persist_writes {
                *self.status.lock().unwrap() = status.clone();
            }
            Ok(status)
        };
        self.leave();
        result
    }
}

fn test_site_config(root: PathBuf) -> SiteConfig {
    let site_dir = root.join("melcloud-site");
    let cli_dir = root.join("melcloud-cli");
    let preset_dir = cli_dir.join("presets");
    let state_dir = site_dir.join("state");
    let cache_dir = site_dir.join("cache");
    let cli_state_dir = cli_dir.join("state");
    SiteConfig {
        root_dir: root.clone(),
        bind_addr: "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        ui_language: UiLanguage::En,
        commit_debounce_ms: 3_000,
        cli_timeout_ms: 90_000,
        weather_icon_timeout_ms: 1_500,
        cli_path: root.join("fake-cli"),
        preset_dir: preset_dir.clone(),
        cli_session_file: cli_state_dir.join("session.json"),
        cli_device_profile: cli_state_dir.join("device.yaml"),
        site_state_path: state_dir.join("site-state.json"),
        public_dir: root.clone(),
        asset_dir: root,
        weather_icon_cache_dir: cache_dir.join("weather-icons"),
    }
}

fn status_json() -> String {
    serde_json::to_string(&sample_status("cool")).unwrap()
}

fn write_fake_cli(root: &std::path::Path, args_file: &std::path::Path, body: &str) -> PathBuf {
    if cfg!(windows) {
        let path = root.join("fake-cli.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n:args\r\nif \"%~1\"==\"\" goto run\r\n>>\"{}\" echo %~1\r\nshift\r\ngoto args\r\n:run\r\n{}\r\n",
                args_file.display(),
                body
            ),
        )
        .unwrap();
        path
    } else {
        let path = root.join("fake-cli");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n: > '{}'\nfor arg do printf '%s\\n' \"$arg\" >> '{}'; done\n{}\n",
                args_file.display(),
                args_file.display(),
                body
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
        path
    }
}

fn echo_json_script(json: &str) -> String {
    if cfg!(windows) {
        format!("echo {json}")
    } else {
        format!("cat <<'JSON'\n{json}\nJSON")
    }
}

fn fail_script(message: &str) -> String {
    if cfg!(windows) {
        format!("echo {message} 1>&2\r\nexit /b 7")
    } else {
        format!("echo {message} >&2\nexit 7")
    }
}

fn sleep_script() -> String {
    if cfg!(windows) {
        "ping 127.0.0.1 -n 6 >nul\r\necho {}".to_string()
    } else {
        "sleep 5\necho '{}'".to_string()
    }
}

fn read_fake_cli_args(args_file: &std::path::Path) -> String {
    fs::read_to_string(args_file)
        .unwrap()
        .lines()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn ensure_fixed_presets_creates_all_site_presets() {
    let dir = temp_dir("presets");
    ensure_fixed_presets(&dir).unwrap();
    for preset_id in FixedPresetId::ALL {
        assert!(preset_path(&dir, preset_id).exists());
    }
}

#[test]
fn infer_preset_id_maps_supported_modes() {
    assert_eq!(
        infer_preset_id(&sample_status("heat").status),
        Some(FixedPresetId::SiteHeat)
    );
    assert_eq!(
        infer_preset_id(&sample_status("fan_only").status),
        Some(FixedPresetId::SiteFan)
    );
    assert_eq!(
        infer_preset_id(&sample_status("cool").status),
        Some(FixedPresetId::SiteCool)
    );
    assert_eq!(
        infer_preset_id(&sample_status("dry").status),
        Some(FixedPresetId::SiteDry)
    );
}

#[test]
fn write_preset_from_status_persists_full_snapshot() {
    let dir = temp_dir("capture");
    ensure_fixed_presets(&dir).unwrap();
    write_preset_from_status(&dir, FixedPresetId::SiteCool, &sample_status("cool").status).unwrap();
    let raw = fs::read_to_string(preset_path(&dir, FixedPresetId::SiteCool)).unwrap();
    assert!(raw.contains("target_temperature"));
    assert!(raw.contains("fan_speed"));
}

#[test]
fn load_preset_config_reads_saved_fields() {
    let dir = temp_dir("load-config");
    ensure_fixed_presets(&dir).unwrap();
    write_preset_from_status(&dir, FixedPresetId::SiteCool, &sample_status("cool").status).unwrap();
    let config = load_preset_config(&dir, FixedPresetId::SiteCool).unwrap();
    assert_eq!(config.mode.as_deref(), Some("cool"));
    assert_eq!(config.target_temperature, Some(25.5));
    assert_eq!(config.fan_speed.as_deref(), Some("3"));
}

#[test]
fn corrupt_preset_loads_backup_copy() {
    let dir = temp_dir("preset-backup");
    ensure_fixed_presets(&dir).unwrap();
    let path = preset_path(&dir, FixedPresetId::SiteCool);
    write_preset_from_status(&dir, FixedPresetId::SiteCool, &sample_status("cool").status).unwrap();
    write_preset_from_status(&dir, FixedPresetId::SiteCool, &sample_status("heat").status).unwrap();
    fs::write(path, "{broken").unwrap();

    let config = load_preset_config(&dir, FixedPresetId::SiteCool).unwrap();

    assert_eq!(config.mode.as_deref(), Some("cool"));
}

#[test]
fn missing_preset_is_restored_from_backup_during_ensure() {
    let dir = temp_dir("preset-restore");
    ensure_fixed_presets(&dir).unwrap();
    let path = preset_path(&dir, FixedPresetId::SiteFan);
    write_preset_from_status(
        &dir,
        FixedPresetId::SiteFan,
        &sample_status("fan_only").status,
    )
    .unwrap();
    fs::copy(&path, backup_path(&path)).unwrap();
    fs::remove_file(&path).unwrap();

    ensure_fixed_presets(&dir).unwrap();

    let config = load_preset_config(&dir, FixedPresetId::SiteFan).unwrap();
    assert_eq!(config.mode.as_deref(), Some("fan_only"));
}

#[test]
fn build_weather_cards_keeps_four_slots() {
    let cards = build_weather_cards_without_download(&sample_status("fan_only"), &sample_devices());
    assert_eq!(cards.len(), 4);
    assert!(!cards[0].placeholder);
    assert_eq!(cards[0].period_key, "now");
    assert_eq!(cards[1].period_key, "night");
    assert_eq!(cards[2].period_key, "day");
    assert!(cards[3].placeholder);
    assert_eq!(cards[0].icon, "weather_cloud");
    assert_eq!(cards[1].icon, "weather_moon");
    assert_eq!(cards[2].icon, "weather_cloud");
}

#[tokio::test]
async fn build_weather_cards_returns_fallback_without_waiting_for_cache() {
    let cache_dir = temp_dir("weather-cache");

    let cards = build_weather_cards(&sample_status("fan_only"), &sample_devices(), &cache_dir, 0)
        .await
        .unwrap();

    assert_eq!(cards[0].icon, "weather_cloud");
    assert_eq!(cards[1].icon, "weather_moon");
    assert_eq!(cards[2].icon, "weather_cloud");
    assert!(fs::read_dir(cache_dir).unwrap().next().is_none());
}

#[test]
fn capabilities_are_extracted_from_discovery_payload() {
    let caps = capabilities_from_devices(&sample_devices()).unwrap();
    assert_eq!(caps.fan_speeds, vec![1, 2, 3, 4, 5]);
    assert!(caps.supports_fan_auto);
    assert_eq!(caps.min_temp_heat, Some(10.0));
}

#[test]
fn site_state_roundtrip_preserves_active_preset() {
    let path = temp_dir("state").join("site-state.json");
    write_site_state(
        &path,
        &SiteStateFile {
            active_preset_id: Some(FixedPresetId::SiteFan),
        },
    )
    .unwrap();
    let state = load_site_state(&path).unwrap();
    assert_eq!(state.active_preset_id, Some(FixedPresetId::SiteFan));
}

#[test]
fn corrupt_site_state_loads_backup_copy() {
    let path = temp_dir("state-backup").join("site-state.json");
    write_site_state(
        &path,
        &SiteStateFile {
            active_preset_id: Some(FixedPresetId::SiteFan),
        },
    )
    .unwrap();
    write_site_state(
        &path,
        &SiteStateFile {
            active_preset_id: Some(FixedPresetId::SiteCool),
        },
    )
    .unwrap();
    fs::write(&path, "{broken").unwrap();

    let state = load_site_state(&path).unwrap();

    assert_eq!(state.active_preset_id, Some(FixedPresetId::SiteFan));
}

#[test]
fn failed_state_write_keeps_last_good_file() {
    let path = temp_dir("state-write-failure").join("site-state.json");
    write_site_state(
        &path,
        &SiteStateFile {
            active_preset_id: Some(FixedPresetId::SiteFan),
        },
    )
    .unwrap();
    let mut temp_raw: OsString = path.as_os_str().to_owned();
    temp_raw.push(".tmp");
    fs::create_dir(PathBuf::from(temp_raw)).unwrap();

    let result = write_site_state(
        &path,
        &SiteStateFile {
            active_preset_id: Some(FixedPresetId::SiteCool),
        },
    );

    assert!(result.is_err());
    let state = load_site_state(&path).unwrap();
    assert_eq!(state.active_preset_id, Some(FixedPresetId::SiteFan));
}

#[tokio::test]
async fn service_patch_persists_returned_live_status_after_success() {
    let root = temp_dir("service-patch-success");
    let config = test_site_config(root);
    let runner = FakeRunner::default();
    let service = SiteService::new(config.clone(), Arc::new(runner.clone())).unwrap();
    let patch = ConfigPatchRequest {
        target_temperature: Some(22.5),
        fan_speed: Some("4".to_string()),
        vane_horizontal: Some("2".to_string()),
        ..Default::default()
    };

    let snapshot = service
        .patch_active_preset(FixedPresetId::SiteCool, &patch)
        .await
        .unwrap();
    let saved = load_preset_config(&config.preset_dir, FixedPresetId::SiteCool).unwrap();

    assert_eq!(snapshot.active_preset_id, Some(FixedPresetId::SiteCool));
    assert_eq!(saved.target_temperature, Some(22.5));
    assert_eq!(saved.fan_speed.as_deref(), Some("4"));
    assert_eq!(saved.vane_horizontal.as_deref(), Some("2"));
    assert!(runner.calls().iter().any(|call| call.starts_with("set:")));
}

#[tokio::test]
async fn service_patch_accepts_full_preset_like_config() {
    let root = temp_dir("service-patch-full-preset");
    let config = test_site_config(root);
    let runner = FakeRunner::default();
    let service = SiteService::new(config.clone(), Arc::new(runner.clone())).unwrap();
    let patch = ConfigPatchRequest {
        power: Some(true),
        mode: Some("cool".to_string()),
        target_temperature: Some(20.5),
        fan_speed: Some("4".to_string()),
        vane_horizontal: Some("5".to_string()),
        vane_vertical: Some("1".to_string()),
    };

    let snapshot = service
        .patch_active_preset(FixedPresetId::SiteCool, &patch)
        .await
        .unwrap();
    let saved = load_preset_config(&config.preset_dir, FixedPresetId::SiteCool).unwrap();
    let calls = runner.calls();

    assert_eq!(snapshot.active_preset_id, Some(FixedPresetId::SiteCool));
    assert_eq!(saved.mode.as_deref(), Some("cool"));
    assert_eq!(saved.target_temperature, Some(20.5));
    assert_eq!(saved.fan_speed.as_deref(), Some("4"));
    assert!(calls.iter().any(|call| call.contains("\"mode\":\"cool\"")));
}

#[tokio::test]
async fn service_patch_readback_mismatch_does_not_overwrite_site_preset() {
    let root = temp_dir("service-patch-mismatch");
    let config = test_site_config(root);
    let runner = FakeRunner::with_unconfirmed_writes();
    let service = SiteService::new(config.clone(), Arc::new(runner)).unwrap();
    let before =
        fs::read_to_string(preset_path(&config.preset_dir, FixedPresetId::SiteCool)).unwrap();
    let patch = ConfigPatchRequest {
        target_temperature: Some(22.5),
        ..Default::default()
    };

    let error = service
        .patch_active_preset(FixedPresetId::SiteCool, &patch)
        .await
        .unwrap_err()
        .to_string();
    let after =
        fs::read_to_string(preset_path(&config.preset_dir, FixedPresetId::SiteCool)).unwrap();

    assert!(error.contains("read-back did not confirm"));
    assert_eq!(before, after);
}

#[tokio::test]
async fn service_apply_preset_uses_set_config_path() {
    let root = temp_dir("service-apply-success");
    let config = test_site_config(root);
    let runner = FakeRunner::default();
    let service = SiteService::new(config.clone(), Arc::new(runner.clone())).unwrap();

    let snapshot = service.apply_preset(FixedPresetId::SiteFan).await.unwrap();
    let saved = load_preset_config(&config.preset_dir, FixedPresetId::SiteFan).unwrap();
    let calls = runner.calls();

    assert_eq!(snapshot.active_preset_id, Some(FixedPresetId::SiteFan));
    assert!(calls.iter().any(|call| call.starts_with("set:")));
    assert_eq!(saved.mode.as_deref(), Some("fan_only"));
}

#[tokio::test]
async fn snapshot_ignores_stale_selected_preset_when_live_mode_differs() {
    let root = temp_dir("snapshot-stale-selected");
    let config = test_site_config(root);
    write_site_state(
        &config.site_state_path,
        &SiteStateFile {
            active_preset_id: Some(FixedPresetId::SiteFan),
        },
    )
    .unwrap();
    let service = SiteService::new(
        config,
        Arc::new(FakeRunner::with_status(FixedPresetId::SiteHeat.mode())),
    )
    .unwrap();

    let snapshot = service.snapshot(false).await.unwrap();

    assert_eq!(snapshot.active_preset_id, Some(FixedPresetId::SiteHeat));
}

#[tokio::test]
async fn service_apply_preset_readback_mismatch_does_not_overwrite_site_preset() {
    let root = temp_dir("service-apply-mismatch");
    let config = test_site_config(root);
    let runner = FakeRunner::with_unconfirmed_writes();
    let service = SiteService::new(config.clone(), Arc::new(runner)).unwrap();
    let before =
        fs::read_to_string(preset_path(&config.preset_dir, FixedPresetId::SiteHeat)).unwrap();

    let error = service
        .apply_preset(FixedPresetId::SiteHeat)
        .await
        .unwrap_err()
        .to_string();
    let after =
        fs::read_to_string(preset_path(&config.preset_dir, FixedPresetId::SiteHeat)).unwrap();

    assert!(error.contains("read-back did not confirm"));
    assert_eq!(before, after);
}

#[tokio::test]
async fn service_patch_failure_does_not_overwrite_site_preset() {
    let root = temp_dir("service-patch-failure");
    let config = test_site_config(root);
    let service =
        SiteService::new(config.clone(), Arc::new(FakeRunner::with_set_failure())).unwrap();
    let before =
        fs::read_to_string(preset_path(&config.preset_dir, FixedPresetId::SiteCool)).unwrap();
    let patch = ConfigPatchRequest {
        target_temperature: Some(18.0),
        ..Default::default()
    };

    let result = service
        .patch_active_preset(FixedPresetId::SiteCool, &patch)
        .await;
    let after =
        fs::read_to_string(preset_path(&config.preset_dir, FixedPresetId::SiteCool)).unwrap();

    assert!(result.is_err());
    assert_eq!(before, after);
}

#[tokio::test]
async fn service_operations_are_serialized_around_cli_runner() {
    let root = temp_dir("service-serialized");
    let config = test_site_config(root);
    let runner = FakeRunner::with_delay(20);
    let service = Arc::new(SiteService::new(config, Arc::new(runner.clone())).unwrap());
    let patch = ConfigPatchRequest {
        power: Some(false),
        ..Default::default()
    };

    let read_service = service.clone();
    let write_service = service.clone();
    let (read_result, write_result) = tokio::join!(
        async move { read_service.snapshot(false).await },
        async move {
            write_service
                .patch_active_preset(FixedPresetId::SiteCool, &patch)
                .await
        }
    );

    read_result.unwrap();
    write_result.unwrap();
    assert_eq!(runner.max_in_flight(), 1);
}

#[tokio::test]
async fn concurrent_snapshot_and_write_return_coherent_preset_config() {
    let root = temp_dir("service-coherent-snapshot");
    let config = test_site_config(root);
    let runner = FakeRunner::with_delay(10);
    let service = Arc::new(SiteService::new(config.clone(), Arc::new(runner)).unwrap());
    write_preset_from_status(
        &config.preset_dir,
        FixedPresetId::SiteCool,
        &sample_status("cool").status,
    )
    .unwrap();
    let patch = ConfigPatchRequest {
        target_temperature: Some(22.5),
        ..Default::default()
    };

    let read_service = service.clone();
    let write_service = service.clone();
    let (read_result, write_result) = tokio::join!(
        async move { read_service.snapshot(false).await },
        async move {
            write_service
                .patch_active_preset(FixedPresetId::SiteCool, &patch)
                .await
        }
    );

    let snapshot = read_result.unwrap();
    write_result.unwrap();
    let cool_preset = snapshot
        .presets
        .iter()
        .find(|preset| preset.id == FixedPresetId::SiteCool)
        .unwrap();
    assert_eq!(
        snapshot.live_status.target_temperature,
        cool_preset.config.target_temperature
    );
}

#[tokio::test]
async fn snapshot_read_does_not_persist_inferred_active_preset_state() {
    let root = temp_dir("snapshot-state-side-effect");
    let config = test_site_config(root);
    let service = SiteService::new(config.clone(), Arc::new(FakeRunner::default())).unwrap();

    let snapshot = service.snapshot(false).await.unwrap();

    assert_eq!(snapshot.active_preset_id, Some(FixedPresetId::SiteCool));
    assert!(!config.site_state_path.exists());
}

#[tokio::test]
#[ignore = "spawns a fake CLI subprocess; run with cargo test -p melcloud-site -- --ignored"]
async fn process_cli_runner_set_config_uses_expected_cli_args() {
    let root = temp_dir("process-runner-set");
    let args_file = root.join("args.txt");
    let cli_path = write_fake_cli(&root, &args_file, &echo_json_script(&status_json()));
    let runner = ProcessCliRunner {
        cli_path,
        workdir: root.clone(),
        preset_dir: root.join("melcloud-cli").join("presets"),
        session_file: root.join("melcloud-cli").join("state").join("session.json"),
        device_profile: root.join("melcloud-cli").join("state").join("device.yaml"),
        timeout_ms: 5_000,
    };
    let patch = ConfigPatchRequest {
        power: Some(false),
        mode: Some("heat".to_string()),
        target_temperature: Some(22.5),
        fan_speed: Some("auto".to_string()),
        vane_horizontal: Some("5".to_string()),
        vane_vertical: Some("1".to_string()),
    };

    runner.set_config(&patch).await.unwrap();
    let args = read_fake_cli_args(&args_file);

    assert_eq!(
        args,
        format!(
            "--preset-dir {} --session-file {} --device-profile {} set --json --verify=false --power false --mode heat --target-temperature 22.5 --fan-speed auto --vane-horizontal 5 --vane-vertical 1",
            root.join("melcloud-cli").join("presets").display(),
            root.join("melcloud-cli").join("state").join("session.json").display(),
            root.join("melcloud-cli").join("state").join("device.yaml").display()
        )
    );
}

#[tokio::test]
#[ignore = "spawns a fake CLI subprocess; run with cargo test -p melcloud-site -- --ignored"]
async fn process_cli_runner_nonzero_exit_preserves_cli_stderr() {
    let root = temp_dir("process-runner-error");
    let args_file = root.join("args.txt");
    let cli_path = write_fake_cli(&root, &args_file, &fail_script("fake cli failed"));
    let runner = ProcessCliRunner {
        cli_path,
        workdir: root.clone(),
        preset_dir: root.join("melcloud-cli").join("presets"),
        session_file: root.join("melcloud-cli").join("state").join("session.json"),
        device_profile: root.join("melcloud-cli").join("state").join("device.yaml"),
        timeout_ms: 5_000,
    };

    let error = runner.status_json().await.unwrap_err().to_string();

    assert!(error.contains("\"status\", \"--json\"] failed"));
    assert!(error.contains("fake cli failed"));
}

#[tokio::test]
#[ignore = "spawns a fake CLI subprocess; run with cargo test -p melcloud-site -- --ignored"]
async fn process_cli_runner_timeout_returns_typed_error() {
    let root = temp_dir("process-runner-timeout");
    let args_file = root.join("args.txt");
    let cli_path = write_fake_cli(&root, &args_file, &sleep_script());
    let runner = ProcessCliRunner {
        cli_path,
        workdir: root.clone(),
        preset_dir: root.join("melcloud-cli").join("presets"),
        session_file: root.join("melcloud-cli").join("state").join("session.json"),
        device_profile: root.join("melcloud-cli").join("state").join("device.yaml"),
        timeout_ms: 50,
    };
    let started = Instant::now();

    let error = runner.status_json().await.unwrap_err();

    assert!(matches!(error, SiteError::CliTimeout(_)));
    assert!(started.elapsed() < Duration::from_secs(2));
}
