use ratatui::{
    Frame,
    layout::{Alignment, Constraint},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::{
    app::{App, FocusArea, InputMode, ListMode, SourcePickerRow, SpaceListRow},
    i18n::I18nKey,
    layout::{AppLayout, too_small_message},
    skill::{RiskLevel, SkillState},
    theme::ThemePalette,
};

pub fn render(app: &App, frame: &mut Frame<'_>) {
    let area = frame.area();
    let Some(layout) = AppLayout::calculate(area) else {
        frame.render_widget(
            Paragraph::new(too_small_message(area)).alignment(Alignment::Center),
            area,
        );
        return;
    };

    let theme = app.theme();

    render_search(app, frame, layout.search, theme);
    render_filters(app, frame, layout.filters, theme);
    render_body(app, frame, &layout, theme);
    render_stats(app, frame, layout.stats, theme);
    if layout.output.height > 0 {
        render_output(app, frame, layout.output, theme);
    }
    render_help(app, frame, layout.help, theme);

    if app.show_help() {
        render_help_overlay(app, frame, area, theme);
    }

    if app.settings_open() {
        render_settings(app, frame, area, theme);
    }

    if app.source_picker_open() {
        render_source_picker(app, frame, area, theme);
    }

    if app.pending_action().is_some() {
        render_action_confirmation(app, frame, area, theme);
    }
}

fn render_body(app: &App, frame: &mut Frame<'_>, layout: &AppLayout, theme: ThemePalette) {
    match app.zoomed_focus() {
        Some(FocusArea::Table) => render_table(app, frame, layout.body, theme),
        Some(FocusArea::Details) => render_details(app, frame, layout.body, theme),
        Some(FocusArea::Search) => render_search(app, frame, layout.body, theme),
        Some(FocusArea::Filters) => render_filters(app, frame, layout.body, theme),
        Some(FocusArea::Settings) | None => {
            render_table(app, frame, layout.table, theme);
            render_details(app, frame, layout.details, theme);
        }
    }
}

pub(crate) fn render_loading(frame: &mut Frame<'_>, phase: usize, visible_steps: usize) {
    let area = frame.area();
    let theme = crate::theme::ThemeRegistry::get(crate::config::ThemeName::TokyoNight);
    let popup = centered_rect(area, 84, 66);
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = frames[phase % frames.len()];
    let steps = [
        ("1 ", "scan configured skill roots"),
        ("2 ", "discover AgentBuddy Space list"),
        ("3 ", "build dashboard; wait for selection"),
    ];
    let mut lines = vec![
        Line::from(Span::styled("Skillroom", theme.title())),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{spinner} "), theme.title()),
            Span::styled("Loading local skills", theme.value()),
            Span::raw("  "),
            Span::styled("in progress", theme.success()),
        ]),
        Line::from(""),
    ];
    for (index, (number, label)) in steps.into_iter().enumerate() {
        if index < visible_steps {
            lines.push(Line::from(vec![
                Span::styled(number, theme.title()),
                Span::styled(label, theme.muted()),
            ]));
        } else {
            lines.push(Line::from(""));
        }
    }
    lines.extend([
        Line::from(""),
        Line::from(vec![
            Span::styled("<q> ", theme.info()),
            Span::styled("quit  ", theme.muted()),
            Span::styled("<ctrl-c> ", theme.info()),
            Span::styled("quit", theme.muted()),
        ]),
    ]);

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block("Loading", true, theme))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_search(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let line = Line::from(vec![
        Span::styled(" / ", theme.info()),
        search_prompt(app, theme),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .style(theme.value())
            .block(plain_block(app.focus() == FocusArea::Search, theme))
            .alignment(Alignment::Left),
        area,
    );
}

fn search_prompt(app: &App, theme: ThemePalette) -> Span<'static> {
    match app.input_mode() {
        InputMode::Normal if app.list_mode() == ListMode::Spaces => {
            Span::styled("Search spaces...", theme.muted())
        }
        InputMode::Normal => Span::styled("Search skills...", theme.muted()),
        InputMode::Search if app.search_query().is_empty() => Span::styled("", theme.info()),
        InputMode::Search => Span::styled(app.search_query().to_string(), theme.info()),
    }
}

fn render_filters(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    frame.render_widget(
        Paragraph::new(filter_summary(app))
            .style(theme.value())
            .block(titled_block(
                "Filters",
                app.focus() == FocusArea::Filters,
                theme,
            )),
        area,
    );
}

fn filter_source_label(app: &App) -> String {
    app.filters()
        .source
        .as_ref()
        .map(|source| source.label().to_string())
        .unwrap_or_else(|| app.text(I18nKey::ValueAllSources).to_string())
}

fn filter_scope_label(app: &App) -> &'static str {
    if app.filters().local_only {
        app.text(I18nKey::ValueLocalOnly)
    } else {
        app.text(I18nKey::ValueAllScopes)
    }
}

fn has_active_filters(app: &App) -> bool {
    let filters = app.filters();
    !app.search_query().is_empty()
        || filters.source.is_some()
        || filters.local_only
        || filters.scope.is_some()
        || filters.state.is_some()
        || filters.risk.is_some()
        || filters.update.is_some()
}

fn filter_summary(app: &App) -> Line<'static> {
    let mut spans = Vec::new();
    let filters = app.filters();

    if !app.search_query().is_empty() {
        push_filter_part(&mut spans, "Search", app.search_query().to_string());
    }
    if filters.source.is_some() {
        push_filter_part(&mut spans, "Source", app.source_filter_label());
    }
    if filters.local_only {
        push_filter_part(
            &mut spans,
            "Source",
            app.text(I18nKey::ValueLocalOnly).to_string(),
        );
    }
    if filters.scope.is_some() {
        push_filter_part(
            &mut spans,
            "Scope",
            app.filters()
                .scope
                .map(|scope| scope.label().to_string())
                .unwrap_or_default(),
        );
    }
    if filters.state.is_some() {
        push_filter_part(
            &mut spans,
            "State",
            filters
                .state
                .map(|state| state.label().to_string())
                .unwrap_or_default(),
        );
    }
    if filters.risk.is_some() {
        push_filter_part(
            &mut spans,
            "Risk",
            filters
                .risk
                .map(|risk| risk.label().to_string())
                .unwrap_or_default(),
        );
    }
    if filters.update.is_some() {
        push_filter_part(
            &mut spans,
            "Update",
            filters.update.as_ref().cloned().unwrap_or_default(),
        );
    }

    if spans.is_empty() {
        Line::from("None")
    } else {
        Line::from(spans)
    }
}

