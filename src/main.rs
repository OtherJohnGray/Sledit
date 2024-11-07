// file src/main.rs

mod app;
mod tui_app;

use anyhow::Result;
use crate::tui_app::TuiApp;

fn main() -> Result<()> {
    let mut tui = TuiApp::new()?;
    tui.run()?;
    Ok(())
}