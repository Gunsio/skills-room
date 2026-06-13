mod app;
mod terminal;

pub use app::App;

pub fn run() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let _guard = terminal::TerminalGuard;

    App::default().run(terminal)
}
