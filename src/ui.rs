use ratatui::{
    Frame,
    layout::Alignment,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table, Wrap},
};

use crate::{
    app::App,
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
}

fn render_search(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let line = Line::from(vec![
        " Skillroom ".bold().cyan(),
        format!("{} skills ", app.skills().len()).dim(),
        "[/] Search skills...".dim(),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .block(Block::bordered().title("Command"))
            .alignment(Alignment::Left),
        area,
    );
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

    let rows = app.skills().iter().enumerate().map(|(index, skill)| {
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
            Cell::from(skill.source),
            Cell::from(skill.scope.label()),
            Cell::from(state_line(skill.state)),
            Cell::from(risk_line(skill.risk)),
            Cell::from(skill.update),
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
    .block(Block::bordered().title("Skills"));

    frame.render_widget(table, area);
}

fn render_details(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let lines = match app.selected_skill() {
        Some(skill) => vec![
            Line::from(vec!["Name: ".bold(), skill.name.cyan()]),
            Line::from(vec!["Description: ".bold(), skill.description.into()]),
            Line::from(vec!["Path: ".bold(), skill.path.into()]),
            Line::from(vec!["Version: ".bold(), skill.version.into()]),
            Line::from(vec!["Source: ".bold(), skill.source.into()]),
            Line::from(vec!["Scripts: ".bold(), skill.scripts.join(", ").into()]),
            Line::from(vec!["Tags: ".bold(), skill.tags.join(", ").dim()]),
        ],
        None => vec![Line::from("No skill selected".dim())],
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title("Details"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_stats(app: &App, frame: &mut Frame<'_>, area: ratatui::layout::Rect) {
    let total = app.skills().len();
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
        Line::from(vec!["Total ".dim(), total.to_string().bold()]),
        Line::from(vec!["Local ".dim(), local.to_string().cyan()]),
        Line::from(vec!["Updates ".dim(), updates.to_string().yellow()]),
        Line::from(vec!["High risk ".dim(), high_risk.to_string().red()]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title("Stats")),
        area,
    );
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
        " Tab ".bold().cyan(),
        "focus ".dim(),
        " Enter ".bold().cyan(),
        "select".dim(),
    ]);

    frame.render_widget(Paragraph::new(help).block(Block::bordered()), area);
}

fn state_line(state: SkillState) -> Line<'static> {
    match state {
        SkillState::Ready => Line::from("Ready".green()),
        SkillState::Active => Line::from("Active".cyan()),
        SkillState::UpdateAvailable => Line::from("Update".yellow()),
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
