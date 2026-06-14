use std::path::{Path, PathBuf};

use crate::skill::{SkillRecord, SkillScope, SkillState};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ActionKind {
    Install,
    UpdateSelected,
    UpdateAll,
    Remove,
    OpenPath,
    CopyPath,
}

impl ActionKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::UpdateSelected => "update",
            Self::UpdateAll => "update all",
            Self::Remove => "remove",
            Self::OpenPath => "open path",
            Self::CopyPath => "copy path",
        }
    }

    pub const fn confirmation_token(self) -> Option<&'static str> {
        match self {
            Self::Install => Some("INSTALL"),
            Self::UpdateSelected | Self::UpdateAll => Some("UPDATE"),
            Self::Remove => Some("REMOVE"),
            Self::OpenPath | Self::CopyPath => None,
        }
    }

    pub const fn is_write(self) -> bool {
        matches!(
            self,
            Self::Install | Self::UpdateSelected | Self::UpdateAll | Self::Remove
        )
    }

    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Remove)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ActionPlan {
    pub kind: ActionKind,
    pub title: String,
    pub skill_name: String,
    pub source: String,
    pub scope: SkillScope,
    pub path: PathBuf,
    pub agents: Vec<String>,
    pub impact: String,
    pub commands: Vec<ActionCommand>,
    pub skipped: Vec<String>,
    pub confirmation_token: Option<&'static str>,
    pub target_key: String,
}

impl ActionPlan {
    pub fn command_lines(&self) -> Vec<String> {
        self.commands
            .iter()
            .map(ActionCommand::display_line)
            .collect()
    }

    pub fn is_write(&self) -> bool {
        self.kind.is_write()
    }

