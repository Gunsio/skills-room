use std::{path::PathBuf, process::Command};

use serde_json::Value;

use crate::{
    agentbuddy_marketplace::{CurlHttpClient, HttpClient, HttpResponse},
    skill::{
        CommandPlan, RiskLevel, SkillMetadata, SkillRecord, SkillScope, SkillState, SkillStats,
        Source,
    },
    source::{
        SourceAdapter, SourceAdapterResult, SourceCheck, SourceCheckKind, SourceDetailRequest,
        SourceError, SourceErrorKind, SourceOrder, SourceQuery,
    },
};

pub const DEFAULT_OPENAI_SKILLS_API_URL: &str =
    "https://api.github.com/repos/openai/skills/contents/skills/.curated?ref=main";
pub const DEFAULT_OPENAI_SKILLS_REPO: &str = "openai/skills";
pub const DEFAULT_OPENAI_SKILLS_PATH: &str = "skills/.curated";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OpenAiSkillsConfig {
    pub id: String,
    pub api_url: String,
    pub repo: String,
    pub path: String,
}

impl Default for OpenAiSkillsConfig {
    fn default() -> Self {
        Self {
            id: "openai-curated".to_string(),
            api_url: DEFAULT_OPENAI_SKILLS_API_URL.to_string(),
            repo: DEFAULT_OPENAI_SKILLS_REPO.to_string(),
            path: DEFAULT_OPENAI_SKILLS_PATH.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct OpenAiSkillsAdapter<C> {
    config: OpenAiSkillsConfig,
    client: C,
}

impl<C> OpenAiSkillsAdapter<C> {
    pub fn new(config: OpenAiSkillsConfig, client: C) -> Self {
        Self { config, client }
    }
}

impl<C: HttpClient> SourceAdapter for OpenAiSkillsAdapter<C> {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn health(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::pass(
            SourceCheckKind::Api,
            format!("{}/{} configured", self.config.repo, self.config.path),
        )])
    }

    fn auth(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::skipped(
            SourceCheckKind::Auth,
            "public GitHub source; gh auth is used when available",
        )])
    }

    fn schema(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::pass(
            SourceCheckKind::Schema,
            "GitHub contents entries map to curated skills",
        )])
    }

    fn search(&self, query: &SourceQuery) -> SourceAdapterResult<Vec<SkillRecord>> {
        let response = self
            .client
            .get(&self.config.api_url)
            .map_err(|error| SourceError::new(SourceErrorKind::NetworkDegraded, error.message))?;
        if response.status >= 400 {
            return Err(SourceError::from_http_status(
                response.status,
                "openai skills search",
            ));
        }

        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| SourceError::new(SourceErrorKind::Schema, error.to_string()))?;
        let items = value.as_array().ok_or_else(|| {
            SourceError::new(SourceErrorKind::Schema, "contents list is not an array")
        })?;

        let term = query.term.trim().to_ascii_lowercase();
        let mut records = items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("dir"))
            .map(|item| curated_skill_record(item, &self.config))
            .collect::<SourceAdapterResult<Vec<_>>>()?;
        if !term.is_empty() {
            records.retain(|record| {
                record.name.to_ascii_lowercase().contains(&term)
                    || record.description.to_ascii_lowercase().contains(&term)
                    || record
                        .tags
                        .iter()
                        .any(|tag| tag.to_ascii_lowercase().contains(&term))
            });
        }
        sort_records(&mut records, query.order_by);
        Ok(records)
    }

    fn detail(&self, request: &SourceDetailRequest) -> SourceAdapterResult<SkillRecord> {
        let mut query = SourceQuery::new(&request.id);
        query.order_by = SourceOrder::NameAsc;
        self.search(&query)?
            .into_iter()
            .find(|record| {
                record.name == request.id
                    || record.metadata.source_id.as_deref() == Some(&request.id)
            })
            .ok_or_else(|| SourceError::new(SourceErrorKind::Schema, "curated skill not found"))
    }
}

