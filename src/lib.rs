pub mod cache;
pub mod cli;
pub mod config;
pub mod i18n;
pub mod inventory;
pub mod loaders;
pub mod local_inventory;
pub mod parser;
pub mod scan;

mod app;
mod layout;
pub mod skill;
mod terminal;
pub mod theme;
mod ui;

pub use app::{App, FilterState, FocusArea, InputMode, SortColumn};

pub fn run() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let _guard = terminal::TerminalGuard;

    App::load_local_or_fixture().run(terminal)
}
