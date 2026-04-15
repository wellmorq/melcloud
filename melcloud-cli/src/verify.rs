use crate::preview::compact_remote_preset_signature;
use melcloud_core::{AtaPatch, AtaState, BoundDevice, MelcloudClient, MelcloudError, RemotePreset};
use std::time::Duration;
use tokio::time::delay_for;

const VERIFY_ATTEMPTS: usize = 6;
const VERIFY_DELAY: Duration = Duration::from_secs(5);

pub(crate) async fn wait_for_expected_config(
    client: &mut MelcloudClient,
    device: &BoundDevice,
    expected: &AtaState,
) -> Result<AtaState, MelcloudError> {
    let mut last_seen = None;
    for attempt in 0..VERIFY_ATTEMPTS {
        let state = client.get_device_status(device).await?;
        if config_matches_expected(&state, expected) {
            return Ok(state);
        }
        last_seen = Some(state);
        if attempt + 1 < VERIFY_ATTEMPTS {
            delay_for(VERIFY_DELAY).await;
        }
    }

    let last_seen = last_seen.ok_or_else(|| {
        MelcloudError::Protocol("verification did not observe any device status".to_string())
    })?;
    Err(MelcloudError::Protocol(format!(
        "device did not converge to expected config after verification window. expected={}, last_seen={}",
        compact_config_signature(expected),
        compact_config_signature(&last_seen)
    )))
}

pub(crate) async fn wait_for_remote_preset(
    client: &mut MelcloudClient,
    slot: i64,
    expected_name: &str,
    expected_patch: &AtaPatch,
) -> Result<RemotePreset, MelcloudError> {
    let mut last_seen = None;
    for attempt in 0..VERIFY_ATTEMPTS {
        let presets = client.list_remote_presets().await?;
        if let Some(preset) = presets
            .iter()
            .find(|preset| preset.number == Some(slot))
            .cloned()
        {
            if remote_preset_matches_expected(&preset, expected_name, expected_patch) {
                return Ok(preset);
            }
            last_seen = Some(preset);
        }
        if attempt + 1 < VERIFY_ATTEMPTS {
            delay_for(VERIFY_DELAY).await;
        }
    }

    let last_seen = last_seen.ok_or_else(|| {
        MelcloudError::Protocol(format!(
            "verification did not observe remote preset slot #{slot}"
        ))
    })?;
    Err(MelcloudError::Protocol(format!(
        "remote preset slot #{slot} did not converge to expected state after verification window. expected=name={}, patch={}; last_seen={}",
        expected_name,
        compact_patch_signature(expected_patch),
        compact_remote_preset_signature(&last_seen)
    )))
}

fn config_matches_expected(actual: &AtaState, expected: &AtaState) -> bool {
    actual.power() == expected.power()
        && actual.operation_mode_raw() == expected.operation_mode_raw()
        && float_option_matches(actual.target_temperature(), expected.target_temperature())
        && actual.fan_speed_raw() == expected.fan_speed_raw()
        && actual.vane_horizontal_raw() == expected.vane_horizontal_raw()
        && actual.vane_vertical_raw() == expected.vane_vertical_raw()
}

fn remote_preset_matches_expected(
    preset: &RemotePreset,
    expected_name: &str,
    expected_patch: &AtaPatch,
) -> bool {
    let actual = preset.as_patch();
    preset.name == expected_name
        && actual.power == expected_patch.power
        && actual.operation_mode == expected_patch.operation_mode
        && float_option_matches(actual.target_temperature, expected_patch.target_temperature)
        && actual.fan_speed == expected_patch.fan_speed
        && actual.vane_horizontal == expected_patch.vane_horizontal
        && actual.vane_vertical == expected_patch.vane_vertical
}

fn float_option_matches(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => (left - right).abs() < 0.01,
        (None, None) => true,
        _ => false,
    }
}

fn compact_config_signature(state: &AtaState) -> String {
    format!(
        "power={:?}, mode={:?}, temp={:?}, fan={:?}, vane_h={:?}, vane_v={:?}",
        state.power(),
        state.operation_mode_raw(),
        state.target_temperature(),
        state.fan_speed_raw(),
        state.vane_horizontal_raw(),
        state.vane_vertical_raw()
    )
}

fn compact_patch_signature(patch: &AtaPatch) -> String {
    format!(
        "power={:?}, mode={:?}, temp={:?}, fan={:?}, vane_h={:?}, vane_v={:?}",
        patch.power,
        patch.operation_mode,
        patch.target_temperature,
        patch.fan_speed,
        patch.vane_horizontal,
        patch.vane_vertical
    )
}
