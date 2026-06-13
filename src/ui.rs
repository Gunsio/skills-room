use ratatui::{
    Frame,
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::{App, FocusArea, InputMode},
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
    render_table(app, frame, layout.table, theme);
    render_details(app, frame, layout.details, theme);
    render_stats(app, frame, layout.stats, theme);
    render_output(app, frame, layout.output, theme);
    render_help(app, frame, layout.help, theme);

    if app.show_help() {
        render_help_overlay(app, frame, area, theme);
    }

    if app.settings_open() {
        render_settings(app, frame, area, theme);
    }
}

fn render_search(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let line = Line::from(vec![
        Span::styled(" Skillroom ", theme.title()),
        Span::styled(
            format!(
                "{} / {} skills ",
                app.visible_skills().len(),
                app.skills().len()
            ),
            theme.muted(),
        ),
        search_prompt(app, theme),
        Span::styled(app.text(I18nKey::Focus), theme.muted()),
        Span::styled(app.focus().label(), theme.info()),
        Span::styled(app.text(I18nKey::Sort), theme.muted()),
        sort_label(app, theme),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .style(theme.value())
            .block(focused_block(
                app.text(I18nKey::PanelCommand),
                app.focus() == FocusArea::Search,
                theme,
            ))
            .alignment(Alignment::Left),
        area,
    );
}

fn search_prompt(app: &App, theme: ThemePalette) -> Span<'static> {
    match app.input_mode() {
        InputMode::Normal => Span::styled(app.text(I18nKey::SearchPlaceholder), theme.muted()),
        InputMode::Search if app.search_query().is_empty() => Span::styled("/ ", theme.info()),
        InputMode::Search => Span::styled(format!("/ {}", app.search_query()), theme.info()),
    }
}

fn sort_label(app: &App, theme: ThemePalette) -> Span<'static> {
    let direction = if app.sort_ascending() { "asc" } else { "desc" };
    let column = match app.sort_column() {
        crate::app::SortColumn::Name => app.text(I18nKey::ColumnName),
        crate::app::SortColumn::Source => app.text(I18nKey::ColumnSource),
        crate::app::SortColumn::Scope => app.text(I18nKey::ColumnScope),
        crate::app::SortColumn::State => app.text(I18nKey::ColumnState),
        crate::app::SortColumn::Risk => app.text(I18nKey::ColumnRisk),
        crate::app::SortColumn::Update => app.text(I18nKey::ColumnUpdate),
    };
    Span::styled(format!("{column} {direction}"), theme.info())
}

fn has_active_filters(app: &App) -> bool {
    let filters = app.filters();
    !app.search_query().is_empty()
        || filters.source.is_some()
        || filters.scope.is_some()
        || filters.state.is_some()
        || filters.risk.is_some()
        || filters.update.is_some()
}

fn render_table(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let header = Row::new([
        Cell::from(Span::styled(app.text(I18nKey::ColumnName), theme.label())),
        Cell::from(Span::styled(app.text(I18nKey::ColumnSource), theme.label())),
        Cell::from(Span::styled(app.text(I18nKey::ColumnScope), theme.label())),
        Cell::from(Span::styled(app.text(I18nKey::ColumnState), theme.label())),
        Cell::from(Span::styled(app.text(I18nKey::ColumnRisk), theme.label())),
        Cell::from(Span::styled(app.text(I18nKey::ColumnUpdate), theme.label())),
    ]);

    let rows = app
        .visible_skills()
        .into_iter()
        .enumerate()
        .map(|(index, (_, skill))| {
            let marker = if index == app.selected_index() {
                "> "
            } else {
                "  "
            };
            let style = if index == app.selected_index() {
                theme.selected()
            } else {
                theme.value()
            };

            Row::new([
                Cell::from(format!("{marker}{}", skill.name)),
                Cell::from(skill.source.label().to_string()),
                Cell::from(skill.scope.label()),
                Cell::from(state_line(skill.state, theme)),
                Cell::from(risk_line(skill.risk, theme)),
                Cell::from(skill.update_label().to_string()),
            ])
            .style(style)
        });

    let table = Table::new(
        rows,
        [
            ratatui::layout::Constraint::Percentage(24),
            ratatui::layout::Constraint::Percentage(24),
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Min(8),
        ],
    )
    .header(header)
    .block(focused_block(
        app.text(I18nKey::PanelSkills),
        app.focus() == FocusArea::Table,
        theme,
    ));

    frame.render_widget(table, area);
}

