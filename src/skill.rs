#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkillScope {
    Local,
    Global,
}

impl SkillScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Global => "Global",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkillState {
    Ready,
    Active,
    UpdateAvailable,
    Error,
}

impl SkillState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Active => "Active",
            Self::UpdateAvailable => "Update",
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
pub struct SkillRecord {
    pub name: &'static str,
    pub source: &'static str,
    pub scope: SkillScope,
    pub agents: u16,
    pub state: SkillState,
    pub risk: RiskLevel,
    pub version: &'static str,
    pub update: &'static str,
    pub path: &'static str,
    pub description: &'static str,
    pub scripts: &'static [&'static str],
    pub tags: &'static [&'static str],
}

pub fn fixture_skills() -> Vec<SkillRecord> {
    vec![
        SkillRecord {
            name: "taproom",
            source: "local/git",
            scope: SkillScope::Local,
            agents: 3,
            state: SkillState::Ready,
            risk: RiskLevel::None,
            version: "0.1.0",
            update: "current",
            path: "~/.skillroom/taproom",
            description: "TUI environment manager used as the benchmark interaction model.",
            scripts: &["init.sh", "build.sh", "run.sh"],
            tags: &["tui", "benchmark", "local"],
        },
        SkillRecord {
            name: "data-analysis",
            source: "skills.bytedance.net",
            scope: SkillScope::Global,
            agents: 5,
            state: SkillState::Active,
            risk: RiskLevel::Low,
            version: "2.4.1",
            update: "current",
            path: "~/.agents/skills/data-analysis",
            description: "Analyze CSV, sheets, metrics, and experiment outputs.",
            scripts: &["check.sh"],
            tags: &["analysis", "metrics", "global"],
        },
        SkillRecord {
            name: "code-review",
            source: "curated",
            scope: SkillScope::Local,
            agents: 2,
            state: SkillState::UpdateAvailable,
            risk: RiskLevel::Medium,
            version: "1.8.0",
            update: "1.9.2",
            path: "~/.codex/skills/code-review",
            description: "Review pull requests with repository-local conventions and test gates.",
            scripts: &["lint.sh", "review.sh"],
            tags: &["code", "review", "quality"],
        },
        SkillRecord {
            name: "web-scraper",
            source: "github",
            scope: SkillScope::Global,
            agents: 1,
            state: SkillState::Ready,
            risk: RiskLevel::Low,
            version: "0.9.3",
            update: "current",
            path: "~/.codex/skills/web-scraper",
            description: "Collect and normalize web data with browser assisted checks.",
            scripts: &["crawl.sh"],
            tags: &["browser", "source", "global"],
        },
        SkillRecord {
            name: "legacy-deploy",
            source: "local/archive",
            scope: SkillScope::Local,
            agents: 0,
            state: SkillState::Error,
            risk: RiskLevel::High,
            version: "0.3.7",
            update: "blocked",
            path: "~/.skillroom/legacy-deploy",
            description: "Archived deployment helper kept for migration visibility.",
            scripts: &["deploy.sh"],
            tags: &["legacy", "deploy", "blocked"],
        },
    ]
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
                .any(|skill| skill.source == "skills.bytedance.net")
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
            assert!(!skill.source.is_empty());
            assert!(!skill.scope.label().is_empty());
            assert!(!skill.state.label().is_empty());
            assert!(!skill.risk.label().is_empty());
        }
    }
}
