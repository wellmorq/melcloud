use crate::error::{Result, SiteError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8787";
const DEFAULT_COMMIT_DEBOUNCE_MS: u64 = 3_000;
const DEFAULT_CLI_TIMEOUT_MS: u64 = 90_000;
const DEFAULT_WEATHER_ICON_TIMEOUT_MS: u64 = 1_500;
const SITE_CONFIG_FILE_NAME: &str = "melcloud-site.yaml";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UiLanguage {
    En,
    Ru,
}

impl UiLanguage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SiteConfig {
    pub root_dir: PathBuf,
    pub bind_addr: SocketAddr,
    pub ui_language: UiLanguage,
    pub commit_debounce_ms: u64,
    pub cli_timeout_ms: u64,
    pub weather_icon_timeout_ms: u64,
    pub cli_path: PathBuf,
    pub preset_dir: PathBuf,
    pub cli_session_file: PathBuf,
    pub cli_device_profile: PathBuf,
    pub site_state_path: PathBuf,
    pub public_dir: PathBuf,
    pub asset_dir: PathBuf,
    pub weather_icon_cache_dir: PathBuf,
}

pub(crate) fn load_site_config() -> Result<SiteConfig> {
    let root_dir = discover_root_dir()?;
    let site_dir = root_dir.join("melcloud-site");
    let cli_dir = root_dir.join("melcloud-cli");
    let file_config = load_or_create_site_file(&site_dir.join(SITE_CONFIG_FILE_NAME))?;
    let env = load_env_file(&root_dir.join(".env"))?;
    let bind_addr = resolve_bind_addr(&env, &file_config)?;
    let state_dir = site_dir.join("state");
    let cache_dir = site_dir.join("cache");
    let cli_preset_dir = cli_dir.join("presets");
    let cli_state_dir = cli_dir.join("state");
    let preset_dir = cli_preset_dir.clone();
    let cli_session_file = cli_state_dir.join("session.json");
    let cli_device_profile = cli_state_dir.join("device.yaml");
    let public_dir = site_dir.join("public");
    let asset_dir = site_dir.join("site-assets");
    let weather_icon_cache_dir = cache_dir.join("weather-icons");
    let site_state_path = state_dir.join("site-state.json");
    let cli_path = resolve_cli_path(&root_dir, &env)?;
    if !public_dir.exists() {
        return Err(SiteError::Protocol(format!(
            "site public directory is missing: {}",
            public_dir.display()
        )));
    }
    if !asset_dir.exists() {
        return Err(SiteError::Protocol(format!(
            "site asset directory is missing: {}",
            asset_dir.display()
        )));
    }
    fs::create_dir_all(&preset_dir)?;
    fs::create_dir_all(&state_dir)?;
    fs::create_dir_all(&cli_state_dir)?;
    fs::create_dir_all(&weather_icon_cache_dir)?;
    Ok(SiteConfig {
        root_dir,
        bind_addr,
        ui_language: file_config.ui_language,
        commit_debounce_ms: file_config.commit_debounce_ms,
        cli_timeout_ms: file_config.cli_timeout_ms,
        weather_icon_timeout_ms: file_config.weather_icon_timeout_ms,
        cli_path,
        preset_dir,
        cli_session_file,
        cli_device_profile,
        site_state_path,
        public_dir,
        asset_dir,
        weather_icon_cache_dir,
    })
}

fn discover_root_dir() -> Result<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(std::env::current_dir()?);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.to_path_buf());
            if let Some(parent) = dir.parent() {
                candidates.push(parent.to_path_buf());
            }
        }
    }
    for candidate in candidates {
        for root in candidate.ancestors() {
            if is_workspace_root(root) || is_packaged_runtime_root(root) {
                return Ok(root.to_path_buf());
            }
        }
    }
    Err(SiteError::Protocol(
        "failed to discover workspace root for melcloud-site".to_string(),
    ))
}

fn is_workspace_root(root: &Path) -> bool {
    root.join("melcloud-cli").is_dir() && root.join("melcloud-site").is_dir()
}

fn is_packaged_runtime_root(root: &Path) -> bool {
    default_cli_path(root).is_file()
        && root.join("melcloud-site").join("public").is_dir()
        && root.join("melcloud-site").join("site-assets").is_dir()
}