fn push_filter_part(spans: &mut Vec<Span<'static>>, label: &'static str, value: String) {
    if !spans.is_empty() {
        spans.push(Span::raw(" | "));
    }
    spans.push(Span::raw(format!("{label} ")));
    spans.push(Span::raw(value));
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TableColumn {
    Icon,
    Name,
    Source,
    Scope,
    State,
    Risk,
    Update,
}

impl TableColumn {
    fn visible(area_width: u16) -> Vec<Self> {
        if area_width < 58 {
            vec![
                Self::Icon,
                Self::Name,
                Self::Source,
                Self::Scope,
                Self::State,
            ]
        } else if area_width < 82 {
            vec![
                Self::Icon,
                Self::Name,
                Self::Source,
                Self::Scope,
                Self::State,
                Self::Update,
            ]
        } else {
            vec![
                Self::Icon,
                Self::Name,
                Self::Source,
                Self::Scope,
                Self::State,
                Self::Risk,
                Self::Update,
            ]
        }
    }

    fn title(self, app: &App) -> String {
        if self == Self::Icon {
            return String::new();
        }

        let title = match self {
            Self::Icon => "",
            Self::Name => app.text(I18nKey::ColumnName),
            Self::Source => app.text(I18nKey::ColumnSource),
            Self::Scope => app.text(I18nKey::ColumnScope),
            Self::State => app.text(I18nKey::ColumnState),
            Self::Risk => app.text(I18nKey::ColumnRisk),
            Self::Update => app.text(I18nKey::ColumnUpdate),
        };
        if self.sort_column() == app.sort_column() {
            let marker = if app.sort_ascending() { "↑" } else { "↓" };
            format!("{marker} {title}")
        } else {
            title.to_string()
        }
    }

    fn sort_column(self) -> crate::app::SortColumn {
        match self {
            Self::Icon => crate::app::SortColumn::Name,
            Self::Name => crate::app::SortColumn::Name,
            Self::Source => crate::app::SortColumn::Source,
            Self::Scope => crate::app::SortColumn::Scope,
            Self::State => crate::app::SortColumn::State,
            Self::Risk => crate::app::SortColumn::Risk,
            Self::Update => crate::app::SortColumn::Update,
        }
    }

    fn constraint(self, area_width: u16) -> Constraint {
        match self {
            Self::Icon => Constraint::Length(2),
            Self::Name if area_width < 58 => Constraint::Percentage(31),
            Self::Name => Constraint::Percentage(27),
            Self::Source => Constraint::Percentage(24),
            Self::Scope => Constraint::Length(8),
            Self::State => Constraint::Length(10),
            Self::Risk => Constraint::Length(8),
            Self::Update => Constraint::Min(8),
        }
    }

    fn cell(self, skill: &crate::skill::SkillRecord, theme: ThemePalette) -> Cell<'static> {
        match self {
            Self::Icon => Cell::from(skill_icon(skill, theme)),
            Self::Name => Cell::from(skill.name.clone()),
            Self::Source => Cell::from(skill.source.label().to_string()),
            Self::Scope => Cell::from(skill.scope.label()),
            Self::State => Cell::from(state_line(skill.state, theme)),
            Self::Risk => Cell::from(risk_line(skill.risk, theme)),
            Self::Update => Cell::from(skill.update_label().to_string()),
        }
    }
}

fn render_table(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    match app.list_mode() {
        ListMode::Skills => render_skills_table(app, frame, area, theme),
        ListMode::Spaces => render_spaces_table(app, frame, area, theme),
    }
}

fn render_skills_table(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let columns = TableColumn::visible(area.width);
    let header = Row::new(columns.iter().map(|column| {
        Cell::from(Span::styled(
            column.title(app),
            theme.label().add_modifier(Modifier::BOLD),
        ))
    }))
    .bottom_margin(1);

    let visible = app.visible_skills();
    let rows = visible.into_iter().enumerate().map(|(index, (_, skill))| {
        let style = if index == app.selected_index() {
            theme.selected()
        } else {
            theme.value()
        };

        Row::new(columns.iter().map(|column| column.cell(skill, theme))).style(style)
    });

    let constraints = columns
        .iter()
        .map(|column| column.constraint(area.width))
        .collect::<Vec<_>>();

    let table = Table::new(rows, constraints)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(theme.selected())
        .block(titled_block(
            app.text(I18nKey::PanelSkills),
            app.focus() == FocusArea::Table,
            theme,
        ));

    let mut state = TableState::default()
        .with_selected((!app.visible_skills().is_empty()).then_some(app.selected_index()));
    frame.render_stateful_widget(table, area, &mut state);
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SpaceColumn {
    Icon,
    Name,
    Packages,
    Scope,
    State,
}

impl SpaceColumn {
    fn visible(area_width: u16) -> Vec<Self> {
        if area_width < 58 {
            vec![Self::Icon, Self::Name, Self::Packages, Self::State]
        } else {
            vec![
                Self::Icon,
                Self::Name,
                Self::Packages,
                Self::Scope,
                Self::State,
            ]
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Icon => "",
            Self::Name => "Name",
            Self::Packages => "Skills",
            Self::Scope => "Scope",
            Self::State => "State",
        }
    }

    fn constraint(self, area_width: u16) -> Constraint {
        match self {
            Self::Icon => Constraint::Length(2),
            Self::Name if area_width < 58 => Constraint::Percentage(45),
            Self::Name => Constraint::Percentage(30),
            Self::Packages => Constraint::Length(8),
            Self::Scope => Constraint::Percentage(38),
            Self::State => Constraint::Min(10),
        }
    }

    fn cell(self, space: &SpaceListRow, theme: ThemePalette) -> Cell<'static> {
        match self {
            Self::Icon => Cell::from(space_icon(space, theme)),
            Self::Name => Cell::from(space.label.clone()),
            Self::Packages => Cell::from(space.package_count.to_string()),
            Self::Scope => Cell::from(space.scope.clone()),
            Self::State => Cell::from(space_state_line(space, theme)),
        }
    }
}

fn render_spaces_table(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let columns = SpaceColumn::visible(area.width);
    let header = Row::new(columns.iter().map(|column| {
        Cell::from(Span::styled(
            column.title(),
            theme.label().add_modifier(Modifier::BOLD),
        ))
    }))
    .bottom_margin(1);

    let visible = app.visible_spaces();
    let rows = visible.iter().enumerate().map(|(index, space)| {
        let style = if index == app.space_selected_index() {
            theme.selected()
        } else {
            theme.value()
        };

        Row::new(columns.iter().map(|column| column.cell(space, theme))).style(style)
    });

    let constraints = columns
        .iter()
        .map(|column| column.constraint(area.width))
        .collect::<Vec<_>>();

    let table = Table::new(rows, constraints)
        .header(header)
        .column_spacing(2)
        .row_highlight_style(theme.selected())
        .block(titled_block(
            "Spaces",
            app.focus() == FocusArea::Table,
            theme,
        ));

    let mut state = TableState::default()
        .with_selected((!app.visible_spaces().is_empty()).then_some(app.space_selected_index()));
    frame.render_stateful_widget(table, area, &mut state);
}

