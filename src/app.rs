use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame, layout::Alignment, widgets::Paragraph};

use crate::layout::{AppLayout, too_small_message};

#[derive(Debug, Default)]
pub struct App {
    should_quit: bool,
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
        let area = frame.area();
        let Some(layout) = AppLayout::calculate(area) else {
            frame.render_widget(Paragraph::new(too_small_message(area)).centered(), area);
            return;
        };

        frame.render_widget(
            Paragraph::new(format!("Skillroom TUI [{:?}]", layout.tier))
                .alignment(Alignment::Center),
            layout.search,
        );
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
}
