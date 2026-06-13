use ratatui::{
    Frame,
    layout::Alignment,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::{App, FocusArea, InputMode},
    layout::{AppLayout, too_small_message},
    skill::{RiskLevel, SkillState},
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

    render_search(app, frame, layout.search);
    render_table(app, frame, layout.table);
    render_details(app, frame, layout.details);
    render_stats(app, frame, layout.stats);
    render_output(app, frame, layout.output);
    render_help(frame, layout.help);

    if app.show_help() {
        render_help_overlay(frame, area);
    }

    if app.settings_open() {
        render_settings(app, frame, area);
    }
}

fn render_search(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let line = Line::from(vec![
        " Skillroom ".bold().cyan(),
        format!(
            "{} / {} skills ",
            app.visible_skills().len(),
            app.skills().len()
        )
        .dim(),
        search_prompt(app),
        " focus=".dim(),
        app.focus().label().cyan(),
        " sort=".dim(),
        sort_label(app),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .block(focused_block("Command", app.focus() == FocusArea::Search))
            .alignment(Alignment::Left),
        area,
    );
}

fn search_prompt(app: &App) -> Span<'static> {
    match app.input_mode() {
        InputMode::Normal => "[/] Search skills...".dim(),
        InputMode::Search if app.search_query().is_empty() => "/ ".cyan(),
        InputMode::Search => format!("/ {}", app.search_query()).cyan(),
    }
}

fn sort_label(app: &App) -> Span<'static> {
    let direction = if app.sort_ascending() { "asc" } else { "desc" };
    format!("{} {direction}", app.sort_column().label()).cyan()
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

