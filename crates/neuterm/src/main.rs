//! NeuTerm — fast, configurable terminal emulator.
//!
//! Specs: `specs/SPECS.md`

mod app;
mod input;
mod render;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "neuterm", about = "NeuTerm terminal emulator", version)]
struct Cli {
    /// Override config file path (also: NEUTERM_CONFIG).
    #[arg(long, short = 'c')]
    config: Option<PathBuf>,

    /// Working directory for the initial shell.
    #[arg(long)]
    cwd: Option<PathBuf>,
}

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();
    if let Some(path) = &cli.config {
        std::env::set_var("NEUTERM_CONFIG", path);
    }

    let (mut config, config_path) = neuterm_config::load_or_default();
    if let Some(cwd) = cli.cwd {
        config.term.cwd = Some(cwd.display().to_string());
    }

    tracing::info!(
        "NeuTerm starting (config: {})",
        config_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<built-in defaults>".into())
    );

    app::run(config, config_path)
}
