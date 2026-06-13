mod app;
mod layout;
pub mod skill;
mod terminal;
mod ui;

pub use app::{App, FocusArea};

pub fn run() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let _guard = terminal::TerminalGuard;

    App::default().run(terminal)
}
