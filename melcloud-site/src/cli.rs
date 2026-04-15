use crate::error::{Result, SiteError};
use crate::models::{CliDevicesEntry, CliStatusResponse, ConfigPatchRequest};
use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[async_trait]
pub(crate) trait CliRunner: Send + Sync {
    async fn status_json(&self) -> Result<CliStatusResponse>;
    async fn devices_list_json(&self) -> Result<Vec<CliDevicesEntry>>;
    async fn set_config(&self, patch: &ConfigPatchRequest) -> Result<CliStatusResponse>;
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessCliRunner {
    pub cli_path: PathBuf,
    pub workdir: PathBuf,
    pub preset_dir: PathBuf,
    pub session_file: PathBuf,
    pub device_profile: PathBuf,
    pub timeout_ms: u64,
}

#[async_trait]
impl CliRunner for ProcessCliRunner {
    async fn status_json(&self) -> Result<CliStatusResponse> {
        self.run_json(&["status", "--json"]).await
    }

    async fn devices_list_json(&self) -> Result<Vec<CliDevicesEntry>> {
        self.run_json(&["devices", "list", "--json"]).await
    }

    async fn set_config(&self, patch: &ConfigPatchRequest) -> Result<CliStatusResponse> {
        let mut args = vec![
            "set".to_string(),
            "--json".to_string(),
            "--verify=false".to_string(),
        ];
        if let Some(value) = patch.power {
            args.push("--power".to_string());
            args.push(value.to_string());
        }
        if let Some(value) = patch.mode.as_ref() {
            args.push("--mode".to_string());
            args.push(value.clone());
        }
        if let Some(value) = patch.target_temperature {
            args.push("--target-temperature".to_string());
            args.push(format_temperature(value));
        }
        if let Some(value) = patch.fan_speed.as_ref() {
            args.push("--fan-speed".to_string());
            args.push(value.clone());
        }
        if let Some(value) = patch.vane_horizontal.as_ref() {
            args.push("--vane-horizontal".to_string());
            args.push(value.clone());
        }
        if let Some(value) = patch.vane_vertical.as_ref() {
            args.push("--vane-vertical".to_string());
            args.push(value.clone());
        }
        self.run_json_owned(&args).await
    }
}

impl ProcessCliRunner {
    async fn run_json<T>(&self, args: &[&str]) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let args = args.iter().map(|item| item.to_string()).collect::<Vec<_>>();
        self.run_json_owned(&args).await
    }

    async fn run_json_owned<T>(&self, args: &[String]) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let command_args = self.command_args(args);
        let mut command = Command::new(&self.cli_path);
        command
            .args(&command_args)
            .current_dir(&self.workdir)
            .kill_on_drop(true);
        let output = match timeout(Duration::from_millis(self.timeout_ms), command.output()).await {
            Ok(result) => result?,
            Err(_) => {
                return Err(SiteError::CliTimeout(format!(
                    "melcloud-cli {:?} exceeded {}ms timeout",
                    command_args, self.timeout_ms
                )))
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            return Err(SiteError::Cli(format!(
                "melcloud-cli {:?} failed: {}",
                command_args, detail
            )));
        }
        serde_json::from_slice::<T>(&output.stdout).map_err(Into::into)
    }

    fn command_args(&self, args: &[String]) -> Vec<String> {
        let mut command_args = vec![
            "--preset-dir".to_string(),
            self.preset_dir.display().to_string(),
            "--session-file".to_string(),
            self.session_file.display().to_string(),
            "--device-profile".to_string(),
            self.device_profile.display().to_string(),
        ];
        command_args.extend(args.iter().cloned());
        command_args
    }
}

fn format_temperature(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
