mod app;
mod mmcli;
mod ui;

use anyhow::{Context, Result};
use app::App;

fn main() -> Result<()> {
    // Find the first available modem, or default to 0
    let modem_index = mmcli::list_modems()
        .ok()
        .and_then(|m| m.into_iter().next())
        .unwrap_or(0);

    let mut terminal = ratatui::init();
    let result = App::new(modem_index).run(&mut terminal);
    ratatui::restore();

    result.context("Application error")
}
