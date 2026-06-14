use std::{path::PathBuf, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{DefaultTerminal, Frame};

use crate::{
    actions::{ActionKind, ActionPlan, ActionPlanner},
    agentbuddy_marketplace::{
        AgentBuddyMarketplaceAdapter, AgentBuddyMarketplaceConfig, AgentBuddySpace, CurlHttpClient,
        HttpClient,
    },
    config::{AppConfig, LoadedConfig, SourceKind, SourceSettings, SpaceSettings},
    i18n::{I18nCatalog, I18nKey},
    runner::{ActionRunner, RunnerEvent, RunningAction},
    skill::{RiskLevel, SkillRecord, SkillScope, SkillState, Source, fixture_skills},
    source::{SourceAdapter, SourceOrder, SourceQuery},
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
    pending_action: Option<ActionConfirmation>,
    running_actions: Vec<RunningAction>,
    runner: ActionRunner,
    config_path: PathBuf,
    config: AppConfig,
    i18n: I18nCatalog,
    settings: SettingsState,
    space_picker: SpacePickerState,
    remote_sources_enabled: bool,
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
    pub local_only: bool,
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

        let mut app = Self::from_skills_with_config(skills, loaded_config);
        app.enable_remote_sources_and_load();
        app
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
            pending_action: None,
            running_actions: Vec::new(),
            runner: ActionRunner::for_environment(),
            config_path: loaded_config.path,
            config,
            i18n,
            settings,
            space_picker: SpacePickerState::closed(),
            remote_sources_enabled: false,
        }
    }

    pub(crate) fn enable_remote_sources_and_load(&mut self) {
        self.remote_sources_enabled = true;
        self.discover_remote_spaces();
        self.load_remote_space_into_table();
    }

    pub(crate) fn enable_remote_sources(&mut self) {
        self.remote_sources_enabled = true;
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

        if self.space_picker.open {
            self.handle_space_picker_key(key);
            return;
        }

        if self.settings.open {
            self.handle_settings_key(key);
            return;
        }

        if self.pending_action.is_some() {
            self.handle_action_confirmation_key(key);
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
            (KeyCode::Char('R'), _) => self.refresh_inventory(),
            (KeyCode::Char('a'), _) => self.clear_filters(),
            (KeyCode::Char('f'), _) => self.cycle_source_filter(),
            (KeyCode::Char('i'), _) => self.toggle_local_filter(),
            (KeyCode::Char('o'), _) => self.toggle_state_filter(SkillState::UpdateAvailable),
            (KeyCode::Char('v'), _) => self.toggle_state_filter(SkillState::Active),
            (KeyCode::Char('t'), _) => self.open_action(ActionKind::Install),
            (KeyCode::Char('u'), _) => self.open_action(ActionKind::UpdateSelected),
            (KeyCode::Char('U'), _) => self.open_action(ActionKind::UpdateAll),
            (KeyCode::Char('x'), _) => self.open_action(ActionKind::Remove),
            (KeyCode::Char('h'), _) if self.focus == FocusArea::Details => {
                self.focus = FocusArea::Table;
                self.push_output("[detail] Returned to skills list.");
            }
            (KeyCode::Char('h'), _) => self.open_action(ActionKind::OpenPath),
            (KeyCode::Char('y'), _) => self.open_action(ActionKind::CopyPath),
            (KeyCode::Enter | KeyCode::Char('l'), _) => self.select_current_skill(),
            (KeyCode::Tab, KeyModifiers::SHIFT) => {
                self.focus = self.focus.previous();
            }
            (KeyCode::Tab, _) => {
                self.focus = self.focus.next();
            }
            (KeyCode::Char('s'), _) => self.open_space_picker(),
            (KeyCode::Char('S'), _) => {
                self.sort_column = self.sort_column.next();
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

    fn handle_action_confirmation_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                let title = self
                    .pending_action
                    .as_ref()
                    .map(|confirmation| confirmation.plan.title.clone())
                    .unwrap_or_else(|| "action".to_string());
                self.pending_action = None;
                self.push_output(&format!("[action] Cancelled {title}."));
            }
            KeyCode::Enter => self.confirm_pending_action(),
            KeyCode::Backspace => {
                if let Some(confirmation) = &mut self.pending_action {
                    confirmation.input.pop();
                }
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                if let Some(confirmation) = &mut self.pending_action {
                    confirmation.input.push(character);
                }
            }
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

    fn handle_space_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => {
                self.close_space_picker(false);
            }
            KeyCode::Enter | KeyCode::Char('l') => self.apply_selected_space(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous_space(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next_space(),
            KeyCode::Char('g') => self.space_picker.selected = 0,
            KeyCode::Char('G') => {
                self.space_picker.selected = self.space_picker_rows().len().saturating_sub(1);
            }
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

    fn refresh_inventory(&mut self) {
        let skills = crate::local_inventory::load_local_inventory_from_env();
        self.skills = if skills.is_empty() {
            fixture_skills()
        } else {
            skills
        };
        self.clamp_selection();
        self.push_output(&format!(
            "[status] Refreshed {} skills from local inventory.",
            self.skills.len()
        ));
        if self.remote_sources_enabled {
            self.discover_remote_spaces();
            self.load_remote_space_into_table();
        }
    }

    pub(crate) fn discover_remote_spaces(&mut self) {
        match fetch_available_spaces(&self.config, CurlHttpClient::default()) {
            Ok(spaces) => {
                let total = spaces.len();
                let added = merge_spaces(&mut self.config.spaces, spaces);
                self.push_output(&format!(
                    "[space] Discovered {total} selectable Spaces; {added} added to settings."
                ));
            }
            Err(RemoteSpaceLoadError::NoEnabledSource) => {
                self.push_output("[space] No enabled AgentBuddy source configured.");
            }
            Err(RemoteSpaceLoadError::SearchFailed { space_label, error }) => {
                self.push_output(&format!(
                    "[space] Space list {space_label} unavailable: {error}."
                ));
            }
            Err(RemoteSpaceLoadError::NoActiveSpace) => {}
        }
    }

    pub(crate) fn load_remote_space_into_table(&mut self) {
        match fetch_active_space_records(&self.config, CurlHttpClient::default()) {
            Ok((space_label, records)) => {
                let total = records.len();
                let added = merge_remote_records(&mut self.skills, records);
                self.clamp_selection();
                self.push_output(&format!(
                    "[source] Loaded {total} skills from Space {space_label}; {added} added to table."
                ));
            }
            Err(RemoteSpaceLoadError::NoActiveSpace) => {
                self.push_output("[source] No active Space configured.");
            }
            Err(RemoteSpaceLoadError::NoEnabledSource) => {
                self.push_output("[source] No enabled AgentBuddy source configured.");
            }
            Err(RemoteSpaceLoadError::SearchFailed { space_label, error }) => {
                self.push_output(&format!(
                    "[source] Space {space_label} unavailable: {error}."
                ));
            }
        }
    }

    fn prune_remote_source_records(&mut self) {
        let source_names = self
            .config
            .sources
            .iter()
            .filter(|source| source.kind == SourceKind::AgentBuddy)
            .map(|source| source.name.as_str())
            .collect::<Vec<_>>();
        self.skills.retain(|skill| {
            !skill
                .metadata
                .source_status
                .as_deref()
                .is_some_and(|status| source_names.iter().any(|source| source == &status))
        });
        self.clamp_selection();
    }

    fn select_current_skill(&mut self) {
        let Some(skill) = self.selected_skill() else {
            self.push_output("[select] No visible skill selected.");
            return;
        };
        let name = skill.name.clone();
        let source = skill.source.label().to_string();
        self.focus = FocusArea::Details;
        self.push_output(&format!("[select] {name} selected; source={source}."));
    }

    fn open_space_picker(&mut self) {
        if self.config.spaces.is_empty() && self.remote_sources_enabled {
            self.discover_remote_spaces();
        }
        self.space_picker = SpacePickerState::open(active_space_index(&self.config));
        self.focus = FocusArea::Settings;
        self.push_output("[space] Opened Space list.");
    }

    fn close_space_picker(&mut self, saved: bool) {
        self.space_picker.open = false;
        self.focus = FocusArea::Table;
        if saved {
            self.push_output("[space] Applied Space selection.");
        } else {
            self.push_output("[space] Closed Space list.");
        }
    }

    fn select_previous_space(&mut self) {
        self.space_picker.selected = self.space_picker.selected.saturating_sub(1);
    }

    fn select_next_space(&mut self) {
        let last = self.space_picker_rows().len().saturating_sub(1);
        self.space_picker.selected = self.space_picker.selected.saturating_add(1).min(last);
    }

    fn apply_selected_space(&mut self) {
        let selected = self.space_picker.selected;
        let mut draft = self.config.clone();
        if selected == 0 {
            draft.active_space = None;
        } else if let Some(space) = draft.spaces.get(selected - 1) {
            draft.active_space = Some(space.id.clone());
        } else {
            self.push_output("[space] No selectable Space at cursor.");
            return;
        }

        let (config, warnings) = draft.normalized();
        match crate::config::save(&self.config_path, &config) {
            Ok(()) => {
                self.config = config;
                self.search_query.clear();
                self.filters = FilterState::default();
                self.input_mode = InputMode::Normal;
                for warning in warnings {
                    self.push_output(&format!("[config] {warning}"));
                }
                self.close_space_picker(true);
                if self.remote_sources_enabled {
                    self.prune_remote_source_records();
                    self.load_remote_space_into_table();
                }
            }
            Err(error) => {
                self.push_output(&format!("[space] Failed to save Space selection: {error}"));
            }
        }
    }

    fn clear_filters(&mut self) {
        self.search_query.clear();
        self.filters = FilterState::default();
        self.input_mode = InputMode::Normal;
        self.focus = FocusArea::Table;
        self.clamp_selection();
        self.push_output("[filter] Reset all filters.");
    }

    fn cycle_source_filter(&mut self) {
        self.filters.local_only = false;
        let sources = self.available_sources();
        if sources.is_empty() {
            self.filters.source = None;
            self.push_output("[filter] No sources available.");
            return;
        }

        self.filters.source = match &self.filters.source {
            None => sources.first().cloned(),
            Some(current) => {
                let index = sources.iter().position(|source| source == current);
                match index {
                    Some(index) if index + 1 < sources.len() => sources.get(index + 1).cloned(),
                    _ => None,
                }
            }
        };
        self.clamp_selection();
        self.push_output(&format!(
            "[filter] Source -> {}.",
            self.source_filter_label()
        ));
    }

    fn toggle_local_filter(&mut self) {
        self.filters.local_only = !self.filters.local_only;
        if self.filters.local_only {
            self.filters.source = None;
        }
        self.clamp_selection();
        self.push_output(&format!(
            "[filter] Local skills -> {}.",
            if self.filters.local_only { "on" } else { "off" }
        ));
    }

    fn toggle_state_filter(&mut self, state: SkillState) {
        self.filters.state = if self.filters.state == Some(state) {
            None
        } else {
            Some(state)
        };
        self.clamp_selection();
        self.push_output(&format!(
            "[filter] State -> {}.",
            self.filters
                .state
                .map(|state| state.label())
                .unwrap_or("all")
        ));
    }

    fn open_action(&mut self, kind: ActionKind) {
        self.show_help = false;
        self.input_mode = InputMode::Normal;

        let planner = self.action_planner();
        let plan = match kind {
            ActionKind::UpdateAll => planner.update_all(&self.skills),
            _ => planner.plan_selected(kind, self.selected_skill()),
        };

        match plan {
            Ok(plan) if self.is_action_locked(&plan) => {
                self.push_output(&format!(
                    "[lock] {} already has a running write operation.",
                    plan.skill_name
                ));
            }
            Ok(plan) if plan.confirmation_token.is_some() => {
                self.push_plan_summary(&plan);
                self.pending_action = Some(ActionConfirmation {
                    input: String::new(),
                    plan,
                });
            }
            Ok(plan) => self.apply_immediate_action(plan),
            Err(error) => {
                self.push_output(&format!(
                    "[action] {} unavailable: {}",
                    kind.label(),
                    error.user_message()
                ));
            }
        }
    }

    fn is_action_locked(&self, plan: &ActionPlan) -> bool {
        plan.is_write()
            && self
                .running_actions
                .iter()
                .any(|action| action.target_key == plan.target_key)
    }

    fn confirm_pending_action(&mut self) {
        let Some(confirmation) = self.pending_action.take() else {
            return;
        };

        if let Some(token) = confirmation.plan.confirmation_token
            && confirmation.input != token
        {
            let title = confirmation.plan.title.clone();
            self.pending_action = Some(confirmation);
            self.push_output(&format!("[action] {title} requires typed {token}."));
            return;
        }

        self.start_action(confirmation.plan);
    }

    fn start_action(&mut self, plan: ActionPlan) {
        if plan.is_write()
            && self
                .running_actions
                .iter()
                .any(|action| action.target_key == plan.target_key)
        {
            self.push_output(&format!(
                "[lock] {} already has a running write operation.",
                plan.skill_name
            ));
            return;
        }

        let title = plan.title.clone();
        match self.runner.start(plan) {
            Ok(running) => {
                self.push_output(&format!("[action] Started {title}."));
                self.running_actions.push(running);
            }
            Err(error) => {
                self.push_output(&format!(
                    "[action] Failed to start {title}: {}",
                    error.message
                ));
            }
        }
    }

    fn apply_immediate_action(&mut self, plan: ActionPlan) {
        match plan.kind {
            ActionKind::CopyPath | ActionKind::OpenPath => self.start_action(plan),
            _ => {
                self.push_output(&format!("[action] Prepared {}.", plan.title));
            }
        }
    }

    fn push_plan_summary(&mut self, plan: &ActionPlan) {
        self.push_output(&format!(
            "[plan] {} impact={} scope={} source={}",
            plan.title,
            plan.impact,
            plan.scope.label(),
            plan.source
        ));
        for command in plan.command_lines() {
            self.push_output(&format!("[argv] {command}"));
        }
        for skipped in &plan.skipped {
            self.push_output(&format!("[skip] {skipped}"));
        }
    }

    fn action_planner(&self) -> ActionPlanner {
        ActionPlanner::new(cfg!(test))
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
            SettingsAction::Space => {
                self.settings.draft.active_space = None;
                self.push_output("[settings] Space -> none.");
            }
            SettingsAction::SpaceChoice(index) => {
                if let Some(space) = self.settings.draft.spaces.get(index) {
                    let label = space.label.clone();
                    self.settings.draft.active_space = Some(space.id.clone());
                    self.push_output(&format!("[settings] Space -> {label}."));
                }
            }
            SettingsAction::SourceAdd => {
                let source = if self
                    .settings
                    .draft
                    .sources
                    .iter()
                    .any(|source| source.kind == SourceKind::AgentBuddy)
                {
                    let index = self.settings.draft.sources.len() + 1;
                    SourceSettings::custom(index)
                } else {
                    SourceSettings::bytedance()
                };
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
                    let report = crate::source_check::check_source_settings(source);
                    source.last_status = report.status_line();
                    let name = source.name.clone();
                    self.push_output(&format!(
                        "[settings] Source {name} checks: {}; no remote request.",
                        report.output_line()
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
                if self.remote_sources_enabled {
                    self.prune_remote_source_records();
                    self.load_remote_space_into_table();
                }
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

    #[cfg(test)]
    pub(crate) fn set_selected_for_test(&mut self, selected: usize) {
        self.selected = selected;
        self.clamp_selection();
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

    pub(crate) fn source_filter_label(&self) -> String {
        self.filters
            .source
            .as_ref()
            .map(|source| source.label().to_string())
            .unwrap_or_else(|| "all".to_string())
    }

    #[cfg(test)]
    pub(crate) fn local_filter_label(&self) -> &'static str {
        if self.filters.local_only {
            "local"
        } else {
            "all scopes"
        }
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

    pub(crate) fn active_space_label(&self) -> Option<&str> {
        active_space(&self.config).map(|space| space.label.as_str())
    }

    pub(crate) fn active_space_scope(&self) -> Option<&str> {
        active_space(&self.config).map(|space| space.scope.as_str())
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

    pub(crate) fn space_picker_open(&self) -> bool {
        self.space_picker.open
    }

    pub(crate) fn pending_action(&self) -> Option<&ActionConfirmation> {
        self.pending_action.as_ref()
    }

    pub(crate) fn settings_selected(&self) -> usize {
        self.settings.selected
    }

    pub(crate) fn space_picker_selected(&self) -> usize {
        self.space_picker.selected
    }

    pub(crate) fn space_picker_rows(&self) -> Vec<SpacePickerRow> {
        let mut rows = vec![SpacePickerRow {
            label: "Local only".to_string(),
            value: "no remote Space".to_string(),
            scope: "local inventory".to_string(),
            active: self.config.active_space.is_none(),
        }];

        rows.extend(
            self.config
                .spaces
                .iter()
                .filter(|space| space.enabled)
                .map(|space| SpacePickerRow {
                    label: space.label.clone(),
                    value: format!("{} skills", space.package_count),
                    scope: space.scope.clone(),
                    active: self.config.active_space.as_deref() == Some(space.id.as_str()),
                }),
        );

        rows
    }

    fn settings_actions(&self) -> Vec<SettingsAction> {
        let mut actions = vec![
            SettingsAction::Theme,
            SettingsAction::Language,
            SettingsAction::CacheTtl,
            SettingsAction::CacheClear,
            SettingsAction::Safety,
            SettingsAction::Space,
        ];
        for index in 0..self.settings.draft.spaces.len() {
            actions.push(SettingsAction::SpaceChoice(index));
        }
        actions.push(SettingsAction::SourceAdd);
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
            SettingsAction::Space => SettingsRow::new(
                self.text(I18nKey::SettingsSpace),
                active_space(&self.settings.draft)
                    .map(|space| space.label.clone())
                    .unwrap_or_else(|| {
                        if self.settings.draft.spaces.is_empty() {
                            self.text(I18nKey::ValueNoSpace).to_string()
                        } else {
                            format!("{} discovered", self.settings.draft.spaces.len())
                        }
                    }),
                self.text(I18nKey::HintSpace),
            ),
            SettingsAction::SpaceChoice(index) => {
                let space = &self.settings.draft.spaces[index];
                let selected = self.settings.draft.active_space.as_deref() == Some(&space.id);
                SettingsRow::new(
                    format!("Space {}", space.label),
                    if selected {
                        "selected".to_string()
                    } else {
                        format!("{} skills", space.package_count)
                    },
                    "Enter selects space",
                )
            }
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
            && self.matches_active_space(skill)
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
            && (!self.filters.local_only || is_local_source(&skill.source))
    }

    fn matches_active_space(&self, skill: &SkillRecord) -> bool {
        let Some(space) = active_space(&self.config) else {
            return true;
        };

        if self.filters.scope == Some(SkillScope::Local) {
            return true;
        }

        if self.filters.local_only {
            return true;
        }

        if self
            .filters
            .source
            .as_ref()
            .is_some_and(|source| matches!(source, Source::LocalGit | Source::LocalArchive))
        {
            return true;
        }

        let space_tag = format!("space:{}", space.scope);
        skill.tags.iter().any(|tag| tag == &space_tag)
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

    fn available_sources(&self) -> Vec<Source> {
        let mut sources = Vec::new();
        for skill in &self.skills {
            if !sources.iter().any(|source| source == &skill.source) {
                sources.push(skill.source.clone());
            }
        }
        sources.sort_by(|left, right| left.label().cmp(right.label()));
        sources
    }

    fn tick(&mut self) {
        if self.drain_running_actions() {
            return;
        }

        self.stream_tick = self.stream_tick.saturating_add(1);
        if !self.stream_tick.is_multiple_of(Self::STREAM_INTERVAL_TICKS) {
            return;
        }

        let message = Self::STREAM_MESSAGES[self.stream_cursor % Self::STREAM_MESSAGES.len()];
        self.stream_cursor = self.stream_cursor.saturating_add(1);
        self.push_output(message);
    }

    fn drain_running_actions(&mut self) -> bool {
        let mut outputs = Vec::new();
        let mut completions = Vec::new();

        for action in &mut self.running_actions {
            for event in action.drain_events() {
                match event {
                    RunnerEvent::Started { argv } => {
                        outputs.push(format!("[run] {}: {}", action.title, argv.join(" ")));
                    }
                    RunnerEvent::Stdout(line) => outputs.push(format!("[stdout] {line}")),
                    RunnerEvent::Stderr(line) => outputs.push(format!("[stderr] {line}")),
                    RunnerEvent::CommandExit { argv, code } => {
                        outputs.push(format!(
                            "[exit] {} -> {}",
                            argv.join(" "),
                            exit_code_label(code)
                        ));
                    }
                    RunnerEvent::Finished { code } => {
                        let success = code == Some(0);
                        if success {
                            outputs.push(format!("[action] Finished {}.", action.title));
                        } else {
                            outputs.push(format!(
                                "[action] Failed {}: exit {}; source={}; argv={}; stderr={}",
                                action.title,
                                exit_code_label(code),
                                action.source_label,
                                action.command_lines.join(" && "),
                                action.stderr_summary()
                            ));
                        }
                        completions.push(ActionCompletion {
                            kind: action.kind,
                            skill_name: action.skill_name.clone(),
                            success,
                            reason: if success {
                                None
                            } else {
                                Some(format!(
                                    "exit {}; stderr={}",
                                    exit_code_label(code),
                                    action.stderr_summary()
                                ))
                            },
                        });
                    }
                    RunnerEvent::Failed(reason) => {
                        outputs.push(format!(
                            "[action] Failed {}: {}; source={}; argv={}; stderr={}",
                            action.title,
                            reason,
                            action.source_label,
                            action.command_lines.join(" && "),
                            action.stderr_summary()
                        ));
                        completions.push(ActionCompletion {
                            kind: action.kind,
                            skill_name: action.skill_name.clone(),
                            success: false,
                            reason: Some(reason),
                        });
                    }
                }
            }
        }

        let had_events = !outputs.is_empty();
        self.running_actions.retain(|action| !action.is_finished());

        for output in outputs {
            self.push_output(&output);
        }
        for completion in completions {
            self.apply_action_completion(completion);
        }

        had_events
    }

    fn apply_action_completion(&mut self, completion: ActionCompletion) {
        if !completion.success {
            if let Some(skill) = self
                .skills
                .iter_mut()
                .find(|skill| skill.name == completion.skill_name)
            {
                skill.state = SkillState::Error;
                skill.error = completion.reason;
            }
            return;
        }

        match completion.kind {
            ActionKind::Install => {
                if let Some(skill) = self
                    .skills
                    .iter_mut()
                    .find(|skill| skill.name == completion.skill_name)
                {
                    skill.state = SkillState::Installed;
                    skill.metadata.installed = true;
                    skill.error = None;
                }
            }
            ActionKind::UpdateSelected => {
                if let Some(skill) = self
                    .skills
                    .iter_mut()
                    .find(|skill| skill.name == completion.skill_name)
                {
                    skill.state = SkillState::Ready;
                    skill.update = Some("current".to_string());
                    skill.error = None;
                }
            }
            ActionKind::UpdateAll => {
                for skill in &mut self.skills {
                    if skill.state == SkillState::UpdateAvailable {
                        skill.state = SkillState::Ready;
                        skill.update = Some("current".to_string());
                        skill.error = None;
                    }
                }
            }
            ActionKind::Remove => {
                if let Some(skill) = self
                    .skills
                    .iter_mut()
                    .find(|skill| skill.name == completion.skill_name)
                {
                    skill.state = SkillState::RemoteOnly;
                    skill.metadata.installed = false;
                    skill.update = Some("remote".to_string());
                    skill.error = None;
                }
            }
            ActionKind::OpenPath | ActionKind::CopyPath => {}
        }
    }

    fn push_output(&mut self, message: &str) {
        self.output.push(message.to_string());
        let overflow = self.output.len().saturating_sub(Self::OUTPUT_LIMIT);
        if overflow > 0 {
            self.output.drain(0..overflow);
        }
    }

    const OUTPUT_LIMIT: usize = 16;
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

fn active_space(config: &AppConfig) -> Option<&SpaceSettings> {
    let active = config.active_space.as_ref()?;
    config
        .spaces
        .iter()
        .find(|space| space.enabled && &space.id == active)
}

fn active_space_index(config: &AppConfig) -> usize {
    let Some(active) = config.active_space.as_ref() else {
        return 0;
    };

    config
        .spaces
        .iter()
        .filter(|space| space.enabled)
        .position(|space| &space.id == active)
        .map(|index| index + 1)
        .unwrap_or(0)
}

#[derive(Debug)]
enum RemoteSpaceLoadError {
    NoActiveSpace,
    NoEnabledSource,
    SearchFailed {
        space_label: String,
        error: crate::source::SourceError,
    },
}

fn fetch_active_space_records<C: HttpClient>(
    config: &AppConfig,
    client: C,
) -> Result<(String, Vec<SkillRecord>), RemoteSpaceLoadError> {
    let Some(space) = active_space(config) else {
        return Err(RemoteSpaceLoadError::NoActiveSpace);
    };
    let Some(source) = active_agentbuddy_source(config) else {
        return Err(RemoteSpaceLoadError::NoEnabledSource);
    };

    let mut source_config = agentbuddy_config(source);
    source_config.scope = space.scope.clone();
    let adapter = AgentBuddyMarketplaceAdapter::new(source_config, client);
    let mut query = SourceQuery::new("");
    query.scope = Some(space.scope.clone());
    query.order_by = SourceOrder::StarDesc;

    adapter
        .search(&query)
        .map(|records| (space.label.clone(), records))
        .map_err(|error| RemoteSpaceLoadError::SearchFailed {
            space_label: space.label.clone(),
            error,
        })
}

fn fetch_available_spaces<C: HttpClient>(
    config: &AppConfig,
    client: C,
) -> Result<Vec<SpaceSettings>, RemoteSpaceLoadError> {
    let Some(source) = active_agentbuddy_source(config) else {
        return Err(RemoteSpaceLoadError::NoEnabledSource);
    };

    let adapter = AgentBuddyMarketplaceAdapter::new(agentbuddy_config(source), client);
    adapter
        .search_spaces(&config.space_search_query)
        .map(|spaces| {
            spaces
                .into_iter()
                .filter(selectable_space)
                .map(space_settings_from_agentbuddy)
                .collect()
        })
        .map_err(|error| RemoteSpaceLoadError::SearchFailed {
            space_label: config.space_search_query.clone(),
            error,
        })
}

fn active_agentbuddy_source(config: &AppConfig) -> Option<&SourceSettings> {
    config
        .sources
        .iter()
        .find(|source| source.enabled && source.kind == SourceKind::AgentBuddy)
}

fn agentbuddy_config(source: &SourceSettings) -> AgentBuddyMarketplaceConfig {
    let default_source_config = AgentBuddyMarketplaceConfig::default();
    let portal_url = source
        .portal_url
        .clone()
        .unwrap_or_else(|| default_source_config.portal_url.clone());
    AgentBuddyMarketplaceConfig {
        id: source.name.clone(),
        api_base: source.url.trim_end_matches('/').to_string(),
        portal_url,
        ..default_source_config
    }
}

fn selectable_space(space: &AgentBuddySpace) -> bool {
    space.can_view && space.can_download && space.package_count > 0
}

fn space_settings_from_agentbuddy(space: AgentBuddySpace) -> SpaceSettings {
    SpaceSettings {
        id: space.id,
        label: space.label,
        scope: space.scope,
        url: space.url,
        package_count: space.package_count,
        enabled: true,
    }
}

fn merge_remote_records(skills: &mut Vec<SkillRecord>, records: Vec<SkillRecord>) -> usize {
    let before = skills.len();
    for record in records {
        if skills
            .iter()
            .any(|skill| same_source_record(skill, &record))
        {
            continue;
        }
        skills.push(record);
    }
    skills.len().saturating_sub(before)
}

fn same_source_record(left: &SkillRecord, right: &SkillRecord) -> bool {
    match (
        left.metadata.source_id.as_deref(),
        right.metadata.source_id.as_deref(),
    ) {
        (Some(left_id), Some(right_id)) => return left_id == right_id,
        (Some(_), None) | (None, Some(_)) => {}
        (None, None) => {}
    }

    if left.source == right.source && left.path == right.path {
        return true;
    }

    left.source == right.source
        && left.metadata.source_status == right.metadata.source_status
        && left.name == right.name
}

fn is_local_source(source: &Source) -> bool {
    matches!(source, Source::LocalGit | Source::LocalArchive)
}

fn merge_spaces(spaces: &mut Vec<SpaceSettings>, discovered: Vec<SpaceSettings>) -> usize {
    let before = spaces.len();
    for space in discovered {
        match spaces.iter_mut().find(|existing| existing.id == space.id) {
            Some(existing) => *existing = space,
            None => spaces.push(space),
        }
    }
    spaces.sort_by(|left, right| {
        right
            .package_count
            .cmp(&left.package_count)
            .then_with(|| left.label.cmp(&right.label))
    });
    spaces.len().saturating_sub(before)
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
pub(crate) struct SpacePickerRow {
    pub label: String,
    pub value: String,
    pub scope: String,
    pub active: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SettingsState {
    open: bool,
    selected: usize,
    draft: AppConfig,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SpacePickerState {
    open: bool,
    selected: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ActionCompletion {
    kind: ActionKind,
    skill_name: String,
    success: bool,
    reason: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ActionConfirmation {
    pub plan: ActionPlan,
    pub input: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SettingsAction {
    Theme,
    Language,
    CacheTtl,
    CacheClear,
    Safety,
    Space,
    SpaceChoice(usize),
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

impl SpacePickerState {
    fn closed() -> Self {
        Self {
            open: false,
            selected: 0,
        }
    }

    fn open(selected: usize) -> Self {
        Self {
            open: true,
            selected,
        }
    }
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn exit_code_label(code: Option<i32>) -> String {
    code.map_or_else(|| "signal".to_string(), |code| code.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agentbuddy_marketplace::{HttpResponse, MockHttpClient},
        config::{Language, SafetySettings, SpaceSettings, ThemeName, load_or_default},
        i18n::I18nKey,
        runner::{ActionRunner, MockActionRunner, RunnerEvent},
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
        app.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(app.sort_column(), SortColumn::Source);
        app.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(app.sort_column(), SortColumn::Scope);
        app.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(app.sort_column(), SortColumn::State);
        app.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(app.sort_column(), SortColumn::Risk);
        app.handle_key(KeyEvent::from(KeyCode::Char('S')));
        assert_eq!(app.sort_column(), SortColumn::Update);

        assert!(app.sort_ascending());
        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert!(app.space_picker_open());
        assert_eq!(app.sort_column(), SortColumn::Update);
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
    fn source_filter_key_cycles_available_sources_and_clamps_selection() {
        let mut app = App::default();
        app.selected = app.visible_skills().len() - 1;

        app.handle_key(KeyEvent::from(KeyCode::Char('f')));

        assert_eq!(app.filters.source, Some(Source::Curated));
        assert_eq!(app.selected_index(), 0);
        assert_eq!(app.visible_skills().len(), 1);
        assert_eq!(app.visible_skills()[0].1.name, "code-review");
        assert_eq!(app.source_filter_label(), "curated");
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Source -> curated"))
        );

        for _ in 0..5 {
            app.handle_key(KeyEvent::from(KeyCode::Char('f')));
        }

        assert_eq!(app.filters.source, None);
        assert_eq!(app.source_filter_label(), "all");
    }

    #[test]
    fn local_filter_key_toggles_local_source_only() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('i')));

        assert!(app.filters.local_only);
        assert_eq!(app.local_filter_label(), "local");
        assert!(
            app.visible_skills()
                .iter()
                .all(|(_, skill)| is_local_source(&skill.source))
        );
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Local skills -> on"))
        );

        app.handle_key(KeyEvent::from(KeyCode::Char('i')));

        assert!(!app.filters.local_only);
        assert_eq!(app.local_filter_label(), "all scopes");
        assert_eq!(app.visible_skills().len(), fixture_skills().len());
    }

    #[test]
    fn taproom_style_filter_keys_toggle_state_and_reset_all() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('o')));
        assert_eq!(app.filters.state, Some(SkillState::UpdateAvailable));
        assert!(
            app.visible_skills()
                .iter()
                .all(|(_, skill)| skill.state == SkillState::UpdateAvailable)
        );

        app.handle_key(KeyEvent::from(KeyCode::Char('v')));
        assert_eq!(app.filters.state, Some(SkillState::Active));
        assert!(
            app.visible_skills()
                .iter()
                .all(|(_, skill)| skill.state == SkillState::Active)
        );

        app.handle_key(KeyEvent::from(KeyCode::Char('a')));
        assert_eq!(app.filters, FilterState::default());
        assert_eq!(app.input_mode(), InputMode::Normal);
        assert_eq!(app.visible_skills().len(), fixture_skills().len());
    }

    #[test]
    fn enter_selects_current_skill_and_focuses_details() {
        let mut app = App::default();
        let selected = app.selected_skill().unwrap().name.clone();

        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert_eq!(app.focus(), FocusArea::Details);
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("[select]") && line.contains(&selected))
        );

        app.handle_key(KeyEvent::from(KeyCode::Char('h')));
        assert_eq!(app.focus(), FocusArea::Table);

        app.handle_key(KeyEvent::from(KeyCode::Char('l')));
        assert_eq!(app.focus(), FocusArea::Details);
    }

    #[test]
    fn space_key_opens_picker_and_enter_applies_selected_space() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig {
                    spaces: vec![SpaceSettings::qianchuan_fe()],
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        app.handle_key(KeyEvent::from(KeyCode::Char('s')));
        assert!(app.space_picker_open());
        assert_eq!(app.space_picker_selected(), 0);
        assert_eq!(app.space_picker_rows().len(), 2);

        app.handle_key(KeyEvent::from(KeyCode::Char('j')));
        assert_eq!(app.space_picker_selected(), 1);
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(!app.space_picker_open());
        assert_eq!(app.active_space_label(), Some("qianchuan/fe"));
        let loaded = load_or_default(path);
        assert_eq!(loaded.config.active_space.as_deref(), Some("qianchuan-fe"));
    }

    #[test]
    fn refresh_key_reloads_inventory_without_changing_command_mode() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('R')));

        assert_eq!(app.input_mode(), InputMode::Normal);
        assert!(app.output().iter().any(|line| line.contains("Refreshed")));
    }

    #[test]
    fn streaming_output_is_bounded() {
        let mut app = App::default();

        for _ in 0..512 {
            app.tick();
        }

        assert!(app.output().len() <= App::OUTPUT_LIMIT);
        assert_eq!(app.output().len(), app.output().iter().count());
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
    fn settings_space_selects_discovered_space_without_startup_args() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig {
                    spaces: vec![SpaceSettings::qianchuan_fe()],
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        assert_eq!(app.active_space_label(), None);

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        move_to_setting(&mut app, "Space");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert_eq!(app.settings.draft.active_space, None);
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Space -> none"))
        );

        move_to_setting(&mut app, "Space qianchuan/fe");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            app.settings.draft.active_space.as_deref(),
            Some("qianchuan-fe")
        );

        move_to_setting(&mut app, "Save");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        let loaded = load_or_default(path);
        assert_eq!(loaded.config.active_space.as_deref(), Some("qianchuan-fe"));
    }

    #[test]
    fn active_space_fetch_uses_configured_group_and_agentbuddy_source() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"count":15,"data":[{"identifier":"skills:skills.byted.org/qianchuan/fe/qc-component-workflow","name":"qc-component-workflow","description":"component workflow","newest_version":{"version":"1.0.1"},"namespace":"skills.byted.org/qianchuan/fe","stars":2,"download_total":96,"no_permission":false}]}"#,
        ))]);
        let config = AppConfig {
            active_space: Some("qianchuan-fe".to_string()),
            spaces: vec![SpaceSettings::qianchuan_fe()],
            ..AppConfig::default()
        };

        let (space_label, records) =
            fetch_active_space_records(&config, &client).expect("active space records should load");

        assert_eq!(space_label, "qianchuan/fe");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "qc-component-workflow");
        assert_eq!(records[0].source, Source::InternalRegistry);
        assert_eq!(records[0].state, SkillState::Installable);
        assert_eq!(records[0].update.as_deref(), Some("96"));
        assert!(
            records[0]
                .tags
                .contains(&"space:skills.byted.org/qianchuan/fe".to_string())
        );
        assert_eq!(
            client.urls()[0],
            "https://artifact-api.byted.org/api/v1/package/search/skills/?group=skills.byted.org%2Fqianchuan%2Ffe&page=1&page_size=30&order_by=star_desc"
        );
    }

    #[test]
    fn active_space_fetch_requires_enabled_agentbuddy_source() {
        let mut config = AppConfig {
            active_space: Some("qianchuan-fe".to_string()),
            spaces: vec![SpaceSettings::qianchuan_fe()],
            ..AppConfig::default()
        };
        config.sources[0].enabled = false;
        let client = MockHttpClient::default();

        let error = fetch_active_space_records(&config, &client).unwrap_err();

        assert!(matches!(error, RemoteSpaceLoadError::NoEnabledSource));
        assert!(client.urls().is_empty());
    }

    #[test]
    fn active_space_limits_visible_list_to_that_space() {
        let mut skills = fixture_skills();
        skills.push(remote_space_skill("taproom"));
        skills.push(remote_space_skill("qc-component-workflow"));

        let mut other_space = remote_space_skill("other-team-skill");
        other_space.tags = vec!["space:skills.byted.org/other/team".to_string()];
        other_space.metadata.source_id =
            Some("skills:skills.byted.org/other/team/other-team-skill".to_string());
        skills.push(other_space);

        let app = App::from_skills_with_config(
            skills,
            LoadedConfig {
                path: PathBuf::from("config.toml"),
                config: AppConfig {
                    active_space: Some("qianchuan-fe".to_string()),
                    spaces: vec![SpaceSettings::qianchuan_fe()],
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        let visible_names = app
            .visible_skills()
            .iter()
            .map(|(_, skill)| skill.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(visible_names, vec!["qc-component-workflow", "taproom"]);
        assert!(app.visible_skills().iter().all(|(_, skill)| {
            skill
                .tags
                .contains(&"space:skills.byted.org/qianchuan/fe".to_string())
        }));
    }

    #[test]
    fn space_discovery_keeps_only_selectable_agentbuddy_spaces() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":[{"name":"skills.byted.org/qianchuan/pc","package_count":0,"user_permission":{"view":false,"download":false}},{"name":"skills.byted.org/qianchuan_marketing/ai","package_count":4,"user_permission":{"view":false,"download":false}},{"name":"skills.byted.org/qianchuan/fe","package_count":15,"user_permission":{"view":true,"download":true}}]}"#,
        ))]);

        let spaces = fetch_available_spaces(&AppConfig::default(), &client)
            .expect("space discovery should parse AgentBuddy response");

        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0].id, "qianchuan-fe");
        assert_eq!(spaces[0].label, "qianchuan/fe");
        assert_eq!(spaces[0].package_count, 15);
        assert_eq!(
            client.urls()[0],
            "https://artifact-api.byted.org/api/v1/group/search/skills?q=qianchuan&page=1&page_size=30"
        );
    }

    #[test]
    fn remote_space_records_merge_without_clobbering_same_named_local_skills() {
        let mut skills = fixture_skills();
        let mut duplicate = skills[0].clone();
        duplicate.source = Source::InternalRegistry;
        duplicate.state = SkillState::Installable;
        duplicate.path = PathBuf::from("agentbuddy://skills:skills.byted.org/qianchuan/fe/taproom");
        duplicate.metadata.source_id =
            Some("skills:skills.byted.org/qianchuan/fe/taproom".to_string());
        duplicate.metadata.source_status = Some("bytedance-agentbuddy".to_string());
        duplicate.tags = vec!["space:skills.byted.org/qianchuan/fe".to_string()];
        let mut remote = duplicate.clone();
        remote.name = "qc-component-workflow".to_string();
        remote.path = PathBuf::from(
            "agentbuddy://skills:skills.byted.org/qianchuan/fe/qc-component-workflow",
        );
        remote.metadata.source_id =
            Some("skills:skills.byted.org/qianchuan/fe/qc-component-workflow".to_string());

        let added = merge_remote_records(&mut skills, vec![duplicate, remote]);

        assert_eq!(added, 2);
        assert!(skills.iter().any(|skill| skill.name == "taproom"));
        assert!(
            skills
                .iter()
                .any(|skill| skill.name == "qc-component-workflow")
        );
        assert_eq!(
            skills
                .iter()
                .filter(|skill| skill.name == "taproom")
                .count(),
            2
        );
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
        assert!(source.last_status.contains("Api:warn"));
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("no remote request"))
        );
    }

    #[test]
    fn settings_add_source_restores_agentbuddy_when_missing() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: path.clone(),
                config: AppConfig {
                    sources: vec![SourceSettings::custom(1)],
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        app.handle_key(KeyEvent::from(KeyCode::Char(',')));
        move_to_setting(&mut app, "Sources");
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        move_to_setting(&mut app, "Save");
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        let loaded = load_or_default(path);
        assert!(
            loaded
                .config
                .sources
                .iter()
                .any(|source| source.name == "bytedance-agentbuddy")
        );
    }

    #[test]
    fn install_action_requires_typed_confirmation() {
        let mut app = App::default();

        move_to_skill(&mut app, "taproom");
        app.handle_key(KeyEvent::from(KeyCode::Char('t')));

        assert!(app.pending_action().is_some());
        assert_eq!(
            app.pending_action().unwrap().plan.confirmation_token,
            Some("INSTALL")
        );
        assert!(
            app.pending_action()
                .unwrap()
                .plan
                .command_lines()
                .iter()
                .any(|line| line == "skillroom install taproom")
        );

        for character in "NOPE".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(app.pending_action().is_some());
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("requires typed INSTALL"))
        );

        while !app.pending_action().unwrap().input.is_empty() {
            app.handle_key(KeyEvent::from(KeyCode::Backspace));
        }
        for character in "INSTALL".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Enter));

        assert!(app.pending_action().is_none());
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Started Install taproom"))
        );

        app.tick();
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("[run] Install taproom"))
        );
        app.tick();
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("[stdout] mock runner accepted argv"))
        );
        app.tick();
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Finished Install taproom"))
        );
        assert_eq!(app.selected_skill().unwrap().state, SkillState::Installed);
    }

    #[test]
    fn update_all_action_only_queues_trusted_updateable_skills() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('U')));

        let confirmation = app.pending_action().unwrap();
        assert_eq!(confirmation.plan.confirmation_token, Some("UPDATE"));
        assert_eq!(confirmation.plan.commands.len(), 1);
        assert_eq!(
            confirmation.plan.commands[0].display_line(),
            "skillroom update code-review"
        );
        assert!(
            confirmation
                .plan
                .skipped
                .iter()
                .any(|line| line.contains("taproom skipped"))
        );
    }

    #[test]
    fn remove_action_is_guarded_for_real_home_path_under_tests() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));

        assert!(app.pending_action().is_none());
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("remove blocked under test mode"))
        );
    }

    #[test]
    fn open_and_copy_path_actions_do_not_require_confirmation() {
        let mut app = App::default();

        app.handle_key(KeyEvent::from(KeyCode::Char('h')));
        app.handle_key(KeyEvent::from(KeyCode::Char('y')));

        assert!(app.pending_action().is_none());
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Started Open path"))
        );
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("Started Copy path"))
        );

        app.tick();
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("[run] Open path"))
        );
        app.tick();
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("[run] Copy path"))
        );
    }

    #[test]
    fn running_action_lock_blocks_duplicate_write_for_same_skill() {
        let mut app = App::default();

        move_to_skill(&mut app, "taproom");
        app.handle_key(KeyEvent::from(KeyCode::Char('t')));
        for character in "INSTALL".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        app.handle_key(KeyEvent::from(KeyCode::Char('u')));

        assert!(app.pending_action().is_none());
        assert!(
            app.output()
                .iter()
                .any(|line| line.contains("already has a running write operation"))
        );
    }

    #[test]
    fn failed_runner_preserves_stderr_source_argv_and_marks_skill_error() {
        let mut app = App {
            runner: ActionRunner::Mock(MockActionRunner::new(vec![
                RunnerEvent::Stderr("permission denied".to_string()),
                RunnerEvent::Finished { code: Some(2) },
            ])),
            ..App::default()
        };

        move_to_skill(&mut app, "taproom");
        app.handle_key(KeyEvent::from(KeyCode::Char('t')));
        for character in "INSTALL".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(character)));
        }
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        app.tick();
        app.tick();
        app.tick();

        assert!(app.output().iter().any(|line| {
            line.contains("Failed Install taproom")
                && line.contains("source=local/git")
                && line.contains("argv=skillroom install taproom")
                && line.contains("stderr=permission denied")
        }));
        let skill = app.selected_skill().unwrap();
        assert_eq!(skill.state, SkillState::Error);
        assert!(
            skill
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("permission denied")
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

    fn remote_space_skill(name: &str) -> SkillRecord {
        let mut skill = fixture_skills()[0].clone();
        skill.name = name.to_string();
        skill.source = Source::InternalRegistry;
        skill.scope = SkillScope::Global;
        skill.state = SkillState::Installable;
        skill.version = Some("1.0.0".to_string());
        skill.update = Some("10".to_string());
        skill.path = PathBuf::from(format!(
            "agentbuddy://skills:skills.byted.org/qianchuan/fe/{name}"
        ));
        skill.tags = vec!["space:skills.byted.org/qianchuan/fe".to_string()];
        skill.metadata.source_id = Some(format!("skills:skills.byted.org/qianchuan/fe/{name}"));
        skill.metadata.installable = true;
        skill.metadata.installed = false;
        skill.metadata.source_status = Some("bytedance-agentbuddy".to_string());
        skill
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

    fn move_to_skill(app: &mut App, name: &str) {
        let target = app
            .visible_skills()
            .iter()
            .position(|(_, skill)| skill.name == name)
            .unwrap_or_else(|| panic!("missing skill {name}"));

        while app.selected_index() < target {
            app.handle_key(KeyEvent::from(KeyCode::Down));
        }
        while app.selected_index() > target {
            app.handle_key(KeyEvent::from(KeyCode::Up));
        }
    }
}
