use std::{path::PathBuf, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::{
    config::{AppConfig, LoadedConfig, SourceSettings},
    i18n::{I18nCatalog, I18nKey},
    skill::{RiskLevel, SkillRecord, SkillScope, SkillState, Source, fixture_skills},
    theme::{ThemePalette, ThemeRegistry},
};

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
    config_path: PathBuf,
    config: AppConfig,
    i18n: I18nCatalog,
    settings: SettingsState,
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
    pub source: Option<Source>,
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
        Self::from_skills(fixture_skills())
    }
}

impl App {
    pub fn load_local_or_fixture() -> Self {
        let loaded_config = crate::config::load_from_env();
        let skills = crate::local_inventory::load_local_inventory_from_env();
        let skills = if skills.is_empty() {
            fixture_skills()
        } else {
            skills
        };

        Self::from_skills_with_config(skills, loaded_config)
    }

    pub fn from_skills(skills: Vec<SkillRecord>) -> Self {
        Self::from_skills_with_config(
            skills,
            LoadedConfig {
                path: PathBuf::from("skillroom/config.toml"),
                config: AppConfig::default(),
                warnings: Vec::new(),
            },
        )
    }

    pub fn from_skills_with_config(skills: Vec<SkillRecord>, loaded_config: LoadedConfig) -> Self {
        let mut output = vec![
            "[system] Skillroom daemon started.".to_string(),
            format!(
                "[skill] Loaded {} skills from local inventory.",
                skills.len()
            ),
            "[prompt] Ready for command.".to_string(),
        ];

        output.extend(skills.iter().filter_map(|skill| {
            skill
                .error
                .as_ref()
                .map(|error| format!("[error] {}: {error}", skill.name))
        }));
        let (config, normalization_warnings) = loaded_config.config.normalized();
        output.extend(
            loaded_config
                .warnings
                .iter()
                .map(|warning| format!("[config] {warning}")),
        );
        output.extend(
            normalization_warnings
                .iter()
                .map(|warning| format!("[config] {warning}")),
        );

        let i18n = I18nCatalog::new(config.language);
        output.extend(i18n.errors().iter().map(|error| format!("[i18n] {error}")));
        let settings = SettingsState::closed(config.clone());

        Self {
            should_quit: false,
            skills,
            selected: 0,
            focus: FocusArea::Table,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            filters: FilterState::default(),
            sort_column: SortColumn::Name,
            sort_ascending: true,
            show_help: false,
            output,
            stream_tick: 0,
            stream_cursor: 0,
            config_path: loaded_config.path,
            config,
            i18n,
            settings,
        }
    }
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
        if event::poll(Duration::from_millis(120))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            self.handle_key(key);
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