    pub fn is_destructive(&self) -> bool {
        self.kind.is_destructive()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ActionCommand {
    pub argv: Vec<String>,
    pub stdin: Option<String>,
}

impl ActionCommand {
    pub fn new(argv: Vec<String>) -> Result<Self, ActionPlanError> {
        if argv.is_empty() || argv[0].trim().is_empty() {
            return Err(ActionPlanError::InvalidCommand(
                "command argv must include a program".to_string(),
            ));
        }

        Ok(Self { argv, stdin: None })
    }

    pub fn with_stdin(
        argv: Vec<String>,
        stdin: impl Into<String>,
    ) -> Result<Self, ActionPlanError> {
        let mut command = Self::new(argv)?;
        command.stdin = Some(stdin.into());
        Ok(command)
    }

    pub fn display_line(&self) -> String {
        let mut line = self
            .argv
            .iter()
            .map(|part| {
                if part.contains(char::is_whitespace) {
                    format!("{part:?}")
                } else {
                    part.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(stdin) = &self.stdin {
            line.push_str(&format!(" [stdin={}]", stdin));
        }
        line
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ActionPlanError {
    MissingSkill,
    Unsupported(String),
    InvalidCommand(String),
    Guarded(String),
}

impl ActionPlanError {
    pub fn user_message(&self) -> String {
        match self {
            Self::MissingSkill => "no selected skill".to_string(),
            Self::Unsupported(reason) | Self::InvalidCommand(reason) | Self::Guarded(reason) => {
                reason.clone()
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ActionPlanner {
    test_mode: bool,
}

impl ActionPlanner {
    pub const fn new(test_mode: bool) -> Self {
        Self { test_mode }
    }

    pub fn plan_selected(
        &self,
        kind: ActionKind,
        skill: Option<&SkillRecord>,
    ) -> Result<ActionPlan, ActionPlanError> {
        let skill = skill.ok_or(ActionPlanError::MissingSkill)?;
        match kind {
            ActionKind::Install => self.install(skill),
            ActionKind::UpdateSelected => self.update_selected(skill),
            ActionKind::Remove => self.remove(skill),
            ActionKind::OpenPath => self.open_path(skill),
            ActionKind::CopyPath => Ok(self.copy_path(skill)),
            ActionKind::UpdateAll => Err(ActionPlanError::Unsupported(
                "update all needs the full skill list".to_string(),
            )),
        }
    }

    pub fn update_all(&self, skills: &[SkillRecord]) -> Result<ActionPlan, ActionPlanError> {
        let mut commands = Vec::new();
        let mut skipped = Vec::new();
        let mut names = Vec::new();

        for skill in skills {
            if skill.state != SkillState::UpdateAvailable {
                skipped.push(format!(
                    "{} skipped: state is {}",
                    skill.name,
                    skill.state.label()
                ));
                continue;
            }

            match parse_command_list(&skill.command_plan.update) {
                Ok(skill_commands) if !skill_commands.is_empty() => {
                    commands.extend(skill_commands);
                    names.push(skill.name.clone());
                }
                Ok(_) => skipped.push(format!("{} skipped: no update command", skill.name)),
                Err(error) => {
                    skipped.push(format!("{} skipped: {}", skill.name, error.user_message()))
                }
            }
        }

        if commands.is_empty() {
            return Err(ActionPlanError::Unsupported(
                "no trusted update-available skills with executable update commands".to_string(),
            ));
        }

        Ok(ActionPlan {
            kind: ActionKind::UpdateAll,
            title: "Update all trusted skills".to_string(),
            skill_name: names.join(", "),
            source: "mixed".to_string(),
            scope: SkillScope::Global,
            path: PathBuf::from("multiple"),
            agents: Vec::new(),
            impact: format!("updates {} trusted skill(s)", names.len()),
            commands,
            skipped,
            confirmation_token: ActionKind::UpdateAll.confirmation_token(),
            target_key: "update-all".to_string(),
        })
    }

    fn install(&self, skill: &SkillRecord) -> Result<ActionPlan, ActionPlanError> {
        let commands = parse_command_list(&skill.command_plan.install)?;
        if commands.is_empty() {
            return Err(ActionPlanError::Unsupported(format!(
                "{} has no install command",
                skill.name
            )));
        }

        Ok(self.skill_plan(
            ActionKind::Install,
            skill,
            commands,
            format!("installs {} into {}", skill.name, skill.scope.label()),
        ))
    }

    fn update_selected(&self, skill: &SkillRecord) -> Result<ActionPlan, ActionPlanError> {
        if skill.state == SkillState::Unknown {
            return Err(ActionPlanError::Unsupported(format!(
                "{} update state is unknown; source/version is not judgeable",
                skill.name
            )));
        }

        let commands = parse_command_list(&skill.command_plan.update)?;
        if commands.is_empty() {
            return Err(ActionPlanError::Unsupported(format!(
                "{} has no update command",
                skill.name
            )));
        }

        Ok(self.skill_plan(
            ActionKind::UpdateSelected,
            skill,
            commands,
            format!(
                "updates {} from {} to {}",
                skill.name,
                skill.version_label(),
                skill.update_label()
            ),
        ))
    }

    fn remove(&self, skill: &SkillRecord) -> Result<ActionPlan, ActionPlanError> {
        if self.test_mode && is_real_home_path(&skill.path) {
            return Err(ActionPlanError::Guarded(format!(
                "{} remove blocked under test mode for real home path {}",
                skill.name,
                skill.path.display()
            )));
        }

        let commands = parse_command_list(&skill.command_plan.remove)?;
        if commands.is_empty() {
            return Err(ActionPlanError::Unsupported(format!(
                "{} has no remove command",
                skill.name
            )));
        }

        Ok(self.skill_plan(
            ActionKind::Remove,
            skill,
            commands,
            format!(
                "removes the selected {} skill path only; per-agent removal is not part of M5",
                skill.scope.label()
            ),
        ))
    }

    fn open_path(&self, skill: &SkillRecord) -> Result<ActionPlan, ActionPlanError> {
        Ok(self.skill_plan(
            ActionKind::OpenPath,
            skill,
            vec![ActionCommand::new(vec![
                "open".to_string(),
                skill.path.display().to_string(),
            ])?],
            format!("opens {}", skill.path.display()),
        ))
    }

    fn copy_path(&self, skill: &SkillRecord) -> ActionPlan {
        self.skill_plan(
            ActionKind::CopyPath,
            skill,
            vec![
                ActionCommand::with_stdin(
                    vec!["pbcopy".to_string()],
                    skill.path.display().to_string(),
                )
                .expect("pbcopy argv is static and valid"),
            ],
            format!("copies {}", skill.path.display()),
        )
    }

    fn skill_plan(
        &self,
        kind: ActionKind,
        skill: &SkillRecord,
        commands: Vec<ActionCommand>,
        impact: String,
    ) -> ActionPlan {
        ActionPlan {
            kind,
            title: format!("{} {}", title_case(kind.label()), skill.name),
            skill_name: skill.name.clone(),
            source: skill.source.label().to_string(),
            scope: skill.scope,
            path: skill.path.clone(),
            agents: skill
                .agents
                .iter()
                .filter(|agent| agent.enabled)
                .map(|agent| agent.name.clone())
                .collect(),
            impact,
            commands,
            skipped: Vec::new(),
            confirmation_token: kind.confirmation_token(),
            target_key: format!("skill:{}", skill.name),
        }
    }
}

pub fn parse_command_list(commands: &[String]) -> Result<Vec<ActionCommand>, ActionPlanError> {
    commands
        .iter()
        .map(|command| parse_command_line(command).and_then(ActionCommand::new))
        .collect()
}

pub fn parse_command_line(command: &str) -> Result<Vec<String>, ActionPlanError> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            character if character.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }

    if escaped {
        current.push('\\');
    }

    if let Some(character) = quote {
        return Err(ActionPlanError::InvalidCommand(format!(
            "command has unclosed quote {character:?}: {command}"
        )));
    }

    if !current.is_empty() {
        parts.push(current);
    }

    if parts.is_empty() {
        return Err(ActionPlanError::InvalidCommand(
            "command is empty".to_string(),
        ));
    }

    Ok(parts)
}

fn is_real_home_path(path: &Path) -> bool {
    let text = path.display().to_string();
    if text == "~" || text.starts_with("~/") || text == "$HOME" || text.starts_with("$HOME/") {
        return true;
    }

    std::env::var("HOME")
        .ok()
        .filter(|home| !home.is_empty())
        .is_some_and(|home| path.starts_with(home))
}

fn title_case(label: &str) -> String {
    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{CommandPlan, Source, fixture_skills};

    #[test]
    fn parses_command_strings_to_argv_without_shell() {
        assert_eq!(
            parse_command_line("agentbuddy skill add abc --all").unwrap(),
            vec!["agentbuddy", "skill", "add", "abc", "--all"]
        );
        assert_eq!(
            parse_command_line("skillroom install \"code review\"").unwrap(),
            vec!["skillroom", "install", "code review"]
        );
        assert!(parse_command_line("skillroom install \"broken").is_err());
    }

    #[test]
    fn selected_install_plan_shows_source_scope_agents_and_argv() {
        let skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "taproom")
            .unwrap();
        let plan = ActionPlanner::new(false)
            .plan_selected(ActionKind::Install, Some(&skill))
            .unwrap();

        assert_eq!(plan.confirmation_token, Some("INSTALL"));
        assert_eq!(plan.source, "local/git");
        assert_eq!(plan.scope, SkillScope::Local);
        assert_eq!(plan.agents.len(), 3);
        assert_eq!(
            plan.commands[0].argv,
            vec!["skillroom", "install", "taproom"]
        );
    }

    #[test]
    fn update_selected_explains_unknown_state() {
        let mut skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "code-review")
            .unwrap();
        skill.state = SkillState::Unknown;

        let error = ActionPlanner::new(false)
            .plan_selected(ActionKind::UpdateSelected, Some(&skill))
            .unwrap_err();

        assert!(error.user_message().contains("unknown"));
        assert!(error.user_message().contains("not judgeable"));
    }

    #[test]
    fn update_all_only_includes_trusted_update_available_records() {
        let skills = fixture_skills();
        let plan = ActionPlanner::new(false).update_all(&skills).unwrap();

        assert_eq!(plan.confirmation_token, Some("UPDATE"));
        assert_eq!(plan.commands.len(), 1);
        assert_eq!(
            plan.commands[0].argv,
            vec!["skillroom", "update", "code-review"]
        );
        assert!(plan.skipped.iter().any(|line| line.contains("taproom")));
    }

    #[test]
    fn remove_is_blocked_for_real_home_path_under_test_mode() {
        let skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "code-review")
            .unwrap();

        let error = ActionPlanner::new(true)
            .plan_selected(ActionKind::Remove, Some(&skill))
            .unwrap_err();

        assert!(matches!(error, ActionPlanError::Guarded(_)));
    }

    #[test]
    fn remove_plan_allows_temp_fixture_path_under_test_mode() {
        let mut skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "code-review")
            .unwrap();
        skill.source = Source::LocalGit;
        skill.path = PathBuf::from("/private/tmp/skillroom-test/code-review");
        skill.command_plan = CommandPlan {
            remove: vec![
                "skillroom remove --path /private/tmp/skillroom-test/code-review".to_string(),
            ],
            ..CommandPlan::default()
        };

        let plan = ActionPlanner::new(true)
            .plan_selected(ActionKind::Remove, Some(&skill))
            .unwrap();

        assert_eq!(plan.confirmation_token, Some("REMOVE"));
        assert!(plan.impact.contains("per-agent removal is not part of M5"));
        assert_eq!(
            plan.commands[0].argv,
            vec![
                "skillroom",
                "remove",
                "--path",
                "/private/tmp/skillroom-test/code-review"
            ]
        );
    }

    #[test]
    fn open_and_copy_path_have_non_destructive_plans() {
        let skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "taproom")
            .unwrap();
        let planner = ActionPlanner::new(true);

        let open = planner
            .plan_selected(ActionKind::OpenPath, Some(&skill))
            .unwrap();
        let copy = planner
            .plan_selected(ActionKind::CopyPath, Some(&skill))
            .unwrap();

        assert_eq!(open.confirmation_token, None);
        assert_eq!(copy.confirmation_token, None);
        assert!(!open.is_write());
        assert!(!copy.is_destructive());
        assert_eq!(copy.commands[0].argv, vec!["pbcopy"]);
        let path = skill.path.display().to_string();
        assert_eq!(copy.commands[0].stdin.as_deref(), Some(path.as_str()));
    }
}