fn skill_icon(skill: &crate::skill::SkillRecord, theme: ThemePalette) -> Line<'static> {
    let (icon, style) = if skill.error.is_some()
        || matches!(
            skill.state,
            SkillState::AuthError | SkillState::SchemaError | SkillState::Error
        ) {
        ("!", theme.error().add_modifier(Modifier::BOLD))
    } else if skill.risk == RiskLevel::High {
        ("▲", theme.error().add_modifier(Modifier::BOLD))
    } else if skill.state == SkillState::UpdateAvailable {
        ("↻", theme.warning().add_modifier(Modifier::BOLD))
    } else if matches!(skill.state, SkillState::Installed | SkillState::LocalOnly)
        || skill.metadata.installed
    {
        ("◆", theme.title())
    } else if skill.metadata.installable || skill.state == SkillState::Installable {
        ("◇", theme.info())
    } else {
        ("•", theme.muted())
    };

    Line::from(Span::styled(icon, style))
}

fn space_icon(space: &SpaceListRow, theme: ThemePalette) -> Line<'static> {
    let (icon, style) = if space.active {
        ("◆", theme.title())
    } else if space.enabled {
        ("◇", theme.info())
    } else {
        ("!", theme.error().add_modifier(Modifier::BOLD))
    };

    Line::from(Span::styled(icon, style))
}

fn space_state_line(space: &SpaceListRow, theme: ThemePalette) -> Line<'static> {
    let (label, style) = space_state_label(space, theme);

    Line::from(Span::styled(label, style))
}

fn space_state_label(space: &SpaceListRow, theme: ThemePalette) -> (&'static str, Style) {
    if space.active {
        ("Active", theme.title())
    } else if space.id.is_none() {
        ("Local", theme.value())
    } else if space.enabled {
        ("Available", theme.info())
    } else {
        ("Unavailable", theme.error())
    }
}

fn render_details(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    if app.list_mode() == ListMode::Spaces {
        render_space_details(app, frame, area, theme);
        return;
    }

    let lines = match app.selected_skill() {
        Some(skill) => detail_lines(app, skill, area, theme),
        None => vec![Line::from(Span::styled(
            app.text(I18nKey::NoSkillSelected),
            theme.muted(),
        ))],
    };

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(titled_block(
                app.text(I18nKey::PanelDetails),
                app.focus() == FocusArea::Details,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn detail_lines(
    app: &App,
    skill: &crate::skill::SkillRecord,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) -> Vec<Line<'static>> {
    let density = DetailDensity::for_area(app, area);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("◇ ", theme.title()),
            Span::styled(skill.name.clone(), theme.info()),
        ]),
        Line::from(description_preview(skill, density)),
    ];

    if density == DetailDensity::Fullscreen {
        lines.extend(full_detail_lines(app, skill, theme));
        return lines;
    }

    lines.extend(compact_primary_detail_lines(app, skill, theme));

    if density >= DetailDensity::Roomy {
        lines.extend([
            Line::from(""),
            detail_section("More", theme),
            detail_pair(
                "Source/Scope: ",
                format!("{} | {}", skill.source.label(), skill.scope.label()),
                theme,
            ),
            detail_pair(
                "Space: ",
                tag_value(skill, "space:").unwrap_or("none"),
                theme,
            ),
            detail_pair("Repository: ", repository_summary(skill), theme),
            detail_pair("Updated: ", modified_summary(skill), theme),
            detail_pair("Maintainers: ", maintainers_summary(skill), theme),
            detail_pair(
                app.text(I18nKey::DetailAgents),
                agents_summary(skill),
                theme,
            ),
        ]);
    }

    if density >= DetailDensity::Spacious {
        lines.extend([
            detail_pair(
                app.text(I18nKey::DetailPath),
                skill.path.display().to_string(),
                theme,
            ),
            detail_pair(
                "Commands: ",
                compact_command_summary(skill, area.width),
                theme,
            ),
        ]);
    }

    if density >= DetailDensity::Spacious && detail_has_room_for_rows(area, lines.len(), 5) {
        lines.extend([
            Line::from(""),
            detail_section("Implementation", theme),
            detail_pair("Files: ", files_summary(skill), theme),
            detail_pair("Scripts: ", csv_or_none(&skill.scripts), theme),
            detail_pair("Tags: ", csv_or_none(&skill.tags), theme),
        ]);
    }

    if detail_has_room_for_hint(area, lines.len()) {
        lines.push(Line::from(vec![
            Span::styled("+ ", theme.title()),
            Span::styled("fullscreen details", theme.muted()),
            Span::styled("  - ", theme.title()),
            Span::styled("restore", theme.muted()),
        ]));
    }

    lines
}

fn compact_primary_detail_lines(
    app: &App,
    skill: &crate::skill::SkillRecord,
    theme: ThemePalette,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""),
        detail_section("Primary", theme),
        detail_pair(
            app.text(I18nKey::DetailVersion),
            version_summary(skill),
            theme,
        ),
        detail_pair("Author: ", author_summary(skill), theme),
        detail_pair("Signals: ", signal_summary(skill), theme),
    ];

    lines.extend([
        detail_pair(
            "State/Risk: ",
            format!("{} | {}", skill.state.label(), skill.risk.label()),
            theme,
        ),
        detail_pair(
            app.text(I18nKey::DetailActions),
            action_summary(skill),
            theme,
        ),
        detail_pair("Access: ", access_summary(skill), theme),
    ]);

    lines
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum DetailDensity {
    Compact,
    Roomy,
    Spacious,
    Fullscreen,
}

impl DetailDensity {
    fn for_area(app: &App, area: ratatui::layout::Rect) -> Self {
        if app.details_fullscreen() {
            return Self::Fullscreen;
        }

        match (area.width, area.height) {
            (48.., 34..) => Self::Spacious,
            (34.., 24..) => Self::Roomy,
            _ => Self::Compact,
        }
    }
}

fn detail_has_room_for_hint(area: ratatui::layout::Rect, used_lines: usize) -> bool {
    detail_has_room_for_rows(area, used_lines, 1)
}

fn detail_has_room_for_rows(
    area: ratatui::layout::Rect,
    used_lines: usize,
    rows_needed: usize,
) -> bool {
    usize::from(area.height.saturating_sub(2)) > used_lines + rows_needed
}

fn compact_command_summary(skill: &crate::skill::SkillRecord, area_width: u16) -> String {
    let summary = command_summary(skill);
    if area_width >= 48 {
        return summary;
    }

    truncate_text(&summary, usize::from(area_width.saturating_sub(14)).max(24))
}

