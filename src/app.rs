use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::skill::{RiskLevel, SkillRecord, SkillScope, SkillState, fixture_skills};

#[derive(Debug)]
pub struct App {
    should_quit: bool,
    skills: Vec<SkillRecord>,
    selected: usize,
    focus: FocusArea,
    input_mode: InputMode,
    search_query: String,
    filters: FilterState,
    sort_column: SortColumn,
    sort_ascending: bool,
    show_help: bool,
    output: Vec<String>,
    stream_tick: usize,
    stream_cursor: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SortColumn {
    Name,
    Source,
    Scope,
    State,
    Risk,
    Update,
}

impl SortColumn {
    const ORDER: [Self; 6] = [
        Self::Name,
        Self::Source,
        Self::Scope,
        Self::State,
        Self::Risk,
        Self::Update,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Source => "Source",
            Self::Scope => "Scope",
            Self::State => "State",
            Self::Risk => "Risk",
            Self::Update => "Update",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|column| *column == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct FilterState {
    pub source: Option<String>,
    pub scope: Option<SkillScope>,
    pub state: Option<SkillState>,
    pub risk: Option<RiskLevel>,
    pub update: Option<String>,
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
            input_mode: InputMode::Normal,
            search_query: String::new(),
            filters: FilterState::default(),
            sort_column: SortColumn::Name,
            sort_ascending: true,
            show_help: false,
            output: vec![
                "[system] Skillroom daemon started.".to_string(),
                "[skill] Loaded fixture skills from local storage.".to_string(),
                "[prompt] Ready for command.".to_string(),
            ],
            stream_tick: 0,
            stream_cursor: 0,
        }
    }
}

impl App {
    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
            self.tick();
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
        if matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('c'), KeyModifiers::CONTROL)
        ) {
            self.should_quit = true;
            return;
        }

        if self.input_mode == InputMode::Search {
            self.handle_search_key(key);
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) => {
                self.should_quit = true;
            }
            (KeyCode::Char('/'), _) => self.enter_search_mode(),
            (KeyCode::Char('?'), _) => {
                self.show_help = !self.show_help;
            }
            (KeyCode::Tab, KeyModifiers::SHIFT) => {
                self.focus = self.focus.previous();
            }
            (KeyCode::Tab, _) => {
                self.focus = self.focus.next();
            }
            (KeyCode::Char('s'), _) => {
                self.sort_column = self.sort_column.next();
                self.clamp_selection();
            }
            (KeyCode::Char('S'), _) => {
                self.sort_ascending = !self.sort_ascending;
                self.clamp_selection();
            }
            (KeyCode::Up | KeyCode::Char('k'), _) => self.select_previous(),
            (KeyCode::Down | KeyCode::Char('j'), _) => self.select_next(),
            (KeyCode::PageUp, _) => self.select_page_up(),
            (KeyCode::PageDown, _) => self.select_page_down(),
            (KeyCode::Char('g'), _) => self.select_first(),
            (KeyCode::Char('G'), _) => self.select_last(),
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.search_query.is_empty() {
                    self.input_mode = InputMode::Normal;
                    self.focus = FocusArea::Table;
                } else {
                    self.search_query.clear();
                    self.clamp_selection();
                }
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
                self.focus = FocusArea::Table;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.clamp_selection();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search_query.push(character);
                self.clamp_selection();
            }
            _ => {}
        }
    }

    fn enter_search_mode(&mut self) {
        self.input_mode = InputMode::Search;
        self.focus = FocusArea::Search;
        self.show_help = false;
    }

    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn select_next(&mut self) {
        let last = self.visible_skill_count().saturating_sub(1);
        self.selected = self.selected.saturating_add(1).min(last);
    }

    fn select_page_up(&mut self) {
        self.selected = self.selected.saturating_sub(Self::PAGE_SIZE);
    }

    fn select_page_down(&mut self) {
        let last = self.visible_skill_count().saturating_sub(1);
        self.selected = self.selected.saturating_add(Self::PAGE_SIZE).min(last);
    }

    fn select_first(&mut self) {
        self.selected = 0;
    }

    fn select_last(&mut self) {
        self.selected = self.visible_skill_count().saturating_sub(1);
    }

    fn clamp_selection(&mut self) {
        self.selected = self
            .selected
            .min(self.visible_skill_count().saturating_sub(1));
    }

    fn visible_skill_count(&self) -> usize {
        self.visible_skills().len()
    }

    const PAGE_SIZE: usize = 5;

    pub(crate) fn skills(&self) -> &[SkillRecord] {
        &self.skills
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected
    }

    pub(crate) fn selected_skill(&self) -> Option<&SkillRecord> {
        self.visible_skills()
            .get(self.selected)
            .map(|(_, skill)| *skill)
    }

    pub(crate) fn focus(&self) -> FocusArea {
        self.focus
    }

    pub(crate) fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    pub(crate) fn search_query(&self) -> &str {
        &self.search_query
    }

    pub(crate) fn filters(&self) -> &FilterState {
        &self.filters
    }

    pub(crate) fn sort_column(&self) -> SortColumn {
        self.sort_column
    }

    pub(crate) fn sort_ascending(&self) -> bool {
        self.sort_ascending
    }

    pub(crate) fn show_help(&self) -> bool {
        self.show_help
    }

    pub(crate) fn output(&self) -> &[String] {
        &self.output
    }

    pub(crate) fn visible_skills(&self) -> Vec<(usize, &SkillRecord)> {
        let mut skills: Vec<(usize, &SkillRecord)> = self
            .skills
            .iter()
            .enumerate()
            .filter(|(_, skill)| self.matches_filters(skill))
            .collect();

        skills.sort_by(|(_, left), (_, right)| self.compare_skills(left, right));
        if !self.sort_ascending {
            skills.reverse();
        }

        skills
    }

    fn matches_filters(&self, skill: &SkillRecord) -> bool {
        let matches_query = self.search_query.is_empty()
            || contains_case_insensitive(skill.name, &self.search_query)
            || contains_case_insensitive(skill.source, &self.search_query)
            || contains_case_insensitive(skill.description, &self.search_query)
            || skill
                .tags
                .iter()
                .any(|tag| contains_case_insensitive(tag, &self.search_query));

        matches_query
            && self
                .filters
                .source
                .as_ref()
                .is_none_or(|source| skill.source == source)
            && self.filters.scope.is_none_or(|scope| skill.scope == scope)
            && self.filters.state.is_none_or(|state| skill.state == state)
            && self.filters.risk.is_none_or(|risk| skill.risk == risk)
            && self
                .filters
                .update
                .as_ref()
                .is_none_or(|update| skill.update == update)
    }

    fn compare_skills(&self, left: &SkillRecord, right: &SkillRecord) -> std::cmp::Ordering {
        match self.sort_column {
            SortColumn::Name => left.name.cmp(right.name),
            SortColumn::Source => left
                .source
                .cmp(right.source)
                .then(left.name.cmp(right.name)),
            SortColumn::Scope => left
                .scope
                .label()
                .cmp(right.scope.label())
                .then(left.name.cmp(right.name)),
            SortColumn::State => left
                .state
                .label()
                .cmp(right.state.label())
                .then(left.name.cmp(right.name)),
            SortColumn::Risk => left
                .risk
                .label()
                .cmp(right.risk.label())
                .then(left.name.cmp(right.name)),
            SortColumn::Update => left
                .update
                .cmp(right.update)
                .then(left.name.cmp(right.name)),
        }
    }

    fn tick(&mut self) {
        self.stream_tick = self.stream_tick.saturating_add(1);
        if self.stream_tick % Self::STREAM_INTERVAL_TICKS != 0 {
            return;
        }

        let message = Self::STREAM_MESSAGES[self.stream_cursor % Self::STREAM_MESSAGES.len()];
        self.stream_cursor = self.stream_cursor.saturating_add(1);
        self.push_output(message);
    }

    fn push_output(&mut self, message: &str) {
        self.output.push(message.to_string());
        let overflow = self.output.len().saturating_sub(Self::OUTPUT_LIMIT);
        if overflow > 0 {
            self.output.drain(0..overflow);
        }
    }

    const OUTPUT_LIMIT: usize = 8;
    const STREAM_INTERVAL_TICKS: usize = 4;
    const STREAM_MESSAGES: [&'static str; 5] = [
        "[scan] Checked local skill manifests.",
        "[scan] Indexed source: skills.bytedance.net.",
        "[sort] Applied fixture sort state.",
        "[filter] Applied fixture filters.",
        "[prompt] Ready for keyboard input.",
    ];
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
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

    #[test]
    fn navigation_keys_clamp_selection() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.selected_index(), 0);

        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.selected_index(), 1);

        app.handle_key(KeyEvent::from(KeyCode::PageDown));
        assert_eq!(app.selected_index(), app.visible_skills().len() - 1);

        app.handle_key(KeyEvent::from(KeyCode::PageUp));
        assert_eq!(app.selected_index(), 0);

        app.handle_key(KeyEvent::from(KeyCode::Char('G')));
        assert_eq!(app.selected_index(), app.visible_skills().len() - 1);

        app.handle_key(KeyEvent::from(KeyCode::Char('g')));
        assert_eq!(app.selected_index(), 0);
    }

    #[test]
    fn tab_and_help_keys_update_global_state() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Tab));
        assert_eq!(app.focus(), FocusArea::Search);

        app.handle_key(KeyEvent::from(KeyCode::Char('?')));
        assert!(app.show_help());

        app.handle_key(KeyEvent::from(KeyCode::Char('?')));
        assert!(!app.show_help());
    }

    #[test]
    fn slash_enters_search_and_escape_clears_then_exits() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('/')));
        assert_eq!(app.input_mode(), InputMode::Search);
        assert_eq!(app.focus(), FocusArea::Search);

        app.handle_key(KeyEvent::from(KeyCode::Char('d')));
        app.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.search_query(), "da");

        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.search_query(), "");
        assert_eq!(app.input_mode(), InputMode::Search);

        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.focus(), FocusArea::Table);
    }

    #[test]
    fn search_mode_keeps_q_as_query_text() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('/')));
        app.handle_key(KeyEvent::from(KeyCode::Char('q')));

        assert_eq!(app.search_query(), "q");
        assert!(!app.should_quit);
    }

    #[test]
    fn search_query_filters_visible_skills() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('/')));
        for character in "metrics".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }

        let visible = app.visible_skills();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].1.name, "data-analysis");
    }

    #[test]
    fn sorting_cycles_across_fixture_columns() {
        let mut app = App::default();

        assert_eq!(app.sort_column(), SortColumn::Name);
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(app.sort_column(), SortColumn::Source);
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(app.sort_column(), SortColumn::Scope);
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(app.sort_column(), SortColumn::State);
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(app.sort_column(), SortColumn::Risk);
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert_eq!(app.sort_column(), SortColumn::Update);

        assert!(app.sort_ascending());
        app.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert!(!app.sort_ascending());
    }

    #[test]
    fn fixture_filters_cover_source_scope_state_risk_and_update() {
        let mut app = App::default();

        app.filters.source = Some("skills.bytedance.net".to_string());
        app.filters.scope = Some(SkillScope::Global);
        app.filters.state = Some(SkillState::Active);
        app.filters.risk = Some(RiskLevel::Low);
        app.filters.update = Some("current".to_string());

        let visible = app.visible_skills();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].1.name, "data-analysis");
    }

    #[test]
    fn streaming_output_is_bounded() {
        let mut app = App::default();

        for _ in 0..64 {
            app.tick();
        }

        assert_eq!(app.output().len(), App::OUTPUT_LIMIT);
        assert!(app.output().last().unwrap().starts_with("["));
    }
}
