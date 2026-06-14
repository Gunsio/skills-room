pub mod actions;
pub mod agentbuddy;
pub mod agentbuddy_marketplace;
pub mod cache;
pub mod cli;
pub mod config;
pub mod i18n;
pub mod inventory;
pub mod loaders;
pub mod local_inventory;
pub mod parser;
pub mod runner;
pub mod scan;
pub mod source;
pub mod source_check;

mod app;
mod layout;
pub mod skill;
mod terminal;
pub mod theme;
mod ui;

pub use app::{App, FilterState, FocusArea, InputMode, SortColumn};

pub fn run() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut terminal = ratatui::init();
    let _guard = terminal::TerminalGuard;

    draw_loading(&mut terminal, 0, 0)?;
    std::thread::sleep(std::time::Duration::from_millis(120));
    draw_loading(&mut terminal, 1, 1)?;
    let loaded_config = config::load_from_env();
    let skills = local_inventory::load_local_inventory_from_env();
    let skills = if skills.is_empty() {
        skill::fixture_skills()
    } else {
        skills
    };
    let mut app = App::from_skills_with_config(skills, loaded_config);
    app.enable_remote_sources();
    std::thread::sleep(std::time::Duration::from_millis(120));
    draw_loading(&mut terminal, 2, 2)?;
    app.discover_remote_spaces();
    std::thread::sleep(std::time::Duration::from_millis(120));
    draw_loading(&mut terminal, 3, 3)?;
    app.load_remote_space_into_table();

    app.run(terminal)
}

fn draw_loading(
    terminal: &mut ratatui::DefaultTerminal,
    phase: usize,
    visible_steps: usize,
) -> color_eyre::Result<()> {
    terminal.draw(|frame| ui::render_loading(frame, phase, visible_steps))?;
    Ok(())
}