fn full_detail_lines(
    app: &App,
    skill: &crate::skill::SkillRecord,
    theme: ThemePalette,
) -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        detail_section("Primary", theme),
        detail_pair(
            "Version/Signals: ",
            format!("{} | {}", version_summary(skill), signal_summary(skill)),
            theme,
        ),
        detail_pair("Author: ", author_summary(skill), theme),
        detail_pair(
            "State/Risk/Actions: ",
            format!(
                "{} | {} | {}",
                skill.state.label(),
                skill.risk.label(),
                action_summary(skill)
            ),
            theme,
        ),
        detail_pair(
            "Owner/Access: ",
            format!("{} | {}", owner_summary(skill), access_summary(skill)),
            theme,
        ),
        detail_pair(
            app.text(I18nKey::DetailAgents),
            agents_summary(skill),
            theme,
        ),
        Line::from(""),
        detail_section("More", theme),
        detail_pair(
            "Source/Scope/Space: ",
            format!(
                "{} | {} | {}",
                skill.source.label(),
                skill.scope.label(),
                tag_value(skill, "space:").unwrap_or("none")
            ),
            theme,
        ),
        detail_pair("Repository: ", repository_summary(skill), theme),
        detail_pair("Updated: ", modified_summary(skill), theme),
        detail_pair(
            app.text(I18nKey::DetailPath),
            skill.path.display().to_string(),
            theme,
        ),
        Line::from(""),
        detail_section("Audit", theme),
        detail_pair(
            "Identifier: ",
            skill.metadata.source_id.as_deref().unwrap_or("none"),
            theme,
        ),
        detail_pair("Created: ", created_summary(skill), theme),
        detail_pair("Maintainers: ", maintainers_summary(skill), theme),
        detail_pair("Permissions: ", permission_summary(skill), theme),
        detail_pair("Flags: ", flags_summary(skill), theme),
        detail_pair(
            "Files/Scripts: ",
            format!("{} | {}", files_summary(skill), csv_or_none(&skill.scripts)),
            theme,
        ),
        detail_pair("Commands: ", command_summary(skill), theme),
        detail_pair(
            "Meta/Error/Tags: ",
            format!(
                "{} | {} | {}",
                skill.metadata_label(),
                skill.error.as_deref().unwrap_or("none"),
                csv_or_none(&skill.tags)
            ),
            theme,
        ),
    ]
}

fn detail_section(title: &'static str, theme: ThemePalette) -> Line<'static> {
    Line::from(Span::styled(title, theme.title()))
}

fn detail_pair(
    label: impl Into<String>,
    value: impl Into<String>,
    theme: ThemePalette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(label.into(), theme.label()),
        Span::styled(value.into(), theme.value()),
    ])
}

fn render_space_details(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let lines = match app.selected_space_row() {
        Some(space) => vec![
            Line::from(vec![
                Span::styled("◇ ", theme.title()),
                Span::styled(space.label.clone(), theme.info()),
            ]),
            Line::from(if space.id.is_none() {
                "Local installed skills only".to_string()
            } else {
                "AgentBuddy Space".to_string()
            }),
            Line::from(""),
            Line::from(vec![
                Span::styled("Skills: ", theme.label()),
                Span::styled(space.package_count.to_string(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled("Scope: ", theme.label()),
                Span::styled(space.scope.clone(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled("URL: ", theme.label()),
                Span::styled(
                    if space.url.is_empty() {
                        "none"
                    } else {
                        &space.url
                    }
                    .to_string(),
                    theme.value(),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled("State: ", theme.label()), {
                let (label, style) = space_state_label(&space, theme);
                Span::styled(label, style)
            }]),
            Line::from(vec![
                Span::styled("Source: ", theme.label()),
                Span::styled(
                    if space.id.is_none() {
                        "local inventory"
                    } else {
                        "skills.bytedance.net"
                    },
                    theme.value(),
                ),
            ]),
        ],
        None => vec![Line::from(Span::styled("No Space selected", theme.muted()))],
    };

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(titled_block(
                app.text(I18nKey::PanelDetails),
                app.focus() == FocusArea::Details,
                theme,
            ))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn files_summary(skill: &crate::skill::SkillRecord) -> String {
    format!(
        "{} files, {} dirs, {} refs, {} assets, {} lines",
        skill.stats.files,
        skill.stats.directories,
        skill.stats.references,
        skill.stats.assets,
        skill.stats.line_count
    )
}

fn version_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut parts = vec![skill.version_label().to_string()];
    if let Some(published_by) = &skill.metadata.published_by {
        parts.push(format!("by {published_by}"));
    }
    if let Some(published_at) = &skill.metadata.published_at {
        parts.push(format!("at {published_at}"));
    }
    parts.join(" ")
}

fn description_preview(skill: &crate::skill::SkillRecord, density: DetailDensity) -> String {
    let limit = match density {
        DetailDensity::Compact => 96,
        DetailDensity::Roomy => 240,
        DetailDensity::Spacious => 360,
        DetailDensity::Fullscreen => 800,
    };
    truncate_text(&skill.description, limit)
}

fn truncate_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        truncated.push_str("...");
    }
    truncated
}

fn signal_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut parts = Vec::new();
    if let Some(stars) = skill.metadata.star_count {
        parts.push(format!("stars {stars}"));
    }
    if let Some(downloads) = tag_value(skill, "downloads:") {
        parts.push(format!("downloads {downloads}"));
    }
    if let Some(quality) = skill.metadata.quality {
        parts.push(format!("quality {quality}"));
    }
    if skill.metadata.official == Some(true) {
        parts.push("official".to_string());
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" | ")
    }
}

fn author_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut parts = Vec::new();
    if let Some(created_by) = &skill.metadata.created_by {
        parts.push(format!("created {created_by}"));
    }
    if let Some(published_by) = &skill.metadata.published_by
        && !parts.iter().any(|part| part.ends_with(published_by))
    {
        parts.push(format!("published {published_by}"));
    }
    if parts.is_empty() {
        owner_summary(skill)
    } else {
        parts.join(" | ")
    }
}

fn owner_summary(skill: &crate::skill::SkillRecord) -> String {
    if !skill.metadata.maintainers.is_empty() {
        return truncate_items(&skill.metadata.maintainers, 2);
    }
    skill
        .metadata
        .created_by
        .clone()
        .unwrap_or_else(|| "unknown".to_string())
}

fn access_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut parts = vec![
        if skill.metadata.installable || skill.state == SkillState::Installable {
            "installable".to_string()
        } else {
            "not installable".to_string()
        },
    ];
    if let Some(view) = skill.metadata.view_permission {
        parts.push(format!("view {}", bool_label(view)));
    }
    if let Some(download) = skill.metadata.download_permission {
        parts.push(format!("download {}", bool_label(download)));
    }
    parts.join(" | ")
}

