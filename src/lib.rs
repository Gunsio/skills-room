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
pub mod openai_marketplace;
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

use std::{
    sync::mpsc::{self, TryRecvError},
    time::Duration,
};

pub use app::{App, FilterState, FocusArea, InputMode, SortColumn};

pub fn run() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut terminal = ratatui::init();
    let _guard = terminal::TerminalGuard;

    let app = load_app_with_spinner(&mut terminal)?;

    app.run(terminal)
}

enum LoadingEvent {
    Step(usize),
    Done(Box<App>),
}

fn load_app_with_spinner(terminal: &mut ratatui::DefaultTerminal) -> color_eyre::Result<App> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(LoadingEvent::Step(1));
        let loaded_config = config::load_from_env();
        let skills = local_inventory::load_local_inventory_from_env();
        let skills = if skills.is_empty() {
            skill::fixture_skills()
        } else {
            skills
        };
        let mut app = App::from_skills_with_config(skills, loaded_config);
        app.enable_remote_sources();

        let _ = sender.send(LoadingEvent::Step(2));
        app.discover_remote_spaces();

        let _ = sender.send(LoadingEvent::Step(3));
        app.load_enabled_sources_into_table();

        let _ = sender.send(LoadingEvent::Done(Box::new(app)));
    });

    let mut phase = 0usize;
    let mut visible_steps = 0usize;
    loop {
        match receiver.try_recv() {
            Ok(LoadingEvent::Step(step)) => visible_steps = visible_steps.max(step),
            Ok(LoadingEvent::Done(app)) => return Ok(*app),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Err(color_eyre::eyre::eyre!("loading worker exited early"));
            }
        }

        draw_loading(terminal, phase, visible_steps)?;
        phase = phase.wrapping_add(1);
        if crossterm::event::poll(Duration::from_millis(80))?
            && let crossterm::event::Event::Key(key) = crossterm::event::read()?
            && key.kind == crossterm::event::KeyEventKind::Press
            && matches!(
                (key.code, key.modifiers),
                (crossterm::event::KeyCode::Char('q'), _)
                    | (
                        crossterm::event::KeyCode::Char('c'),
                        crossterm::event::KeyModifiers::CONTROL
                    )
            )
        {
            return Err(color_eyre::eyre::eyre!("cancelled during loading"));
        }
    }
}

fn draw_loading(
    terminal: &mut ratatui::DefaultTerminal,
    phase: usize,
    visible_steps: usize,
) -> color_eyre::Result<()> {
    terminal.draw(|frame| ui::render_loading(frame, phase, visible_steps))?;
    Ok(())
}