fn render_details(
    app: &App,
    frame: &mut Frame<'_>,
    area: ratatui::layout::Rect,
    theme: ThemePalette,
) {
    let lines = match app.selected_skill() {
        Some(skill) => vec![
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailName), theme.label()),
                Span::styled(skill.name.clone(), theme.info()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailScope), theme.label()),
                Span::styled(skill.scope.label(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailState), theme.label()),
                Span::styled(skill.state.label(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailSource), theme.label()),
                Span::styled(skill.source.label(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailVersion), theme.label()),
                Span::styled(skill.version_label(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailPath), theme.label()),
                Span::styled(skill.path.display().to_string(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailAgents), theme.label()),
                Span::styled(agents_summary(skill), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailRisk), theme.label()),
                Span::styled(skill.risk.label(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailFiles), theme.label()),
                Span::styled(
                    format!(
                        "{} files, {} dirs, {} refs, {} assets, {} lines",
                        skill.stats.files,
                        skill.stats.directories,
                        skill.stats.references,
                        skill.stats.assets,
                        skill.stats.line_count
                    ),
                    theme.value(),
                ),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailScripts), theme.label()),
                Span::styled(csv_or_none(&skill.scripts), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailActions), theme.label()),
                Span::styled(action_summary(skill), theme.muted()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailError), theme.label()),
                Span::styled(skill.error.as_deref().unwrap_or("none"), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailDescription), theme.label()),
                Span::styled(skill.description.clone(), theme.value()),
            ]),
            Line::from(vec![
                Span::styled(app.text(I18nKey::DetailTags), theme.label()),
                Span::styled(csv_or_none(&skill.tags), theme.muted()),
            ]),
        ],
        None => vec![Line::from(Span::styled(
            app.text(I18nKey::NoSkillSelected),
            theme.muted(),
        ))],
    };

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(
                app.text(I18nKey::PanelDetails),
                app.focus() == FocusArea::Details,
                theme,
            )),
        area,
    );
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
    let total = app.skills().len();
    let visible = app.visible_skills().len();
    let local = app
        .skills()
        .iter()
        .filter(|skill| skill.scope.label() == "Local")
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

    let lines = vec![
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatFilters), theme.muted()),
            if has_active_filters(app) {
                Span::styled(app.text(I18nKey::StatusActive), theme.warning())
            } else if app.focus() == FocusArea::Filters {
                Span::styled(app.text(I18nKey::StatusFocused), theme.info())
            } else {
                Span::styled(app.text(I18nKey::StatusReady), theme.muted())
            },
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatSettings), theme.muted()),
            if app.focus() == FocusArea::Settings {
                Span::styled(app.text(I18nKey::StatusFocused), theme.info())
            } else {
                Span::styled(app.text(I18nKey::StatusPlaceholder), theme.muted())
            },
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatVisible), theme.muted()),
            Span::styled(visible.to_string(), theme.label()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatTotal), theme.muted()),
            Span::styled(total.to_string(), theme.label()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatLocal), theme.muted()),
            Span::styled(local.to_string(), theme.info()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatUpdates), theme.muted()),
            Span::styled(updates.to_string(), theme.warning()),
        ]),
        Line::from(vec![
            Span::styled(app.text(I18nKey::StatHighRisk), theme.muted()),
            Span::styled(high_risk.to_string(), theme.error()),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .style(theme.value())
            .block(focused_block(
                app.text(I18nKey::PanelStats),
                matches!(app.focus(), FocusArea::Filters | FocusArea::Settings),
                theme,
            )),
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
    let lines: Vec<Line<'static>> = app
        .output()
        .iter()
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

fn render_help(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect, theme: ThemePalette) {
    let key_style = theme.title();
    let text_style = theme.muted();
    let help = Line::from(vec![
        Span::styled(" q ", key_style),
        Span::styled(app.text(I18nKey::KeyQuit), text_style),
        Span::styled(" / ", key_style),
        Span::styled(app.text(I18nKey::KeySearch), text_style),
        Span::styled(" ? ", key_style),
        Span::styled(app.text(I18nKey::KeyHelp), text_style),
        Span::styled(" , ", key_style),
        Span::styled(app.text(I18nKey::KeySettings), text_style),
        Span::styled(" Tab ", key_style),
        Span::styled(app.text(I18nKey::KeyFocus), text_style),
        Span::styled(" Enter ", key_style),
        Span::styled(app.text(I18nKey::KeySelect), text_style),
    ]);

    frame.render_widget(
        Paragraph::new(help)
            .style(theme.value())
            .block(focused_block("", false, theme)),
        area,
    );
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
    match state {
        SkillState::Ready => Line::from(Span::styled("Ready", theme.success())),
        SkillState::Active => Line::from(Span::styled("Active", theme.info())),
        SkillState::UpdateAvailable => Line::from(Span::styled("Update", theme.warning())),
        SkillState::Installed => Line::from(Span::styled("Installed", theme.success())),
        SkillState::LocalOnly => {
            Line::from(Span::styled("Local", Style::new().fg(theme.secondary)))
        }
        SkillState::Unknown => Line::from(Span::styled("Unknown", theme.muted())),
        SkillState::Error => Line::from(Span::styled("Error", theme.error())),
    }
}

fn risk_line(risk: RiskLevel, theme: ThemePalette) -> Line<'static> {
    match risk {
        RiskLevel::None => Line::from(Span::styled("None", theme.muted())),
        RiskLevel::Low => Line::from(Span::styled("Low", theme.success())),
        RiskLevel::Medium => Line::from(Span::styled("Medium", theme.warning())),
        RiskLevel::High => Line::from(Span::styled(
            "High",
            theme.error().add_modifier(ratatui::style::Modifier::BOLD),
        )),
    }
}

fn focused_block(title: &'static str, focused: bool, theme: ThemePalette) -> Block<'static> {
    Block::bordered()
        .title(title)
        .border_style(theme.border(focused))
        .style(Style::new().bg(theme.surface).fg(theme.foreground))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        App,
        config::{AppConfig, Language, LoadedConfig},
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
            assert!(render_app_snapshot(app, 120, 40).contains("Skillroom"));

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

    fn render_snapshot(width: u16, height: u16) -> String {
        render_app_snapshot(App::default(), width, height)
    }

    fn render_app_snapshot(app: App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| render(&app, frame)).unwrap();
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
