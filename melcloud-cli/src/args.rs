use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

const CLI_AFTER_HELP: &str = "\
Examples:
  melcloud auth test
  melcloud devices sync
  melcloud status
  melcloud weather --json
  melcloud set --target-temperature +1 --preview
  melcloud preset capture evening
  melcloud preset set-field evening target_temperature 24.5
  melcloud preset preview evening
  melcloud remote-preset list
  melcloud remote-preset preview 2
  melcloud remote-preset export ventilation
  melcloud remote-preset apply 2
  melcloud remote-preset save --slot 3 --name TestPreset --target-temperature 24
  melcloud preset apply evening

Runtime files:
  session: .\\melcloud-cli\\state\\session.json
  device profile: .\\melcloud-cli\\state\\device.yaml
  remote preset backups: .\\melcloud-cli\\state\\remote-preset-backups\\slot-<n>.json
  presets: .\\melcloud-cli\\presets\\*.yaml

Remote MELCloud presets are fixed slots. `remote-preset save` stores a local rollback backup before overwriting a slot; `remote-preset delete` restores that backup.

Language:
  --language en|ru or a MELCloud numeric language id
  .env key: language=en|ru or a MELCloud numeric language id";

#[derive(Parser)]
#[command(name = "melcloud")]
#[command(about = "MelCloud single-device ATA CLI with YAML presets.")]
#[command(after_long_help = CLI_AFTER_HELP)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,
    #[arg(long, global = true)]
    pub json: bool,
    #[arg(long, global = true)]
    pub email: Option<String>,
    #[arg(long, global = true)]
    pub password: Option<String>,
    #[arg(long, global = true)]
    pub language: Option<String>,
    #[arg(long, global = true)]
    pub session_file: Option<PathBuf>,
    #[arg(long, global = true)]
    pub preset_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pub device_profile: Option<PathBuf>,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    Devices {
        #[command(subcommand)]
        action: DeviceAction,
    },
    Status,
    Weather,
    Set {
        #[command(flatten)]
        patch: PatchArgs,
        #[arg(long)]
        preview: bool,
        #[arg(long, default_value_t = true, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        verify: bool,
    },
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
    RemotePreset {
        #[command(subcommand)]
        action: RemotePresetAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum AuthAction {
    Test,
}

#[derive(Subcommand)]
pub(crate) enum DeviceAction {
    List,
    Sync,
}

#[derive(Args, Clone)]
pub(crate) struct PatchArgs {
    #[arg(long)]
    pub power: Option<bool>,
    #[arg(long)]
    pub mode: Option<String>,
    #[arg(long)]
    pub target_temperature: Option<String>,
    #[arg(long)]
    pub fan_speed: Option<String>,
    #[arg(long)]
    pub vane_horizontal: Option<String>,
    #[arg(long)]
    pub vane_vertical: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum PresetAction {
    List,
    Show {
        name: String,
    },
    Init {
        name: String,
    },
    Capture {
        name: String,
    },
    Preview {
        name: String,
    },
    SetField {
        name: String,
        key: String,
        value: String,
    },
    Apply {
        name: String,
        #[arg(long, default_value_t = true, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        verify: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum RemotePresetAction {
    List,
    Show {
        selector: String,
    },
    Preview {
        selector: String,
    },
    Export {
        selector: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Apply {
        selector: String,
        #[arg(long, default_value_t = true, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        verify: bool,
    },
    Save {
        #[arg(long)]
        slot: i64,
        #[arg(long)]
        name: String,
        #[command(flatten)]
        patch: PatchArgs,
        #[arg(long)]
        preview: bool,
    },
    Delete {
        selector: String,
    },
}
