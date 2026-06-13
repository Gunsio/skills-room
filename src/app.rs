use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame, widgets::Paragraph};

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
        frame.render_widget(
            Paragraph::new("Skillroom TUI initializing... press q to quit"),
            frame.area(),
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
