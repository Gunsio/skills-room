use std::path::PathBuf;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Source {
    LocalGit,
    LocalArchive,
    Curated,
    Github,
    InternalRegistry,
    Npx,
    Unknown(String),
}

impl Source {
    pub fn label(&self) -> &str {
        match self {
            Self::LocalGit => "local/git",
            Self::LocalArchive => "local/archive",
            Self::Curated => "curated",
            Self::Github => "github",
            Self::InternalRegistry => "skills.bytedance.net",
            Self::Npx => "npx skills",
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkillScope {
    Local,
    Global,
    Project,
}

impl SkillScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Global => "Global",
            Self::Project => "Project",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkillState {
    Ready,
    Active,
    UpdateAvailable,
    Installed,
    LocalOnly,
    Unknown,
    Error,
}

impl SkillState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Active => "Active",
            Self::UpdateAvailable => "Update",
            Self::Installed => "Installed",
            Self::LocalOnly => "LocalOnly",
            Self::Unknown => "Unknown",
            Self::Error => "Error",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Agent {
    pub name: String,
    pub enabled: bool,
}

impl Agent {
    pub fn enabled(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct CommandPlan {
    pub install: Vec<String>,
    pub update: Vec<String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SkillStats {
    pub files: usize,
    pub directories: usize,
    pub references: usize,
    pub assets: usize,
    pub line_count: usize,
    pub modified_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SkillRecord {
    pub name: String,
    pub source: Source,
    pub scope: SkillScope,
    pub agents: Vec<Agent>,
    pub state: SkillState,
    pub risk: RiskLevel,
    pub version: Option<String>,
    pub update: Option<String>,
    pub path: PathBuf,
    pub description: String,
    pub scripts: Vec<String>,
    pub tags: Vec<String>,
    pub command_plan: CommandPlan,
    pub stats: SkillStats,
    pub error: Option<String>,
}

impl SkillRecord {
    pub fn agents_count(&self) -> usize {
        self.agents.iter().filter(|agent| agent.enabled).count()
    }

    pub fn version_label(&self) -> &str {
        self.version.as_deref().unwrap_or("unknown")
    }

    pub fn update_label(&self) -> &str {
        self.update.as_deref().unwrap_or("current")
    }
}

pub fn fixture_skills() -> Vec<SkillRecord> {
    vec![
        SkillRecord {
            name: "taproom".to_string(),
            source: Source::LocalGit,
            scope: SkillScope::Local,
            agents: agents(3),
            state: SkillState::Ready,
            risk: RiskLevel::None,
            version: Some("0.1.0".to_string()),
            update: Some("current".to_string()),
            path: PathBuf::from("~/.skillroom/taproom"),
            description: "TUI environment manager used as the benchmark interaction model."
                .to_string(),
            scripts: strings(["init.sh", "build.sh", "run.sh"]),
            tags: strings(["tui", "benchmark", "local"]),
            command_plan: CommandPlan {
                install: strings(["skillroom install taproom"]),
                update: strings(["skillroom update taproom"]),
                remove: strings(["skillroom remove taproom"]),
            },
            stats: SkillStats {
                files: 24,
                directories: 2,
                references: 3,
                assets: 1,
                line_count: 420,
                modified_unix_seconds: None,
            },
            error: None,
        },
        SkillRecord {
            name: "data-analysis".to_string(),
            source: Source::InternalRegistry,
            scope: SkillScope::Global,
            agents: agents(5),
            state: SkillState::Active,
            risk: RiskLevel::Low,
            version: Some("2.4.1".to_string()),
            update: Some("current".to_string()),
            path: PathBuf::from("~/.agents/skills/data-analysis"),
            description: "Analyze CSV, sheets, metrics, and experiment outputs.".to_string(),
            scripts: strings(["check.sh"]),
            tags: strings(["analysis", "metrics", "global"]),
            command_plan: CommandPlan::default(),
            stats: SkillStats {
                files: 18,
                directories: 3,
                references: 4,
                assets: 0,
                line_count: 780,
                modified_unix_seconds: None,
            },
            error: None,
        },
        SkillRecord {
            name: "code-review".to_string(),
            source: Source::Curated,
            scope: SkillScope::Local,
            agents: agents(2),
            state: SkillState::UpdateAvailable,
            risk: RiskLevel::Medium,
            version: Some("1.8.0".to_string()),
            update: Some("1.9.2".to_string()),
            path: PathBuf::from("~/.codex/skills/code-review"),
            description: "Review pull requests with repository-local conventions and test gates."
                .to_string(),
            scripts: strings(["lint.sh", "review.sh"]),
            tags: strings(["code", "review", "quality"]),
            command_plan: CommandPlan {
                install: Vec::new(),
                update: strings(["skillroom update code-review"]),
                remove: strings(["skillroom remove code-review"]),
            },
            stats: SkillStats {
                files: 12,
                directories: 2,
                references: 1,
                assets: 0,
                line_count: 360,
                modified_unix_seconds: None,
            },
            error: None,
        },
        SkillRecord {
            name: "web-scraper".to_string(),
            source: Source::Github,
            scope: SkillScope::Global,
            agents: agents(1),
            state: SkillState::Ready,
            risk: RiskLevel::Low,
            version: Some("0.9.3".to_string()),
            update: Some("current".to_string()),
            path: PathBuf::from("~/.codex/skills/web-scraper"),
            description: "Collect and normalize web data with browser assisted checks.".to_string(),
            scripts: strings(["crawl.sh"]),
            tags: strings(["browser", "source", "global"]),
            command_plan: CommandPlan::default(),
            stats: SkillStats {
                files: 9,
                directories: 1,
                references: 2,
                assets: 1,
                line_count: 240,
                modified_unix_seconds: None,
            },
            error: None,
        },
        SkillRecord {
            name: "legacy-deploy".to_string(),
            source: Source::LocalArchive,
            scope: SkillScope::Local,
            agents: Vec::new(),
            state: SkillState::Error,
            risk: RiskLevel::High,
            version: Some("0.3.7".to_string()),
            update: Some("blocked".to_string()),
            path: PathBuf::from("~/.skillroom/legacy-deploy"),
            description: "Archived deployment helper kept for migration visibility.".to_string(),
            scripts: strings(["deploy.sh"]),
            tags: strings(["legacy", "deploy", "blocked"]),
            command_plan: CommandPlan {
                install: Vec::new(),
                update: Vec::new(),
                remove: strings(["skillroom remove legacy-deploy"]),
            },
            stats: SkillStats {
                files: 7,
                directories: 1,
                references: 0,
                assets: 0,
                line_count: 110,
                modified_unix_seconds: None,
            },
            error: Some("SKILL.md frontmatter is missing required fields".to_string()),
        },
    ]
}

fn agents(count: usize) -> Vec<Agent> {
    (1..=count)
        .map(|index| Agent::enabled(format!("agent-{index}")))
        .collect()
}

fn strings<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_data_has_multiple_sources_and_states() {
        let skills = fixture_skills();

        assert!(skills.len() >= 5);
        assert!(
            skills
                .iter()
                .any(|skill| skill.source == Source::InternalRegistry)
        );
        assert!(
            skills
                .iter()
                .any(|skill| skill.state == SkillState::UpdateAvailable)
        );
        assert!(skills.iter().any(|skill| skill.risk == RiskLevel::High));
    }

    #[test]
    fn fixture_records_are_renderable_without_allocation_side_effects() {
        for skill in fixture_skills() {
            assert!(!skill.name.is_empty());
            assert!(!skill.source.label().is_empty());
            assert!(!skill.scope.label().is_empty());
            assert!(!skill.state.label().is_empty());
            assert!(!skill.risk.label().is_empty());
            assert!(!skill.path.as_os_str().is_empty());
        }
    }

    #[test]
    fn command_plan_and_agents_are_first_class_fields() {
        let taproom = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "taproom")
            .unwrap();

        assert_eq!(taproom.agents_count(), 3);
        assert!(!taproom.command_plan.install.is_empty());
        assert_eq!(taproom.version_label(), "0.1.0");
        assert_eq!(taproom.update_label(), "current");
    }
}