fn repository_summary(skill: &crate::skill::SkillRecord) -> String {
    if let Some(git_repo) = &skill.metadata.git_repo
        && !git_repo.trim().is_empty()
    {
        return git_repo.clone();
    }
    let mut parts = Vec::new();
    if let Some(repository) = &skill.metadata.repository {
        parts.push(repository.clone());
    }
    if let Some(kind) = &skill.metadata.repository_type {
        parts.push(kind.clone());
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(" | ")
    }
}

fn modified_summary(skill: &crate::skill::SkillRecord) -> String {
    skill
        .metadata
        .modified_at
        .as_ref()
        .or(skill.metadata.published_at.as_ref())
        .cloned()
        .unwrap_or_else(|| "unknown".to_string())
}

fn created_summary(skill: &crate::skill::SkillRecord) -> String {
    match (&skill.metadata.created_by, &skill.metadata.created_at) {
        (Some(by), Some(at)) => format!("{by} at {at}"),
        (Some(by), None) => by.clone(),
        (None, Some(at)) => at.clone(),
        (None, None) => "unknown".to_string(),
    }
}

fn maintainers_summary(skill: &crate::skill::SkillRecord) -> String {
    truncate_items(&skill.metadata.maintainers, 4)
}

fn permission_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut parts = Vec::new();
    push_bool_part(&mut parts, "view", skill.metadata.view_permission);
    push_bool_part(&mut parts, "download", skill.metadata.download_permission);
    push_bool_part(&mut parts, "config", skill.metadata.config_permission);
    push_bool_part(&mut parts, "deploy", skill.metadata.deploy_permission);
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(" | ")
    }
}

fn flags_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut parts = Vec::new();
    push_bool_part(&mut parts, "private", skill.metadata.private);
    push_bool_part(&mut parts, "official", skill.metadata.official);
    push_bool_part(&mut parts, "prod", skill.metadata.production);
    push_bool_part(&mut parts, "pinned", skill.metadata.pinned);
    push_bool_part(&mut parts, "public", skill.metadata.view_public);
    push_bool_part(&mut parts, "repo_cfg", skill.metadata.repo_config_available);
    push_bool_part(&mut parts, "tracking", skill.metadata.tracking_enabled);
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(" | ")
    }
}

fn command_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut commands = Vec::new();
    commands.extend(
        skill
            .command_plan
            .install
            .iter()
            .map(|command| format!("install: {command}")),
    );
    commands.extend(
        skill
            .command_plan
            .update
            .iter()
            .map(|command| format!("update: {command}")),
    );
    commands.extend(
        skill
            .command_plan
            .remove
            .iter()
            .map(|command| format!("remove: {command}")),
    );
    if commands.is_empty() {
        "none".to_string()
    } else {
        commands.join(" | ")
    }
}

fn tag_value<'a>(skill: &'a crate::skill::SkillRecord, prefix: &str) -> Option<&'a str> {
    skill.tags.iter().find_map(|tag| tag.strip_prefix(prefix))
}

