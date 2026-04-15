use crate::args::PatchArgs;
use crate::models::TemperatureValue;
use melcloud_core::{
    parse_fan_speed, parse_horizontal_vane, parse_mode_input, parse_vertical_vane, AtaPatch,
    AtaState, MelcloudError,
};

pub(crate) fn patch_from_args(
    args: &PatchArgs,
    current: Option<&AtaState>,
) -> Result<AtaPatch, MelcloudError> {
    Ok(AtaPatch {
        power: args.power,
        operation_mode: match args.mode.as_ref() {
            Some(value) => Some(parse_mode_input(value).ok_or_else(|| {
                MelcloudError::Protocol(format!(
                    "invalid mode value: {value}. allowed: off, heat, dry, cool, fan_only, auto"
                ))
            })?),
            None => None,
        },
        target_temperature: match args.target_temperature.as_deref() {
            Some(raw) => Some(resolve_temperature_value(raw, current)?),
            None => None,
        },
        fan_speed: parse_optional_field(args.fan_speed.as_deref(), parse_fan_speed, "fan-speed")?,
        vane_horizontal: parse_optional_field(
            args.vane_horizontal.as_deref(),
            parse_horizontal_vane,
            "vane-horizontal",
        )?,
        vane_vertical: parse_optional_field(
            args.vane_vertical.as_deref(),
            parse_vertical_vane,
            "vane-vertical",
        )?,
    })
}

fn resolve_temperature_value(raw: &str, current: Option<&AtaState>) -> Result<f64, MelcloudError> {
    match parse_temperature_value(raw)? {
        TemperatureValue::Absolute(value) => Ok(value),
        TemperatureValue::Relative(delta) => {
            let current = current
                .and_then(AtaState::target_temperature)
                .ok_or_else(|| {
                    MelcloudError::Protocol(
                        "relative target temperature requires current live state".to_string(),
                    )
                })?;
            Ok(current + delta)
        }
    }
}

fn parse_temperature_value(raw: &str) -> Result<TemperatureValue, MelcloudError> {
    let trimmed = raw.trim();
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        let delta = trimmed
            .parse::<f64>()
            .map_err(|_| invalid_temperature(raw))?;
        return Ok(TemperatureValue::Relative(delta));
    }
    let absolute = trimmed
        .parse::<f64>()
        .map_err(|_| invalid_temperature(raw))?;
    Ok(TemperatureValue::Absolute(absolute))
}

fn parse_optional_field<T, F>(
    input: Option<&str>,
    parse: F,
    field: &'static str,
) -> Result<Option<T>, MelcloudError>
where
    F: Fn(&str) -> Option<T>,
{
    input
        .map(|raw| {
            parse(raw).ok_or_else(|| {
                MelcloudError::Protocol(format!(
                    "invalid {field} value: {raw}. see `melcloud --help` for accepted aliases"
                ))
            })
        })
        .transpose()
}

fn invalid_temperature(raw: &str) -> MelcloudError {
    MelcloudError::Protocol(format!(
        "invalid target temperature value: {raw}. use an absolute number like 24.5 or a relative delta like +1 / -0.5"
    ))
}