fn load_env_file(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let mut env = HashMap::new();
    let content = fs::read_to_string(path)?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SiteFileConfig {
    #[serde(default = "default_bind_addr")]
    bind_addr: String,
    #[serde(default = "default_ui_language")]
    ui_language: UiLanguage,
    #[serde(default = "default_commit_debounce_ms")]
    commit_debounce_ms: u64,
    #[serde(default = "default_cli_timeout_ms")]
    cli_timeout_ms: u64,
    #[serde(default = "default_weather_icon_timeout_ms")]
    weather_icon_timeout_ms: u64,
}

fn load_or_create_site_file(path: &Path) -> Result<SiteFileConfig> {
    if path.exists() {
        let raw = fs::read_to_string(path)?;
        return serde_yaml::from_str(&raw).map_err(Into::into);
    }
    let default = SiteFileConfig::default();
    let raw = serde_yaml::to_string(&default)?;
    fs::write(path, raw)?;
    Ok(default)
}

fn resolve_bind_addr(
    env: &HashMap<String, String>,
    file_config: &SiteFileConfig,
) -> Result<SocketAddr> {
    let raw = std::env::var("MELCLOUD_SITE_BIND")
        .ok()
        .or_else(|| env.get("site_bind").cloned())
        .unwrap_or_else(|| file_config.bind_addr.clone());
    raw.parse::<SocketAddr>()
        .map_err(|err| SiteError::Protocol(format!("invalid site_bind value `{raw}`: {err}")))
}

fn resolve_cli_path(root_dir: &Path, env: &HashMap<String, String>) -> Result<PathBuf> {
    let configured = std::env::var("MELCLOUD_CLI_PATH")
        .ok()
        .or_else(|| env.get("cli_path").cloned())
        .map(PathBuf::from);
    let default = default_cli_path(root_dir);
    let path = configured.unwrap_or(default);
    if path.exists() {
        return Ok(path);
    }
    Err(SiteError::Protocol(format!(
        "melcloud CLI executable is missing: {}",
        path.display()
    )))
}

fn default_cli_file_name() -> &'static str {
    if cfg!(windows) {
        "melcloud-cli.exe"
    } else {
        "melcloud-cli"
    }
}

fn default_cli_path(root_dir: &Path) -> PathBuf {
    for path in [
        root_dir.join("bin").join(default_cli_file_name()),
        root_dir
            .join("build")
            .join("bin")
            .join(default_cli_file_name()),
        root_dir
            .join("target")
            .join("debug")
            .join(default_cli_file_name()),
        root_dir
            .join("target")
            .join("release")
            .join(default_cli_file_name()),
        root_dir.join(default_cli_file_name()),
    ] {
        if path.is_file() {
            return path;
        }
    }
    root_dir.join("bin").join(default_cli_file_name())
}

fn default_bind_addr() -> String {
    DEFAULT_BIND_ADDR.to_string()
}

fn default_ui_language() -> UiLanguage {
    UiLanguage::En
}

fn default_commit_debounce_ms() -> u64 {
    DEFAULT_COMMIT_DEBOUNCE_MS
}

fn default_cli_timeout_ms() -> u64 {
    DEFAULT_CLI_TIMEOUT_MS
}

fn default_weather_icon_timeout_ms() -> u64 {
    DEFAULT_WEATHER_ICON_TIMEOUT_MS
}

impl Default for SiteFileConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            ui_language: default_ui_language(),
            commit_debounce_ms: default_commit_debounce_ms(),
            cli_timeout_ms: default_cli_timeout_ms(),
            weather_icon_timeout_ms: default_weather_icon_timeout_ms(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("melcloud-site-config-{name}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn load_or_create_site_file_writes_default_yaml() {
        let dir = temp_dir("default");
        let path = dir.join(SITE_CONFIG_FILE_NAME);
        let config = load_or_create_site_file(&path).unwrap();
        assert_eq!(config.ui_language, UiLanguage::En);
        assert_eq!(config.commit_debounce_ms, DEFAULT_COMMIT_DEBOUNCE_MS);
        assert_eq!(config.cli_timeout_ms, DEFAULT_CLI_TIMEOUT_MS);
        assert_eq!(
            config.weather_icon_timeout_ms,
            DEFAULT_WEATHER_ICON_TIMEOUT_MS
        );
        assert!(path.exists());
    }

    #[test]
    fn load_or_create_site_file_reads_yaml_values() {
        let dir = temp_dir("custom");
        let path = dir.join(SITE_CONFIG_FILE_NAME);
        fs::write(
            &path,
            "bind_addr: 127.0.0.1:9898\nui_language: ru\ncommit_debounce_ms: 3500\ncli_timeout_ms: 45000\nweather_icon_timeout_ms: 750\n",
        )
        .unwrap();
        let config = load_or_create_site_file(&path).unwrap();
        assert_eq!(config.bind_addr, "127.0.0.1:9898");
        assert_eq!(config.ui_language, UiLanguage::Ru);
        assert_eq!(config.commit_debounce_ms, 3_500);
        assert_eq!(config.cli_timeout_ms, 45_000);
        assert_eq!(config.weather_icon_timeout_ms, 750);
    }

    #[test]
    fn parse_env_value_trims_and_unquotes_values() {
        assert_eq!(parse_env_value(" value "), "value");
        assert_eq!(parse_env_value(" \"quoted value\" "), "quoted value");
        assert_eq!(parse_env_value(" 'quoted value' "), "quoted value");
    }

    #[test]
    fn packaged_runtime_root_is_detected_by_binaries_and_site_assets() {
        let dir = temp_dir("runtime-root");
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::write(dir.join("bin").join(default_cli_file_name()), "").unwrap();
        fs::create_dir_all(dir.join("melcloud-site").join("public")).unwrap();
        fs::create_dir_all(dir.join("melcloud-site").join("site-assets")).unwrap();

        assert!(is_packaged_runtime_root(&dir));
    }
}