fn push_bool_part(parts: &mut Vec<String>, label: &str, value: Option<bool>) {
    if let Some(value) = value {
        parts.push(format!("{label} {}", bool_label(value)));
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn truncate_items(values: &[String], limit: usize) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    let mut items = values.iter().take(limit).cloned().collect::<Vec<_>>();
    let remaining = values.len().saturating_sub(items.len());
    if remaining > 0 {
        items.push(format!("+{remaining} more"));
    }
    items.join(", ")
}

fn csv_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn render_stats(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    if app.list_mode() == ListMode::Spaces {
        render_space_stats(app, frame, area, theme);
        return;
    }

    let total = app.skills().len();
    let visible = app.visible_skills().len();
    let local = app
        .skills()
        .iter()
        .filter(|skill| {
            matches!(
                skill.source,
                crate::skill::Source::LocalGit | crate::skill::Source::LocalArchive
            )
        })
        .count();
    let updates = app
        .skills()
        .iter()
        .filter(|skill| skill.state == SkillState::UpdateAvailable)
        .count();
    let high_risk = app
        .skills()
        .iter()
        .filter(|skill| skill.risk == RiskLevel::High)
        .count();

    let filter_status = if has_active_filters(app) {
        app.text(I18nKey::StatusActive)
    } else {
        "none"
    };
    let space = app.active_space_label().unwrap_or("none");
    let space_scope = app.active_space_scope().unwrap_or("local only");
    let filter_line = if area.width < 112 {
        Line::from(vec![
            Span::styled("Space ", theme.muted()),
            Span::styled(space.to_string(), theme.info()),
            Span::styled(" | Filter ", theme.muted()),
            Span::styled(filter_status, theme.info()),
            Span::styled(" | Source ", theme.muted()),
            Span::styled(filter_source_label(app), theme.info()),
        ])
    } else {
        Line::from(vec![
            Span::styled("Space ", theme.muted()),
            Span::styled(space.to_string(), theme.info()),
            Span::styled(" | Remote scope ", theme.muted()),
            Span::styled(space_scope.to_string(), theme.info()),
            Span::styled(" | Filter ", theme.muted()),
            Span::styled(filter_status, theme.info()),
            Span::styled(" | Source ", theme.muted()),
            Span::styled(filter_source_label(app), theme.info()),
            Span::styled(" | Skill scope ", theme.muted()),
            Span::styled(filter_scope_label(app), theme.info()),
        ])
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(visible.to_string(), theme.title()),
            Span::styled(" visible skills | ", theme.value()),
            Span::styled(total.to_string(), theme.title()),
            Span::styled(" total | ", theme.value()),
            Span::styled(local.to_string(), theme.title()),
            Span::styled(" local | ", theme.value()),
            Span::styled(updates.to_string(), theme.title()),
            Span::styled(" updates | ", theme.value()),
            Span::styled(high_risk.to_string(), theme.title()),
            Span::styled(" high risk", theme.value()),
        ]),
        filter_line,
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_space_stats(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let visible = app.visible_spaces().len();
    let total = app.space_count();
    let active = app.active_space_label().unwrap_or("local only");
    let search = if app.search_query().is_empty() {
        "none".to_string()
    } else {
        app.search_query().to_string()
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(visible.to_string(), theme.title()),
            Span::styled(" visible spaces | ", theme.value()),
            Span::styled(total.to_string(), theme.title()),
            Span::styled(" remote | Active ", theme.value()),
            Span::styled(active.to_string(), theme.info()),
        ]),
        Line::from(vec![
            Span::styled("Space search ", theme.muted()),
            Span::styled(search, theme.info()),
            Span::styled(" | Enter/l opens | h returns", theme.muted()),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn agents_summary(skill: &crate::skill::SkillRecord) -> String {
    if skill.agents.is_empty() {
        return "0".to_string();
    }

    let enabled = skill.agents_count();
    let names = skill
        .agents
        .iter()
        .map(|agent| {
            if agent.enabled {
                agent.name.clone()
            } else {
                format!("{}:off", agent.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("{enabled}/{} [{names}]", skill.agents.len())
}

fn action_summary(skill: &crate::skill::SkillRecord) -> String {
    let mut actions = Vec::new();
    if !skill.command_plan.install.is_empty() {
        actions.push("install");
    }
    if !skill.command_plan.update.is_empty() {
        actions.push("update");
    }
    if !skill.command_plan.remove.is_empty() {
        actions.push("remove");
    }

    if actions.is_empty() {
        "none".to_string()
    } else {
        actions.join(", ")
    }
}

fn render_output(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let output = app.output();
    let line_capacity = usize::from(area.height.saturating_sub(2)).max(1);
    let start = output_window_start(output.len(), line_capacity);
    let lines: Vec<Line<'static>> = app
        .output()
        .iter()
        .skip(start)
        .take(line_capacity)
        .map(|line| {
            Line::from(vec![
                Span::styled("> ", theme.muted()),
                Span::styled(line.clone(), theme.value()),
            ])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(app.text(I18nKey::PanelOutput), false, theme))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn output_window_start(total: usize, capacity: usize) -> usize {
    total.saturating_sub(capacity)
}

fn render_help(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect, theme: ThemePalette) {
    if app.list_mode() == ListMode::Spaces {
        render_space_help(frame, area, theme);
        return;
    }

    let key_style = theme.title();
    let text_style = theme.muted();
    let h_hint = if app.focus() == FocusArea::Details {
        "back"
    } else {
        "open path"
    };
    let lines = if area.width < 100 {
        vec![
            help_line(
                "General   : ",
                [
                    ("q", "quit"),
                    ("R", "refresh"),
                    ("/", "search"),
                    ("?", "help"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Navigation: ",
                [
                    ("j/↓", "down"),
                    ("k/↑", "up"),
                    ("PgUp", "prev"),
                    ("PgDn", "next"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Filter    : ",
                [
                    ("a", "all"),
                    ("f", "source"),
                    ("i", "local"),
                    ("o", "updates"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Commands  : ",
                [
                    ("enter/l", "details"),
                    ("s", "spaces"),
                    ("h", h_hint),
                    ("+", "full"),
                    ("-", "back"),
                ],
                key_style,
                text_style,
            ),
        ]
    } else {
        vec![
            help_line(
                "General   : ",
                [
                    ("q", "quit"),
                    ("R", "refresh"),
                    ("tab", "switch focus"),
                    ("/", "search"),
                    ("esc", "clear search"),
                    ("enter/l", "details"),
                    ("s", "spaces"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Navigation: ",
                [
                    ("j/↓", "cursor down"),
                    ("k/↑", "cursor up"),
                    ("PageUp", "prev page"),
                    ("PageDown", "next page"),
                    ("g", "go to top"),
                    ("G", "go to bottom"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Filter    : ",
                [
                    ("a", "all"),
                    ("f", "source"),
                    ("i", "local"),
                    ("o", "updates"),
                    ("v", "active"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Commands  : ",
                [
                    ("h", h_hint),
                    ("S", "sort"),
                    ("t", "install"),
                    ("u", "update"),
                    ("U", "update all"),
                    ("x", "remove"),
                    ("y", "copy path"),
                    ("+", "full"),
                    ("-", "back"),
                    (",", "settings"),
                ],
                key_style,
                text_style,
            ),
        ]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_space_help(frame: &mut Frame<'_>, area: ratatui::layout::Rect, theme: ThemePalette) {
    let key_style = theme.title();
    let text_style = theme.muted();
    let lines = if area.width < 100 {
        vec![
            help_line(
                "General   : ",
                [
                    ("q", "quit"),
                    ("R", "refresh"),
                    ("/", "search"),
                    ("?", "help"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Navigation: ",
                [
                    ("j/↓", "down"),
                    ("k/↑", "up"),
                    ("PgUp", "prev"),
                    ("PgDn", "next"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Commands  : ",
                [
                    ("enter/l", "open"),
                    ("h", "skills"),
                    ("s", "skills"),
                    ("+", "full"),
                    (",", "settings"),
                ],
                key_style,
                text_style,
            ),
        ]
    } else {
        vec![
            help_line(
                "General   : ",
                [
                    ("q", "quit"),
                    ("R", "refresh spaces"),
                    ("tab", "switch focus"),
                    ("/", "search"),
                    ("esc", "skills"),
                    ("enter/l", "open space"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Navigation: ",
                [
                    ("j/↓", "cursor down"),
                    ("k/↑", "cursor up"),
                    ("PageUp", "prev page"),
                    ("PageDown", "next page"),
                    ("g", "go to top"),
                    ("G", "go to bottom"),
                ],
                key_style,
                text_style,
            ),
            help_line(
                "Commands  : ",
                [
                    ("h", "skills"),
                    ("s", "skills"),
                    ("+", "full"),
                    ("-", "back"),
                    (",", "settings"),
                    ("?", "help"),
                ],
                key_style,
                text_style,
            ),
        ]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn help_line<const N: usize>(
    label: &'static str,
    parts: [(&'static str, &'static str); N],
    key_style: Style,
    text_style: Style,
) -> Line<'static> {
    let mut spans = vec![Span::styled(label, text_style)];
    for (index, (key, text)) in parts.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(key, key_style));
        spans.push(Span::styled(": ", text_style));
        spans.push(Span::styled(text, text_style));
    }
    Line::from(spans)
}

fn render_action_confirmation(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let Some(confirmation) = app.pending_action() else {
        return;
    };
    let plan = &confirmation.plan;
    let popup = centered_rect(area, 74, 80);
    let agents = if plan.agents.is_empty() {
        "none".to_string()
    } else {
        plan.agents.join(", ")
    };
    let token = plan.confirmation_token.unwrap_or("");
    let mut lines = vec![
        Line::from(vec![
            Span::styled(plan.title.clone(), theme.title()),
            Span::raw("  "),
            Span::styled(app.text(I18nKey::ConfirmEscCancels), theme.muted()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::ConfirmImpact), theme.label()),
            Span::styled(plan.impact.clone(), theme.value()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::ConfirmSource), theme.label()),
            Span::styled(plan.source.clone(), theme.value()),
            Span::raw("  "),
            Span::styled(app.text(I18nKey::ConfirmScope), theme.label()),
            Span::styled(plan.scope.label(), theme.value()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::ConfirmPath), theme.label()),
            Span::styled(plan.path.display().to_string(), theme.value()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::ConfirmAgents), theme.label()),
            Span::styled(agents, theme.value()),
        ]),
        Line::from(""),
        Line::from(Span::styled(app.text(I18nKey::ConfirmArgv), theme.label())),
    ];

    for command in plan.command_lines() {
        lines.push(Line::from(Span::styled(command, theme.value())));
    }

    if !plan.skipped.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            app.text(I18nKey::ConfirmSkipped),
            theme.label(),
        )));
        for skipped in plan.skipped.iter().take(3) {
            lines.push(Line::from(Span::styled(skipped.clone(), theme.muted())));
        }
        if plan.skipped.len() > 3 {
            lines.push(Line::from(Span::styled(
                format!("... {} more skipped", plan.skipped.len() - 3),
                theme.muted(),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(app.text(I18nKey::ConfirmToken), theme.warning()),
        Span::styled(token, theme.title()),
    ]));
    lines.push(Line::from(vec![
        Span::styled(app.text(I18nKey::ConfirmInput), theme.label()),
        Span::styled(confirmation.input.clone(), theme.info()),
    ]));

    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(app.text(I18nKey::PanelConfirm), true, theme))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_source_picker(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let popup = centered_rect(area, 64, 58);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(app.text(I18nKey::PanelSources), theme.title()),
            Span::raw("  "),
            Span::styled(app.text(I18nKey::SettingsEscCancels), theme.muted()),
        ]),
        Line::from(Span::styled(
            app.text(I18nKey::HintSourcePicker),
            theme.muted(),
        )),
        Line::from(""),
    ];

    for (index, row) in app.source_picker_rows().into_iter().enumerate() {
        lines.push(source_picker_line(
            row,
            index == app.source_picker_selected(),
            theme,
        ));
    }

    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(app.text(I18nKey::PanelSources), true, theme))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn source_picker_line(row: SourcePickerRow, selected: bool, theme: ThemePalette) -> Line<'static> {
    let marker = if selected { "> " } else { "  " };
    let active = if row.active { "◆ " } else { "◇ " };
    let line = Line::from(vec![
        Span::raw(marker),
        Span::styled(
            active,
            if row.active {
                theme.title()
            } else {
                theme.info()
            },
        ),
        Span::styled(format!("{:<22}", row.label), theme.label()),
        Span::styled(format!("{:<18}", row.value), theme.value()),
        Span::styled(row.hint, theme.muted()),
    ]);
    if selected {
        line.style(theme.selected())
    } else {
        line
    }
}

fn render_settings(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let popup = centered_rect(area, 72, 70);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(app.text(I18nKey::PanelSettings), theme.title()),
            Span::raw("  "),
            Span::styled(app.text(I18nKey::SettingsEscCancels), theme.muted()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::SettingsConfig), theme.muted()),
            Span::styled(app.config_path().display().to_string(), theme.value()),
        ]),
        Line::from(""),
    ];

    for (index, row) in app.settings_rows().into_iter().enumerate() {
        let marker = if index == app.settings_selected() {
            "> "
        } else {
            "  "
        };
        let line = Line::from(vec![
            Span::raw(marker),
            Span::styled(format!("{:<30}", row.label), theme.label()),
            Span::styled(row.value, theme.value()),
            Span::raw("  "),
            Span::styled(row.hint, theme.muted()),
        ]);
        if index == app.settings_selected() {
            lines.push(line.style(theme.selected()));
        } else {
            lines.push(line);
        }
    }

    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(app.text(I18nKey::PanelSettings), true, theme)),
        popup,
    );
}

fn render_help_overlay(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let popup = centered_rect(area, 64, 52);
    let lines = vec![
        Line::from(Span::styled(
            app.text(I18nKey::HelpNavigation),
            theme.title(),
        )),
        Line::from(Span::styled(app.text(I18nKey::HelpMove), theme.value())),
        Line::from(Span::styled(app.text(I18nKey::HelpPage), theme.value())),
        Line::from(Span::styled(
            app.text(I18nKey::HelpTopBottom),
            theme.value(),
        )),
        Line::from(Span::styled(app.text(I18nKey::HelpFocus), theme.value())),
        Line::from(Span::styled(app.text(I18nKey::HelpSort), theme.value())),
        Line::from(Span::styled(app.text(I18nKey::HelpSettings), theme.value())),
        Line::from(Span::styled(app.text(I18nKey::HelpClose), theme.value())),
        Line::from(Span::styled(app.text(I18nKey::HelpQuit), theme.value())),
    ];

    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(app.text(I18nKey::PanelHelp), true, theme))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn centered_rect(
    area: ratatui::layout::Rect,
    percent_x: u16,
    percent_y: u16,
) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Layout};

    let [_, center, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(center);

    center
}

fn state_line(state: SkillState, theme: ThemePalette) -> Line<'static> {
    Line::from(Span::styled(state.label(), state_style(state, theme)))
}

fn state_style(state: SkillState, theme: ThemePalette) -> Style {
    match state {
        SkillState::Ready | SkillState::Installed => theme.success(),
        SkillState::Active | SkillState::Installable => theme.info(),
        SkillState::UpdateAvailable | SkillState::NetworkDegraded => theme.warning(),
        SkillState::LocalOnly => Style::new().fg(theme.secondary),
        SkillState::RemoteOnly | SkillState::Unknown => theme.muted(),
        SkillState::AuthError | SkillState::SchemaError | SkillState::Error => theme.error(),
    }
}

fn risk_line(risk: RiskLevel, theme: ThemePalette) -> Line<'static> {
    Line::from(Span::styled(risk.label(), risk_style(risk, theme)))
}

fn risk_style(risk: RiskLevel, theme: ThemePalette) -> Style {
    match risk {
        RiskLevel::None => theme.muted(),
        RiskLevel::Low => theme.success(),
        RiskLevel::Medium => theme.warning(),
        RiskLevel::High => theme.error().add_modifier(Modifier::BOLD),
    }
}

fn focused_block(title: &'static str, focused: bool, theme: ThemePalette) -> Block<'static> {
    titled_block(title, focused, theme)
}

fn titled_block(title: &'static str, focused: bool, theme: ThemePalette) -> Block<'static> {
    Block::bordered()
        .title(title)
        .border_set(symbols::border::ROUNDED)
        .border_style(theme.border(focused))
        .style(Style::new().fg(theme.foreground))
}

fn plain_block(focused: bool, theme: ThemePalette) -> Block<'static> {
    Block::bordered()
        .border_set(symbols::border::ROUNDED)
        .border_style(theme.border(focused))
        .style(Style::new().fg(theme.foreground))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        App,
        config::{AppConfig, Language, LoadedConfig, SpaceSettings},
        skill::fixture_skills,
        theme::ThemeRegistry,
    };

    #[test]
    fn compact_80x24_snapshot() {
        insta::assert_snapshot!(render_snapshot(80, 24));
    }

    #[test]
    fn standard_120x40_snapshot() {
        insta::assert_snapshot!(render_snapshot(120, 40));
    }

    #[test]
    fn wide_160x50_snapshot() {
        insta::assert_snapshot!(render_snapshot(160, 50));
    }

    #[test]
    fn settings_120x40_snapshot() {
        insta::assert_snapshot!(render_settings_snapshot(120, 40));
    }

    #[test]
    fn zh_cn_120x40_snapshot() {
        let app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: PathBuf::from("skillroom/config.toml"),
                config: AppConfig {
                    language: Language::ZhCn,
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        insta::assert_snapshot!(render_app_snapshot(app, 120, 40));
    }

    #[test]
    fn zh_cn_settings_120x40_snapshot() {
        let mut app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: PathBuf::from("skillroom/config.toml"),
                config: AppConfig {
                    language: Language::ZhCn,
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );
        app.open_settings();

        insta::assert_snapshot!(render_app_snapshot(app, 120, 40));
    }

    #[test]
    fn all_themes_render_main_and_settings_without_panics() {
        for theme in ThemeRegistry::all() {
            let app = App::from_skills_with_config(
                fixture_skills(),
                LoadedConfig {
                    path: PathBuf::from("skillroom/config.toml"),
                    config: AppConfig {
                        theme,
                        ..AppConfig::default()
                    },
                    warnings: Vec::new(),
                },
            );
            assert!(render_app_snapshot(app, 120, 40).contains("Search skills"));

            let mut app = App::from_skills_with_config(
                fixture_skills(),
                LoadedConfig {
                    path: PathBuf::from("skillroom/config.toml"),
                    config: AppConfig {
                        theme,
                        ..AppConfig::default()
                    },
                    warnings: Vec::new(),
                },
            );
            app.open_settings();
            assert!(render_app_snapshot(app, 120, 40).contains("Settings"));
        }
    }

    #[test]
    fn table_scroll_keeps_bottom_selection_visible() {
        let template = fixture_skills().remove(0);
        let skills = (0..30)
            .map(|index| {
                let mut skill = template.clone();
                skill.name = format!("skill-{index:02}");
                skill
            })
            .collect::<Vec<_>>();
        let mut app = App::from_skills(skills);
        app.set_selected_for_test(29);

        let snapshot = render_app_snapshot(app, 80, 24);

        assert!(snapshot.contains("skill-29"));
        assert!(!snapshot.contains("skill-00"));
    }

    #[test]
    fn detail_low_value_fields_appear_after_detail_fullscreen() {
        let normal = render_app_snapshot(App::default(), 120, 40);
        assert!(normal.contains("Primary"));
        assert!(!normal.contains("Audit"));

        let mut app = App::default();
        app.set_zoomed_focus_for_test(Some(FocusArea::Details));
        let expanded = render_app_snapshot(app, 120, 40);

        assert!(expanded.contains("Audit"));
        assert!(expanded.contains("Commands:"));
    }

    #[test]
    fn roomy_detail_panel_uses_available_space_before_fullscreen() {
        let snapshot = render_app_snapshot(App::default(), 160, 50);

        assert!(snapshot.contains("Author:"));
        assert!(snapshot.contains("More"));
        assert!(snapshot.contains("Implementation"));
        assert!(snapshot.contains("Repository:"));
        assert!(snapshot.contains("Updated:"));
        assert!(snapshot.contains("Commands:"));
        assert!(!snapshot.contains("Audit"));
    }

    #[test]
    fn compact_detail_panel_keeps_secondary_metadata_hidden() {
        let snapshot = render_app_snapshot(App::default(), 80, 24);

        assert!(snapshot.contains("Author:"));
        assert!(!snapshot.contains("More"));
        assert!(!snapshot.contains("Repository:"));
    }

    #[test]
    fn active_space_is_context_not_a_filter() {
        let app = App::from_skills_with_config(
            fixture_skills(),
            LoadedConfig {
                path: PathBuf::from("skillroom/config.toml"),
                config: AppConfig {
                    active_space: Some("qianchuan-fe".to_string()),
                    spaces: vec![SpaceSettings::qianchuan_fe()],
                    ..AppConfig::default()
                },
                warnings: Vec::new(),
            },
        );

        let snapshot = render_app_snapshot(app, 120, 40);

        assert!(snapshot.lines().take(4).any(|line| line.contains("None")));
        assert!(snapshot.contains("Space qianchuan/fe"));
        assert!(
            !snapshot
                .lines()
                .take(4)
                .any(|line| line.contains("qianchuan/fe"))
        );
    }

    #[test]
    fn output_window_shows_latest_lines() {
        assert_eq!(output_window_start(3, 8), 0);
        assert_eq!(output_window_start(8, 8), 0);
        assert_eq!(output_window_start(12, 8), 4);
    }

    #[test]
    fn loading_screen_renders_before_inventory_is_loaded() {
        let snapshot = render_loading_snapshot(120, 40, 0, 3);

        assert!(snapshot.contains("Loading"));
        assert!(snapshot.contains("⠋"));
        assert!(snapshot.contains("Loading local skills"));
        assert!(snapshot.contains("build dashboard; wait for selection"));
    }

    #[test]
    fn loading_steps_appear_in_sequence() {
        let first = render_loading_snapshot(120, 40, 1, 1);
        let second = render_loading_snapshot(120, 40, 2, 2);
        let third = render_loading_snapshot(120, 40, 3, 3);

        assert!(first.contains("1 scan configured skill roots"));
        assert!(!first.contains("2 discover AgentBuddy Space list"));
        assert!(second.contains("2 discover AgentBuddy Space list"));
        assert!(!second.contains("3 build dashboard; wait for selection"));
        assert!(third.contains("3 build dashboard; wait for selection"));
    }

    fn render_snapshot(width: u16, height: u16) -> String {
        render_app_snapshot(App::default(), width, height)
    }

    fn render_app_snapshot(app: App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(&app, frame)).unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn render_loading_snapshot(
        width: u16,
        height: u16,
        phase: usize,
        visible_steps: usize,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| render_loading(frame, phase, visible_steps))
            .unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn render_settings_snapshot(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::default();
        app.open_settings();

        terminal.draw(|frame| render(&app, frame)).unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
        let area = buffer.area;
        let mut rows = Vec::with_capacity(area.height as usize);

        for y in area.top()..area.bottom() {
            let mut row = String::with_capacity(area.width as usize);
            for x in area.left()..area.right() {
                row.push_str(buffer[(x, y)].symbol());
            }
            rows.push(row.trim_end().to_string());
        }

        rows.join("\n")
    }
}
