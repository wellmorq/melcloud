use crate::args::Cli;
use crate::models::DeviceProfileLocation;
use melcloud_core::{MelcloudClient, MelcloudConfig, MelcloudError};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_DEVICE_PROFILE_NAME: &str = "device.yaml";
const DEFAULT_RUNTIME_DIR_NAME: &str = "melcloud-cli";
const DEFAULT_PRESET_DIR_NAME: &str = "presets";
const DEFAULT_SESSION_FILE_NAME: &str = "session.json";
const DEFAULT_STATE_DIR_NAME: &str = "state";

pub(crate) async fn build_client(cli: &Cli) -> Result<MelcloudClient, MelcloudError> {
    let state_dir = state_directory();
    let mut env = load_env_file()?;
    let email = cli
        .email
        .clone()
        .or_else(|| std::env::var("MELCLOUD_LOGIN").ok())
        .or_else(|| env.remove("login"));
    let password = cli
        .password
        .clone()
        .or_else(|| std::env::var("MELCLOUD_PASSWORD").ok())
        .or_else(|| env.remove("password"));
    let language_id = resolve_language_id(cli.language.as_deref(), &mut env)?;

    let config = MelcloudConfig {
        email,
        password,
        session_file: Some(session_file_location(cli.session_file.as_ref(), &state_dir)),
        language_id,
        timeout_seconds: 30,
    };
    MelcloudClient::new(config)
}

pub(crate) fn preset_directory(flag_dir: Option<&PathBuf>) -> PathBuf {
    if let Some(path) = flag_dir {
        return path.clone();
    }
    cli_runtime_root().join(DEFAULT_PRESET_DIR_NAME)
}

pub(crate) fn state_directory() -> PathBuf {
    cli_runtime_root().join(DEFAULT_STATE_DIR_NAME)
}

fn cli_runtime_root() -> PathBuf {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if root.join(DEFAULT_RUNTIME_DIR_NAME).is_dir() {
        return root.join(DEFAULT_RUNTIME_DIR_NAME);
    }
    if root
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(DEFAULT_RUNTIME_DIR_NAME))
    {
        return root;
    }
    root.join(DEFAULT_RUNTIME_DIR_NAME)
}

pub(crate) fn device_profile_location(
    flag_path: Option<&PathBuf>,
    state_dir: &Path,
) -> DeviceProfileLocation {
    if let Some(path) = flag_path {
        return DeviceProfileLocation {
            primary: path.clone(),
        };
    }
    if let Ok(path) = std::env::var("MELCLOUD_DEVICE_PROFILE") {
        return DeviceProfileLocation {
            primary: PathBuf::from(path),
        };
    }
    DeviceProfileLocation {
        primary: state_dir.join(DEFAULT_DEVICE_PROFILE_NAME),
    }
}

pub(crate) fn load_env_file() -> Result<HashMap<String, String>, MelcloudError> {
    let path = Path::new(".env");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let mut env = HashMap::new();
    let content = fs::read_to_string(path).map_err(std::io::Error::from)?;
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let mut parts = line.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            env.insert(key.trim().to_string(), parse_env_value(value));
        }
    }
    Ok(env)
}

fn parse_env_value(raw: &str) -> String {
    let value = raw.trim();
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn default_session_file_path(state_dir: &Path) -> PathBuf {
    state_dir.join(DEFAULT_SESSION_FILE_NAME)
}

pub(crate) fn session_file_location(flag_path: Option<&PathBuf>, state_dir: &Path) -> PathBuf {
    if let Some(path) = flag_path {
        return path.clone();
    }
    if let Ok(path) = std::env::var("MELCLOUD_SESSION_FILE") {
        return PathBuf::from(path);
    }
    default_session_file_path(state_dir)
}

pub(crate) fn resolve_language_id(
    cli_value: Option<&str>,
    env: &mut HashMap<String, String>,
) -> Result<i64, MelcloudError> {
    let value = cli_value
        .map(str::to_string)
        .or_else(|| std::env::var("MELCLOUD_LANGUAGE").ok())
        .or_else(|| env.remove("language"));
    value
        .as_deref()
        .map(parse_language_id)
        .transpose()
        .map(|value| value.unwrap_or(0))
}

pub(crate) fn parse_language_id(raw: &str) -> Result<i64, MelcloudError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        if (0..=25).contains(&value) {
            return Ok(value);
        }
        return Err(MelcloudError::Protocol(format!(
            "unsupported MELCloud language id `{trimmed}`"
        )));
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "en" | "english" => Ok(0),
        "ru" | "russian" | "russian federation" => Ok(16),
        _ => Err(MelcloudError::Protocol(format!(
            "unsupported language `{trimmed}`. use en, ru, or a MELCloud numeric language id"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_env_value;

    #[test]
    fn parse_env_value_trims_and_unquotes_values() {
        assert_eq!(parse_env_value(" value "), "value");
        assert_eq!(parse_env_value(" \"quoted value\" "), "quoted value");
        assert_eq!(parse_env_value(" 'quoted value' "), "quoted value");
    }
}
