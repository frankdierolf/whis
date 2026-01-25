mod app;
mod args;
mod commands;
mod error;
mod hotkey;
mod ipc;
mod service;
mod ui;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    // Run CLI and handle errors with helpful messages
    if let Err(err) = run() {
        error::display_anyhow_error(err);
        std::process::exit(1);
    }
    Ok(())
}

fn run() -> Result<()> {
    let cli = args::Cli::parse();

    // Enable verbose logging if requested
    whis_core::set_verbose(cli.verbose);

    match cli.command {
        Some(args::Commands::Start { autotype, preset }) => commands::start::run(autotype, preset),
        Some(args::Commands::Stop) => commands::stop::run(),
        Some(args::Commands::Restart { autotype, preset }) => {
            commands::restart::run(autotype, preset)
        }
        Some(args::Commands::Status) => commands::status::run(),
        Some(args::Commands::Toggle) => commands::toggle::run(),
        Some(args::Commands::Config {
            key,
            value,
            list,
            path,
        }) => commands::config::run(key, value, list, path),
        Some(args::Commands::Preset { action }) => commands::preset::run(action),
        Some(args::Commands::Setup) => commands::setup::run(),
        Some(args::Commands::Model { action }) => commands::model::run(action),
        None => {
            // Microphone recording or file transcription
            let config =
                commands::record::RecordConfig::from_cli(&cli.input, &cli.processing, &cli.output)?;
            commands::record::run(config)
        }
    }
}