fn render_table(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let header = Row::new([
        Cell::from("Name".bold()),
        Cell::from("Source".bold()),
        Cell::from("Scope".bold()),
        Cell::from("State".bold()),
        Cell::from("Risk".bold()),
        Cell::from("Update".bold()),
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
                Style::new().reversed()
            } else {
                Style::new()
            };

            Row::new([
                Cell::from(format!("{marker}{}", skill.name)),
                Cell::from(skill.source.label().to_string()),
                Cell::from(skill.scope.label()),
                Cell::from(state_line(skill.state)),
                Cell::from(risk_line(skill.risk)),
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
    .block(focused_block("Skills", app.focus() == FocusArea::Table));

    frame.render_widget(table, area);
}

fn render_details(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let lines = match app.selected_skill() {
        Some(skill) => vec![
            Line::from(vec!["Name: ".bold(), skill.name.as_str().cyan()]),
            Line::from(vec!["Scope: ".bold(), skill.scope.label().into()]),
            Line::from(vec!["State: ".bold(), skill.state.label().into()]),
            Line::from(vec!["Source: ".bold(), skill.source.label().into()]),
            Line::from(vec!["Version: ".bold(), skill.version_label().into()]),
            Line::from(vec![
                "Path: ".bold(),
                skill.path.display().to_string().into(),
            ]),
            Line::from(vec!["Agents: ".bold(), agents_summary(skill).into()]),
            Line::from(vec!["Risk: ".bold(), skill.risk.label().into()]),
            Line::from(vec![
                "Files: ".bold(),
                format!(
                    "{} files, {} dirs, {} refs, {} assets, {} lines",
                    skill.stats.files,
                    skill.stats.directories,
                    skill.stats.references,
                    skill.stats.assets,
                    skill.stats.line_count
                )
                .into(),
            ]),
            Line::from(vec!["Scripts: ".bold(), csv_or_none(&skill.scripts).into()]),
            Line::from(vec!["Actions: ".bold(), action_summary(skill).dim()]),
            Line::from(vec![
                "Error: ".bold(),
                skill.error.as_deref().unwrap_or("none").into(),
            ]),
            Line::from(vec![
                "Description: ".bold(),
                skill.description.as_str().into(),
            ]),
            Line::from(vec!["Tags: ".bold(), csv_or_none(&skill.tags).dim()]),
        ],
        None => vec![Line::from("No skill selected".dim())],
    };

    frame.render_widget(
        Paragraph::new(lines).block(focused_block("Details", app.focus() == FocusArea::Details)),
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

fn render_stats(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
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
            "Filters ".dim(),
            if has_active_filters(app) {
                "active".yellow()
            } else if app.focus() == FocusArea::Filters {
                "focused".cyan()
            } else {
                "ready".dim()
            },
        ]),
        Line::from(vec![
            "Settings ".dim(),
            if app.focus() == FocusArea::Settings {
                "focused".cyan()
            } else {
                "placeholder".dim()
            },
        ]),
        Line::from(vec!["Visible ".dim(), visible.to_string().bold()]),
        Line::from(vec!["Total ".dim(), total.to_string().bold()]),
        Line::from(vec!["Local ".dim(), local.to_string().cyan()]),
        Line::from(vec!["Updates ".dim(), updates.to_string().yellow()]),
        Line::from(vec!["High risk ".dim(), high_risk.to_string().red()]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(focused_block(
            "Stats",
            matches!(app.focus(), FocusArea::Filters | FocusArea::Settings),
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

fn render_output(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let lines: Vec<Line<'static>> = app
        .output()
        .iter()
        .map(|line| Line::from(vec![Span::from("> ").dim(), Span::from(line.clone())]))
        .collect();

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title("Output"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let help = Line::from(vec![
        " q ".bold().cyan(),
        "quit ".dim(),
        " / ".bold().cyan(),
        "search ".dim(),
        " ? ".bold().cyan(),
        "help ".dim(),
        " , ".bold().cyan(),
        "settings ".dim(),
        " Tab ".bold().cyan(),
        "focus ".dim(),
        " Enter ".bold().cyan(),
        "select".dim(),
    ]);

    frame.render_widget(Paragraph::new(help).block(Block::bordered()), area);
}

fn render_settings(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let popup = centered_rect(area, 72, 70);
    let mut lines = vec![
        Line::from(vec![
            "Settings".bold().cyan(),
            "  ".into(),
            "Esc cancels".dim(),
        ]),
        Line::from(vec![
            "Config: ".dim(),
            app.config_path().display().to_string().into(),
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
            marker.into(),
            format!("{:<12}", row.label).bold(),
            row.value.into(),
            "  ".into(),
            row.hint.dim(),
        ]);
        if index == app.settings_selected() {
            lines.push(line.style(Style::new().reversed()));
        } else {
            lines.push(line);
        }
    }

    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines).block(focused_block("Settings", true)),
        popup,
    );
}

fn render_help_overlay(frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let popup = centered_rect(area, 64, 52);
    let lines = vec![
        Line::from("Navigation".bold().cyan()),
        Line::from("j/k or arrows: move selection"),
        Line::from("PageUp/PageDown: page selection"),
        Line::from("g/G: jump to top/bottom"),
        Line::from("Tab / Shift+Tab: cycle focus"),
        Line::from("s/S: cycle sort column / reverse sort"),
        Line::from("?: close help"),
        Line::from("q: quit"),
    ];

    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block("Help", true))
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

fn state_line(state: SkillState) -> Line<'static> {
    match state {
        SkillState::Ready => Line::from("Ready".green()),
        SkillState::Active => Line::from("Active".cyan()),
        SkillState::UpdateAvailable => Line::from("Update".yellow()),
        SkillState::Installed => Line::from("Installed".green()),
        SkillState::LocalOnly => Line::from("Local".magenta()),
        SkillState::Unknown => Line::from("Unknown".dim()),
        SkillState::Error => Line::from("Error".red()),
    }
}

fn risk_line(risk: RiskLevel) -> Line<'static> {
    match risk {
        RiskLevel::None => Line::from("None".dim()),
        RiskLevel::Low => Line::from("Low".green()),
        RiskLevel::Medium => Line::from("Medium".yellow()),
        RiskLevel::High => Line::from("High".red().bold()),
    }
}

fn focused_block(title: &'static str, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::new().cyan()
    } else {
        Style::new().dim()
    };

    Block::bordered().title(title).border_style(border_style)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::App;

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

    fn render_snapshot(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::default();

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
