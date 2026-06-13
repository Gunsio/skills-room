use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::skill::{SkillRecord, fixture_skills};

#[derive(Debug)]
pub struct App {
    should_quit: bool,
    skills: Vec<SkillRecord>,
    selected: usize,
    output: Vec<String>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            should_quit: false,
            skills: fixture_skills(),
            selected: 0,
            output: vec![
                "[system] Skillroom daemon started.".to_string(),
                "[skill] Loaded fixture skills from local storage.".to_string(),
                "[prompt] Ready for command.".to_string(),
            ],
        }
    }
}

impl App {
    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }

        Ok(())
    }

    fn render(&self, frame: &mut Frame<'_>) {
        crate::ui::render(self, frame);
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key(key);
                }
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    pub(crate) fn skills(&self) -> &[SkillRecord] {
        &self.skills
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected
    }

    pub(crate) fn selected_skill(&self) -> Option<&SkillRecord> {
        self.skills.get(self.selected)
    }

    pub(crate) fn output(&self) -> &[String] {
        &self.output
    }
}
