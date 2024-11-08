// file src/main.rs
mod app;
mod tui_app;

use crate::tui_app::TuiApp;
use clap::*;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Sled database directory
    #[arg(value_name = "DB_PATH")]
    db_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut tui = TuiApp::new(cli.db_path)?;
    tui.run()?;
    Ok(())
}