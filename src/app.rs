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
    focus: FocusArea,
    output: Vec<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FocusArea {
    Table,
    Search,
    Filters,
    Details,
    Settings,
}

impl FocusArea {
    const ORDER: [Self; 5] = [
        Self::Table,
        Self::Search,
        Self::Filters,
        Self::Details,
        Self::Settings,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Table => "Table",
            Self::Search => "Search",
            Self::Filters => "Filters",
            Self::Details => "Details",
            Self::Settings => "Settings",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|area| *area == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|area| *area == self)
            .unwrap_or(0);
        Self::ORDER[(index + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            should_quit: false,
            skills: fixture_skills(),
            selected: 0,
            focus: FocusArea::Table,
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

    pub(crate) fn focus(&self) -> FocusArea {
        self.focus
    }

    pub(crate) fn output(&self) -> &[String] {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_order_covers_placeholders() {
        let mut focus = FocusArea::Table;
        let mut seen = Vec::new();

        for _ in 0..FocusArea::ORDER.len() {
            seen.push(focus);
            focus = focus.next();
        }

        assert_eq!(
            seen,
            vec![
                FocusArea::Table,
                FocusArea::Search,
                FocusArea::Filters,
                FocusArea::Details,
                FocusArea::Settings,
            ]
        );
        assert_eq!(focus, FocusArea::Table);
        assert_eq!(focus.previous(), FocusArea::Settings);
    }
}
