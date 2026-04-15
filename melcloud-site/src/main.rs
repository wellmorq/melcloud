mod api;
mod cli;
mod config;
mod error;
mod file_store;
mod models;
mod presets;
mod service;
mod site_state;
mod weather;

#[cfg(test)]
mod tests;

use cli::ProcessCliRunner;
use config::load_site_config;
use service::SiteService;
use std::process;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let config = match load_site_config() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("init error: {err}");
            process::exit(1);
        }
    };

    let runner = Arc::new(ProcessCliRunner {
        cli_path: config.cli_path.clone(),
        workdir: config.root_dir.clone(),
        preset_dir: config.preset_dir.clone(),
        session_file: config.cli_session_file.clone(),
        device_profile: config.cli_device_profile.clone(),
        timeout_ms: config.cli_timeout_ms,
    });
    let service = match SiteService::new(config.clone(), runner) {
        Ok(service) => Arc::new(service),
        Err(err) => {
            eprintln!("init error: {err}");
            process::exit(1);
        }
    };

    println!(
        "melcloud-site listening on http://{} ({})",
        config.bind_addr,
        config.ui_language.as_str()
    );
    warp::serve(api::routes(service))
        .run(config.bind_addr)
        .await;
}
