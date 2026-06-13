use std::{fmt, path::PathBuf, process::Command};

use serde::Deserialize;

use crate::skill::{
    Agent, CommandPlan, RiskLevel, SkillRecord, SkillScope, SkillState, SkillStats, Source,
};

#[derive(Debug, Clone)]
pub struct NpxSkillsLoader {
    command: String,
}

impl Default for NpxSkillsLoader {
    fn default() -> Self {
        Self {
            command: "npx".to_string(),
        }
    }
}

impl NpxSkillsLoader {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn load(&self) -> Result<Vec<SkillRecord>, NpxLoaderError> {
        let output = Command::new(&self.command)
            .args(["skills", "list", "--json"])
            .output()
            .map_err(NpxLoaderError::Io)?;

        if !output.status.success() {
            return Err(NpxLoaderError::CommandFailed {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        parse_npx_skills_json(&output.stdout)
    }
}

pub fn parse_npx_skills_json(input: impl AsRef<[u8]>) -> Result<Vec<SkillRecord>, NpxLoaderError> {
    let payload: NpxPayload =
        serde_json::from_slice(input.as_ref()).map_err(NpxLoaderError::Json)?;
    let entries = match payload {
        NpxPayload::Array(entries) => entries,
        NpxPayload::Envelope { skills } => skills,
    };

    Ok(entries
        .into_iter()
        .map(NpxSkillEntry::into_record)
        .collect())
}

#[derive(Debug)]
pub enum NpxLoaderError {
    Io(std::io::Error),
    Json(serde_json::Error),
    CommandFailed { status: Option<i32>, stderr: String },
}

impl fmt::Display for NpxLoaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to execute npx skills: {error}"),
            Self::Json(error) => write!(formatter, "failed to parse npx skills JSON: {error}"),
            Self::CommandFailed { status, stderr } => {
                write!(
                    formatter,
                    "npx skills exited with status {:?}: {}",
                    status, stderr
                )
            }
        }
    }
}

impl std::error::Error for NpxLoaderError {}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NpxPayload {
    Array(Vec<NpxSkillEntry>),
    Envelope { skills: Vec<NpxSkillEntry> },
}

#[derive(Debug, Deserialize)]
struct NpxSkillEntry {
    name: String,
    path: PathBuf,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    agents: AgentField,
}

impl NpxSkillEntry {
    fn into_record(self) -> SkillRecord {
        SkillRecord {
            name: self.name,
            source: Source::Npx,
            scope: parse_scope(self.scope.as_deref()),
            agents: self.agents.into_agents(),
            state: SkillState::Installed,
            risk: RiskLevel::None,
            version: None,
            update: Some("current".to_string()),
            path: self.path,
            description: String::new(),
            scripts: Vec::new(),
            tags: Vec::new(),
            command_plan: CommandPlan::default(),
            stats: SkillStats::default(),
            error: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum AgentField {
    Count(usize),
    Names(Vec<String>),
    Objects(Vec<AgentObject>),
    #[default]
    Missing,
}

impl AgentField {
    fn into_agents(self) -> Vec<Agent> {
        match self {
            Self::Count(count) => (1..=count)
                .map(|index| Agent::enabled(format!("agent-{index}")))
                .collect(),
            Self::Names(names) => names.into_iter().map(Agent::enabled).collect(),
            Self::Objects(objects) => objects
                .into_iter()
                .map(|agent| Agent {
                    name: agent.name,
                    enabled: agent.enabled.unwrap_or(true),
                })
                .collect(),
            Self::Missing => Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AgentObject {
    name: String,
    #[serde(default)]
    enabled: Option<bool>,
}

fn parse_scope(scope: Option<&str>) -> SkillScope {
    match scope.unwrap_or("local").to_ascii_lowercase().as_str() {
        "global" => SkillScope::Global,
        "project" => SkillScope::Project,
        _ => SkillScope::Local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_payload_with_agent_count() {
        let records = parse_npx_skills_json(
            br#"[{"name":"code-review","path":"/tmp/skills/code-review","scope":"local","agents":2}]"#,
        )
        .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "code-review");
        assert_eq!(records[0].scope, SkillScope::Local);
        assert_eq!(records[0].agents_count(), 2);
        assert_eq!(records[0].source, Source::Npx);
    }

    #[test]
    fn parses_envelope_payload_with_named_agents() {
        let records = parse_npx_skills_json(
            br#"{"skills":[{"name":"data-analysis","path":"/tmp/skills/data-analysis","scope":"global","agents":["analyst","chart"]}]}"#,
        )
        .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].scope, SkillScope::Global);
        assert_eq!(records[0].agents[0].name, "analyst");
        assert_eq!(records[0].agents[1].name, "chart");
    }

    #[test]
    fn parses_agent_objects_and_project_scope() {
        let records = parse_npx_skills_json(
            br#"[{"name":"repo-skill","path":"/repo/.skills/repo-skill","scope":"project","agents":[{"name":"reviewer","enabled":false}]}]"#,
        )
        .unwrap();

        assert_eq!(records[0].scope, SkillScope::Project);
        assert_eq!(records[0].agents_count(), 0);
        assert_eq!(records[0].agents[0].name, "reviewer");
    }

    #[test]
    fn rejects_invalid_json() {
        let error = parse_npx_skills_json(b"not json").unwrap_err();
        assert!(error.to_string().contains("failed to parse"));
    }
}
