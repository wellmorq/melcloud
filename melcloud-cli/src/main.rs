mod args;
mod commands;
mod file_store;
mod json_output;
mod models;
mod patch_input;
mod presets;
mod preview;
mod profile;
mod remote_presets;
mod render;
mod runtime;
mod verify;

#[cfg(test)]
mod tests;

use args::Cli;
use clap::Parser;
use std::process;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut client = match runtime::build_client(&cli).await {
        Ok(client) => client,
        Err(err) => {
            eprintln!("init error: {err}");
            process::exit(1);
        }
    };

    let output = match commands::dispatch(&mut client, &cli).await {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
    };

    match (cli.json, output) {
        (true, Some(text)) | (false, Some(text)) => println!("{text}"),
        _ => {}
    }
}
