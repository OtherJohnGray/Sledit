// file src/main.rs
mod app;
mod tui_app;
mod example;

use crate::tui_app::TuiApp;
use clap::*;
use std::path::PathBuf;
use anyhow::Result;
use ctrlc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Create example database with test data
    #[arg(long)]
    make_example_db: bool,

    /// Path to the Sled database directory
    #[arg(value_name = "DB_PATH")]
    db_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Set up Ctrl-C handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;
    
    if cli.make_example_db {
        example::create_example_db(&cli.db_path, running)?;
    } 

    let mut tui = TuiApp::new(cli.db_path)?;
    tui.run(running)?;
    Ok(())
}