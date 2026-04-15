mod auth;
mod devices;
mod preset;
mod read;
mod remote_preset_read;
mod remote_preset_write;
mod set;

use crate::args::{Cli, Command, RemotePresetAction};
use crate::runtime::{device_profile_location, preset_directory, state_directory};
use melcloud_core::{MelcloudClient, MelcloudError};

pub(crate) async fn dispatch(
    client: &mut MelcloudClient,
    cli: &Cli,
) -> Result<Option<String>, MelcloudError> {
    let preset_dir = preset_directory(cli.preset_dir.as_ref());
    let state_dir = state_directory();
    let profile_location = device_profile_location(cli.device_profile.as_ref(), &state_dir);

    match &cli.command {
        Command::Auth { action } => auth::handle(client, cli.json, action).await,
        Command::Devices { action } => {
            devices::handle(client, cli.json, &profile_location, action).await
        }
        Command::Status => read::handle_status(client, cli.json, &profile_location).await,
        Command::Weather => read::handle_weather(client, cli.json, &profile_location).await,
        Command::Set {
            patch,
            preview,
            verify,
        } => {
            set::handle(
                client,
                cli.json,
                &profile_location,
                patch,
                *preview,
                *verify,
            )
            .await
        }
        Command::Preset { action } => {
            preset::handle(client, cli.json, &preset_dir, &profile_location, action).await
        }
        Command::RemotePreset { action } => match action {
            RemotePresetAction::List => {
                remote_preset_read::handle_list(client, cli.json, &profile_location).await
            }
            RemotePresetAction::Show { selector } => {
                remote_preset_read::handle_show(client, cli.json, &profile_location, selector).await
            }
            RemotePresetAction::Preview { selector } => {
                remote_preset_read::handle_preview(client, cli.json, &profile_location, selector)
                    .await
            }
            RemotePresetAction::Export { selector, output } => {
                remote_preset_read::handle_export(
                    client,
                    cli.json,
                    &preset_dir,
                    &profile_location,
                    selector,
                    output.as_ref(),
                )
                .await
            }
            RemotePresetAction::Apply { selector, verify } => {
                remote_preset_write::handle_apply(
                    client,
                    cli.json,
                    &profile_location,
                    selector,
                    *verify,
                )
                .await
            }
            RemotePresetAction::Save {
                slot,
                name,
                patch,
                preview,
            } => {
                remote_preset_write::handle_save(
                    client,
                    cli.json,
                    &profile_location,
                    *slot,
                    name,
                    patch,
                    *preview,
                )
                .await
            }
            RemotePresetAction::Delete { selector } => {
                remote_preset_write::handle_delete(client, cli.json, &profile_location, selector)
                    .await
            }
        },
    }
}