fn curated_skill_record(
    item: &Value,
    config: &OpenAiSkillsConfig,
) -> SourceAdapterResult<SkillRecord> {
    let name = string_field(item, "name")
        .ok_or_else(|| SourceError::new(SourceErrorKind::Schema, "curated skill missing name"))?;
    let path = string_field(item, "path").unwrap_or_else(|| format!("{}/{}", config.path, name));
    let html_url = string_field(item, "html_url")
        .unwrap_or_else(|| format!("https://github.com/{}/tree/main/{path}", config.repo));
    let sha = string_field(item, "sha");

    Ok(SkillRecord {
        name: name.clone(),
        source: Source::Curated,
        scope: SkillScope::Global,
        agents: Vec::new(),
        state: SkillState::RemoteOnly,
        risk: RiskLevel::None,
        version: sha.as_ref().map(|value| value.chars().take(7).collect()),
        update: Some("remote".to_string()),
        path: PathBuf::from(&html_url),
        description: format!("OpenAI curated Codex skill from {}/{}.", config.repo, path),
        scripts: Vec::new(),
        tags: vec![
            "official".to_string(),
            "openai".to_string(),
            format!("repo:{}", config.repo),
            format!("path:{path}"),
        ],
        command_plan: CommandPlan::default(),
        stats: SkillStats::default(),
        metadata: SkillMetadata {
            source_id: Some(path),
            source_status: Some(config.id.clone()),
            repository: Some(config.repo.clone()),
            repository_type: Some("github".to_string()),
            git_repo: Some(format!("https://github.com/{}", config.repo)),
            homepage: Some(html_url),
            official: Some(true),
            view_public: Some(true),
            ..SkillMetadata::default()
        },
        error: None,
    })
}

fn sort_records(records: &mut [SkillRecord], order: SourceOrder) {
    match order {
        SourceOrder::StarDesc | SourceOrder::NameAsc => {
            records.sort_by(|left, right| left.name.cmp(&right.name));
        }
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(ToString::to_string)
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct GhApiHttpClient {
    fallback: CurlHttpClient,
}

impl HttpClient for GhApiHttpClient {
    fn get(&self, url: &str) -> Result<HttpResponse, crate::agentbuddy_marketplace::HttpError> {
        if let Some(path) = url.strip_prefix("https://api.github.com/") {
            let output = Command::new("gh")
                .args(["api", path])
                .output()
                .map_err(|error| {
                    crate::agentbuddy_marketplace::HttpError::new(format!("gh failed: {error}"))
                })?;
            if output.status.success() {
                return Ok(HttpResponse::json(
                    200,
                    String::from_utf8_lossy(&output.stdout).to_string(),
                ));
            }
        }

        self.fallback.get(url)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        agentbuddy_marketplace::{HttpResponse, MockHttpClient},
        source::{SourceOrder, SourceQuery},
    };

    use super::*;

    #[test]
    fn openai_curated_source_maps_github_contents_to_records() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"[{"name":"openai-docs","path":"skills/.curated/openai-docs","sha":"4a774545829d91404b3615f8f71011a1ed857e92","html_url":"https://github.com/openai/skills/tree/main/skills/.curated/openai-docs","type":"dir"},{"name":"README.md","type":"file"}]"#,
        ))]);
        let adapter = OpenAiSkillsAdapter::new(OpenAiSkillsConfig::default(), client);

        let records = adapter.search(&SourceQuery::new("openai")).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "openai-docs");
        assert_eq!(records[0].source, Source::Curated);
        assert_eq!(records[0].state, SkillState::RemoteOnly);
        assert_eq!(
            records[0].metadata.source_status.as_deref(),
            Some("openai-curated")
        );
        assert_eq!(records[0].metadata.official, Some(true));
        assert!(records[0].command_plan.install.is_empty());
    }

    #[test]
    fn openai_curated_source_sorts_by_name() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"[{"name":"zeta","type":"dir"},{"name":"alpha","type":"dir"}]"#,
        ))]);
        let adapter = OpenAiSkillsAdapter::new(OpenAiSkillsConfig::default(), client);
        let mut query = SourceQuery::new("");
        query.order_by = SourceOrder::StarDesc;

        let records = adapter.search(&query).unwrap();

        assert_eq!(
            records
                .iter()
                .map(|record| record.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
    }
}