        if self.settings.open {
            self.handle_settings_key(key);
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
            (KeyCode::Char(','), _) => self.open_settings(),
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

    fn handle_settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.close_settings(false),
            KeyCode::Enter => self.activate_setting(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous_setting(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_setting(),
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

    pub(crate) fn open_settings(&mut self) {
        self.settings = SettingsState::open(self.config.clone());
        self.focus = FocusArea::Settings;
        self.input_mode = InputMode::Normal;
        self.show_help = false;
        self.push_output("[settings] Opened settings.");
    }

    fn close_settings(&mut self, saved: bool) {
        self.settings.open = false;
        self.focus = FocusArea::Table;
        if saved {
            self.push_output("[settings] Saved settings.");
        } else {
            self.push_output("[settings] Cancelled settings.");
        }
    }

    fn activate_setting(&mut self) {
        let Some(action) = self.settings_actions().get(self.settings.selected).copied() else {
            return;
        };

        match action {
            SettingsAction::Theme => {
                self.settings.draft.theme = self.settings.draft.theme.next();
                self.push_output(&format!(
                    "[settings] Theme -> {}.",
                    self.settings.draft.theme.label()
                ));
            }
            SettingsAction::Language => {
                self.settings.draft.language = self.settings.draft.language.next();
                self.push_output(&format!(
                    "[settings] Language -> {}.",
                    self.settings.draft.language.label()
                ));
            }
            SettingsAction::CacheTtl => {
                self.settings.draft.cache.ttl_seconds =
                    next_cache_ttl(self.settings.draft.cache.ttl_seconds);
                self.push_output(&format!(
                    "[settings] Cache TTL -> {}s.",
                    self.settings.draft.cache.ttl_seconds
                ));
            }
            SettingsAction::CacheClear => {
                self.settings.draft.cache.last_status = "clear-requested".to_string();
                self.push_output("[settings] Cache clear requested.");
            }
            SettingsAction::Safety => {
                self.settings.draft.safety.delete_confirmation = true;
                self.settings.draft.safety.home_delete_guard = true;
                self.push_output("[settings] Safety locks remain enabled.");
            }
            SettingsAction::SourceAdd => {
                let index = self.settings.draft.sources.len() + 1;
                let source = SourceSettings::custom(index);
                self.push_output(&format!("[settings] Added source {}.", source.name));
                self.settings.draft.sources.push(source);
            }
            SettingsAction::SourceToggle(index) => {
                if let Some(source) = self.settings.draft.sources.get_mut(index) {
                    source.enabled = !source.enabled;
                    let name = source.name.clone();
                    let enabled = source.enabled;
                    self.push_output(&format!("[settings] Source {name} enabled={enabled}."));
                }
            }
            SettingsAction::SourceTest(index) => {
                if let Some(source) = self.settings.draft.sources.get_mut(index) {
                    source.last_status = "configured-only".to_string();
                    let name = source.name.clone();
                    self.push_output(&format!(
                        "[settings] Source {name} dry-run ok; no remote request."
                    ));
                }
            }
            SettingsAction::Save => self.save_settings(),
        }
    }

    fn save_settings(&mut self) {
        let (config, warnings) = self.settings.draft.clone().normalized();
        match crate::config::save(&self.config_path, &config) {
            Ok(()) => {
                self.config = config;
                self.i18n = I18nCatalog::new(self.config.language);
                for warning in warnings {
                    self.push_output(&format!("[config] {warning}"));
                }
                for error in self.i18n.errors().to_vec() {
                    self.push_output(&format!("[i18n] {error}"));
                }
                self.close_settings(true);
            }
            Err(error) => {
                self.push_output(&format!("[settings] Failed to save settings: {error}"));
            }
        }
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

    fn select_previous_setting(&mut self) {
        self.settings.selected = self.settings.selected.saturating_sub(1);
    }

    fn select_next_setting(&mut self) {
        let last = self.settings_rows().len().saturating_sub(1);
        self.settings.selected = self.settings.selected.saturating_add(1).min(last);
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

    pub(crate) fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub(crate) fn theme(&self) -> ThemePalette {
        ThemeRegistry::get(self.config.theme)
    }

    pub(crate) fn text(&self, key: I18nKey) -> &'static str {
        self.i18n.text(key)
    }

    pub(crate) fn settings_open(&self) -> bool {
        self.settings.open
    }

    pub(crate) fn settings_selected(&self) -> usize {
        self.settings.selected
    }

    fn settings_actions(&self) -> Vec<SettingsAction> {
        let mut actions = vec![
            SettingsAction::Theme,
            SettingsAction::Language,
            SettingsAction::CacheTtl,
            SettingsAction::CacheClear,
            SettingsAction::Safety,
            SettingsAction::SourceAdd,
        ];
        for index in 0..self.settings.draft.sources.len() {
            actions.push(SettingsAction::SourceToggle(index));
            actions.push(SettingsAction::SourceTest(index));
        }
        actions.push(SettingsAction::Save);
        actions
    }

    pub(crate) fn settings_rows(&self) -> Vec<SettingsRow> {
        self.settings_actions()
            .into_iter()
            .map(|action| self.settings_row(action))
            .collect()
    }

    fn settings_row(&self, action: SettingsAction) -> SettingsRow {
        match action {
            SettingsAction::Theme => SettingsRow::new(
                self.text(I18nKey::SettingsTheme),
                self.settings.draft.theme.label(),
                self.text(I18nKey::HintTheme),
            ),
            SettingsAction::Language => SettingsRow::new(
                self.text(I18nKey::SettingsLanguage),
                self.settings.draft.language.label(),
                self.text(I18nKey::HintLanguage),
            ),
            SettingsAction::CacheTtl => SettingsRow::new(
                self.text(I18nKey::SettingsCacheTtl),
                format!("{}s", self.settings.draft.cache.ttl_seconds),
                self.text(I18nKey::HintCacheTtl),
            ),
            SettingsAction::CacheClear => SettingsRow::new(
                self.text(I18nKey::SettingsCache),
                self.settings.draft.cache.last_status.clone(),
                self.text(I18nKey::HintCache),
            ),
            SettingsAction::Safety => SettingsRow::new(
                self.text(I18nKey::SettingsSafety),
                safety_summary(
                    &self.settings.draft,
                    self.text(I18nKey::ValueSafetyLocked),
                    self.text(I18nKey::ValueSafetyRestored),
                ),
                self.text(I18nKey::HintSafety),
            ),
            SettingsAction::SourceAdd => SettingsRow::new(
                self.text(I18nKey::SettingsSources),
                format!(
                    "{}{}",
                    self.settings.draft.sources.len(),
                    self.text(I18nKey::ValueConfiguredSources)
                ),
                self.text(I18nKey::HintSources),
            ),
            SettingsAction::SourceToggle(index) => {
                let source = &self.settings.draft.sources[index];
                SettingsRow::new(
                    format!(
                        "{}{}",
                        self.text(I18nKey::SettingsSourcePrefix),
                        source.name
                    ),
                    if source.enabled {
                        self.text(I18nKey::ValueEnabled)
                    } else {
                        self.text(I18nKey::ValueDisabled)
                    },
                    self.text(I18nKey::HintSourceToggle),
                )
            }
            SettingsAction::SourceTest(index) => {
                let source = &self.settings.draft.sources[index];
                SettingsRow::new(
                    format!("{}{}", self.text(I18nKey::SettingsTestPrefix), source.name),
                    source.last_status.clone(),
                    self.text(I18nKey::HintSourceTest),
                )
            }
            SettingsAction::Save => SettingsRow::new(
                self.text(I18nKey::SettingsSave),
                self.text(I18nKey::ValueSavePersist),
                self.text(I18nKey::HintSave),
            ),
        }
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
            || contains_case_insensitive(&skill.name, &self.search_query)
            || contains_case_insensitive(skill.source.label(), &self.search_query)
            || contains_case_insensitive(&skill.description, &self.search_query)
            || skill
                .tags
                .iter()
                .any(|tag| contains_case_insensitive(tag, &self.search_query));

        matches_query
            && self
                .filters
                .source
                .as_ref()
                .is_none_or(|source| &skill.source == source)
            && self.filters.scope.is_none_or(|scope| skill.scope == scope)
            && self.filters.state.is_none_or(|state| skill.state == state)
            && self.filters.risk.is_none_or(|risk| skill.risk == risk)
            && self
                .filters
                .update
                .as_ref()
                .is_none_or(|update| skill.update_label() == update)
    }

    fn compare_skills(&self, left: &SkillRecord, right: &SkillRecord) -> std::cmp::Ordering {
        match self.sort_column {
            SortColumn::Name => left.name.cmp(&right.name),
            SortColumn::Source => left
                .source
                .label()
                .cmp(right.source.label())
                .then(left.name.cmp(&right.name)),
            SortColumn::Scope => left
                .scope
                .label()
                .cmp(right.scope.label())
                .then(left.name.cmp(&right.name)),
            SortColumn::State => left
                .state
                .label()
                .cmp(right.state.label())
                .then(left.name.cmp(&right.name)),
            SortColumn::Risk => left
                .risk
                .label()
                .cmp(right.risk.label())
                .then(left.name.cmp(&right.name)),
            SortColumn::Update => left
                .update_label()
                .cmp(right.update_label())
                .then(left.name.cmp(&right.name)),
        }
    }

    fn tick(&mut self) {
        self.stream_tick = self.stream_tick.saturating_add(1);
        if !self.stream_tick.is_multiple_of(Self::STREAM_INTERVAL_TICKS) {
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
    const STREAM_INTERVAL_TICKS: usize = 50;
    const STREAM_MESSAGES: [&'static str; 5] = [
        "[status] Local inventory ready.",
        "[status] Search state ready.",
        "[status] Filter state ready.",
        "[status] Details panel ready.",
        "[prompt] Ready for keyboard input.",
    ];
}

fn next_cache_ttl(current: u64) -> u64 {
    const TTL_VALUES: [u64; 4] = [300, 1_800, 3_600, 86_400];
    let index = TTL_VALUES
        .iter()
        .position(|ttl| *ttl == current)
        .unwrap_or(1);
    TTL_VALUES[(index + 1) % TTL_VALUES.len()]
}

fn safety_summary(
    config: &AppConfig,
    locked: &'static str,
    restored: &'static str,
) -> &'static str {
    if config.safety.delete_confirmation && config.safety.home_delete_guard {
        locked
    } else {
        restored
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SettingsRow {
    pub label: String,
    pub value: String,
    pub hint: String,
}

impl SettingsRow {
    fn new(label: impl Into<String>, value: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            hint: hint.into(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SettingsState {
    open: bool,
    selected: usize,
    draft: AppConfig,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SettingsAction {
    Theme,
    Language,
    CacheTtl,
    CacheClear,
    Safety,
    SourceAdd,
    SourceToggle(usize),
    SourceTest(usize),
    Save,
}

impl SettingsState {
    fn closed(config: AppConfig) -> Self {
        Self {
            open: false,
            selected: 0,
            draft: config,
        }
    }

    fn open(config: AppConfig) -> Self {
        Self {
            open: true,
            selected: 0,
            draft: config,
        }
    }
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Language, SafetySettings, ThemeName, load_or_default},
        i18n::I18nKey,
    };
    use tempfile::tempdir;

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

        app.filters.source = Some(Source::InternalRegistry);
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

        for _ in 0..512 {
            app.tick();
        }

        assert_eq!(app.output().len(), App::OUTPUT_LIMIT);
        assert!(app.output().last().unwrap().starts_with("["));
    }

    #[test]
    fn stream_messages_do_not_claim_external_or_fixture_work() {
        assert!(App::STREAM_MESSAGES.iter().all(
            |message| !message.contains("skills.bytedance.net") && !message.contains("fixture")
        ));
    }

    #[test]
    fn skill_errors_enter_output_without_blocking_list() {
        let mut skills = fixture_skills();
        skills[0].error = Some("parse failed".to_string());
        skills[0].state = SkillState::Error;

        let app = App::from_skills(skills);

        assert_eq!(app.skills().len(), 5);
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("[error]") && line.contains("parse failed"))
        );
    }

    #[test]
    fn comma_opens_settings_and_escape_cancels() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        assert!(app.settings_open());
        assert_eq!(app.focus(), FocusArea::Settings);
        assert_eq!(app.settings_selected(), 0);

        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.settings_selected(), 1);
        app.handle_key(KeyEvent::from(KeyCode::Char('k')));
        assert_eq!(app.settings_selected(), 0);

        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(!app.settings_open());
        assert_eq!(app.focus(), FocusArea::Table);
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Cancelled settings"))
        );
    }

    #[test]
    fn enter_selects_settings_row_without_closing() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(app.settings_open());
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Theme -> Catppuccin Mocha"))
        );
    }

    #[test]
    fn settings_save_persists_theme_and_language() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig::default(),
                warnings: Vec::new(),
            },
        );

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Language");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Save");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(!app.settings_open());
        assert_eq!(app.config.theme, ThemeName::CatppuccinMocha);
        assert_eq!(app.config.language, Language::ZhCn);
        assert_eq!(app.text(I18nKey::KeyQuit), "退出 ");

        let loaded = load_or_default(path);
        assert_eq!(loaded.config.theme, ThemeName::CatppuccinMocha);
        assert_eq!(loaded.config.language, Language::ZhCn);
    }

    #[test]
    fn settings_cancel_discards_draft_without_writing_config() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig::default(),
                warnings: Vec::new(),
            },
        );

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        app.handle_key(KeyEvent::from(KeyCode::Esc));

        assert_eq!(app.config.theme, ThemeName::TokyoNight);
        assert!(!path.exists());
    }

    #[test]
    fn settings_cache_controls_persist_after_save() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig::default(),
                warnings: Vec::new(),
            },
        );

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        move_to_setting(&mut app, "Cache TTL");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Cache");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Save");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        let loaded = load_or_default(path);
        assert_eq!(loaded.config.cache.ttl_seconds, 3_600);
        assert_eq!(loaded.config.cache.last_status, "clear-requested");
    }

    #[test]
    fn settings_safety_controls_cannot_disable_guards() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig {
                    safety: SafetySettings {
                        delete_confirmation: false,
                        home_delete_guard: false,
                    },
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        assert!(app.config.safety.delete_confirmation);
        assert!(app.config.safety.home_delete_guard);

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        move_to_setting(&mut app, "Safety");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Save");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        let loaded = load_or_default(path);
        assert!(loaded.config.safety.delete_confirmation);
        assert!(loaded.config.safety.home_delete_guard);
    }

    #[test]
    fn settings_sources_add_toggle_test_and_persist_without_remote_request() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig::default(),
                warnings: Vec::new(),
            },
        );

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        move_to_setting(&mut app, "Sources");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Source custom-2");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Test custom-2");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Save");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        let loaded = load_or_default(path);
        let source = loaded
            .config
            .sources
            .iter()
            .find(|source| source.name == "custom-2")
            .unwrap();
        assert!(source.enabled);
        assert_eq!(source.last_status, "configured-only");
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("no remote request"))
        );
    }

    #[test]
    fn configured_theme_drives_palette() {
        let app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: PathBuf::from("config.toml"),
                config: AppConfig {
                    theme: ThemeName::GruvboxDark,
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        assert_eq!(app.theme().name, ThemeName::GruvboxDark);
    }

    fn move_to_setting(app: &mut App, label: &str) {
        let target = app
            .settings_rows()
            .iter()
            .position(|row| row.label == label)
            .unwrap_or_else(|| panic!("missing settings row {label}"));

        while app.settings_selected() < target {
            app.handle_key(KeyEvent::from(KeyCode::Down));
        }
        while app.settings_selected() > target {
            app.handle_key(KeyEvent::from(KeyCode::Up));
        }
    }
}
