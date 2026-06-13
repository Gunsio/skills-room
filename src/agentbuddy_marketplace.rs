use std::{cell::RefCell, collections::VecDeque, path::PathBuf};

use serde_json::Value;

use crate::{
    skill::{
        Agent, CommandPlan, RiskLevel, SkillMetadata, SkillRecord, SkillScope, SkillState,
        SkillStats, Source,
    },
    source::{
        SourceAdapter, SourceAdapterResult, SourceCheck, SourceCheckKind, SourceDetailRequest,
        SourceError, SourceErrorKind, SourceOrder, SourceQuery,
    },
};

pub const DEFAULT_AGENTBUDDY_API_BASE: &str = "https://artifact-api.byted.org";
pub const DEFAULT_AGENTBUDDY_SEARCH_PATH: &str = "/api/marketplace/search";
pub const DEFAULT_AGENTBUDDY_DETAIL_PATH: &str = "/api/marketplace/detail";
pub const DEFAULT_AGENTBUDDY_SCOPE: &str = "internal";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AgentBuddyMarketplaceConfig {
    pub id: String,
    pub api_base: String,
    pub portal_url: String,
    pub search_path: String,
    pub detail_path: String,
    pub scope: String,
}

impl Default for AgentBuddyMarketplaceConfig {
    fn default() -> Self {
        Self {
            id: "bytedance-agentbuddy".to_string(),
            api_base: DEFAULT_AGENTBUDDY_API_BASE.to_string(),
            portal_url: "https://skills.bytedance.net/".to_string(),
            search_path: DEFAULT_AGENTBUDDY_SEARCH_PATH.to_string(),
            detail_path: DEFAULT_AGENTBUDDY_DETAIL_PATH.to_string(),
            scope: DEFAULT_AGENTBUDDY_SCOPE.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct AgentBuddyMarketplaceAdapter<C> {
    config: AgentBuddyMarketplaceConfig,
    client: C,
}

impl<C> AgentBuddyMarketplaceAdapter<C> {
    pub fn new(config: AgentBuddyMarketplaceConfig, client: C) -> Self {
        Self { config, client }
    }

    pub fn config(&self) -> &AgentBuddyMarketplaceConfig {
        &self.config
    }
}

impl<C: HttpClient> SourceAdapter for AgentBuddyMarketplaceAdapter<C> {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn health(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        match self.client.get(&self.config.api_base) {
            Ok(response) if response.status < 400 => Ok(vec![SourceCheck::pass(
                SourceCheckKind::Api,
                format!("{} reachable", self.config.api_base),
            )]),
            Ok(response) if matches!(response.status, 401 | 403) => Ok(vec![SourceCheck::warn(
                SourceCheckKind::Auth,
                format!("{} requires auth", self.config.api_base),
            )]),
            Ok(response) => Err(SourceError::from_http_status(
                response.status,
                self.config.api_base.clone(),
            )),
            Err(error) => Err(SourceError::new(
                SourceErrorKind::NetworkDegraded,
                error.message,
            )),
        }
    }

    fn auth(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::warn(
            SourceCheckKind::Auth,
            "AgentBuddy API auth is delegated to agentbuddy get-jwt",
        )])
    }

    fn schema(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::pass(
            SourceCheckKind::Schema,
            "mocked marketplace schema accepts id/name and optional metadata",
        )])
    }

    fn search(&self, query: &SourceQuery) -> SourceAdapterResult<Vec<SkillRecord>> {
        let url = self.search_url(query);
        let response = self
            .client
            .get(&url)
            .map_err(|error| SourceError::new(SourceErrorKind::NetworkDegraded, error.message))?;
        if response.status >= 400 {
            return Err(SourceError::from_http_status(
                response.status,
                "agentbuddy search",
            ));
        }

        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| SourceError::new(SourceErrorKind::Schema, error.to_string()))?;
        let items = extract_items(&value).ok_or_else(|| {
            SourceError::new(SourceErrorKind::Schema, "search response missing items")
        })?;
        let mut records: Vec<SkillRecord> = items
            .iter()
            .map(|item| marketplace_skill_record(item, &self.config))
            .collect::<SourceAdapterResult<_>>()?;
        sort_marketplace_records(&mut records, query.order_by);
        Ok(records)
    }

    fn detail(&self, request: &SourceDetailRequest) -> SourceAdapterResult<SkillRecord> {
        let url = self.detail_url(request);
        let response = self
            .client
            .get(&url)
            .map_err(|error| SourceError::new(SourceErrorKind::NetworkDegraded, error.message))?;
        if response.status >= 400 {
            return Err(SourceError::from_http_status(
                response.status,
                "agentbuddy detail",
            ));
        }

        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| SourceError::new(SourceErrorKind::Schema, error.to_string()))?;
        let item = value.get("data").unwrap_or(&value);
        marketplace_skill_record(item, &self.config)
    }
}

impl<C> AgentBuddyMarketplaceAdapter<C> {
    fn search_url(&self, query: &SourceQuery) -> String {
        let scope = query.scope.as_deref().unwrap_or(&self.config.scope);
        format!(
            "{}{}?type=skill&scope={}&q={}&order_by={}",
            self.config.api_base.trim_end_matches('/'),
            self.config.search_path,
            encode_query(scope),
            encode_query(&query.term),
            query.order_by.api_value()
        )
    }

    fn detail_url(&self, request: &SourceDetailRequest) -> String {
        format!(
            "{}{}?type=skill&id={}",
            self.config.api_base.trim_end_matches('/'),
            self.config.detail_path,
            encode_query(&request.id)
        )
    }
}

pub trait HttpClient {
    fn get(&self, url: &str) -> Result<HttpResponse, HttpError>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HttpError {
    pub message: String,
}

impl HttpError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockHttpClient {
    responses: RefCell<VecDeque<Result<HttpResponse, HttpError>>>,
    urls: RefCell<Vec<String>>,
}

impl MockHttpClient {
    pub fn new(responses: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
            urls: RefCell::new(Vec::new()),
        }
    }

    pub fn urls(&self) -> Vec<String> {
        self.urls.borrow().clone()
    }
}

impl HttpClient for MockHttpClient {
    fn get(&self, url: &str) -> Result<HttpResponse, HttpError> {
        self.urls.borrow_mut().push(url.to_string());
        self.responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(HttpError::new("mock response missing")))
    }
}

fn marketplace_skill_record(
    item: &Value,
    config: &AgentBuddyMarketplaceConfig,
) -> SourceAdapterResult<SkillRecord> {
    let id = string_field(item, &["id", "resource_id", "artifact_id"])
        .ok_or_else(|| SourceError::new(SourceErrorKind::Schema, "marketplace skill missing id"))?;
    let name = string_field(item, &["name", "display_name", "title"]).ok_or_else(|| {
        SourceError::new(SourceErrorKind::Schema, "marketplace skill missing name")
    })?;
    let description = string_field(item, &["description", "summary"]).unwrap_or_default();
    let version = string_field(item, &["version"]);
    let star_count = unsigned_field(item, &["star_count", "stars"]);
    let installable = bool_field(item, &["installable"]).unwrap_or(true);
    let agents = agents_field(item);
    let compatible_agents = agents.iter().map(|agent| agent.name.clone()).collect();
    let mut tags = strings_field(item, "tags");
    if let Some(stars) = star_count {
        tags.push(format!("stars:{stars}"));
    }
    if installable {
        tags.push("installable".to_string());
    }

    Ok(SkillRecord {
        name,
        source: Source::InternalRegistry,
        scope: scope_field(item).unwrap_or(SkillScope::Global),
        agents,
        state: if installable {
            SkillState::Installable
        } else {
            SkillState::RemoteOnly
        },
        risk: RiskLevel::None,
        version,
        update: Some("remote".to_string()),
        path: PathBuf::from(format!("agentbuddy://{id}")),
        description,
        scripts: Vec::new(),
        tags,
        command_plan: CommandPlan {
            install: vec![format!("agentbuddy skill add {id} --all")],
            update: Vec::new(),
            remove: Vec::new(),
        },
        stats: SkillStats::default(),
        metadata: SkillMetadata {
            source_id: Some(id),
            star_count,
            installable,
            installed: false,
            compatible_agents,
            source_status: Some(config.id.clone()),
        },
        error: Some(format!("portal={}", config.portal_url)),
    })
}

fn sort_marketplace_records(records: &mut [SkillRecord], order: SourceOrder) {
    match order {
        SourceOrder::StarDesc => records.sort_by(|left, right| {
            right
                .metadata
                .star_count
                .unwrap_or_default()
                .cmp(&left.metadata.star_count.unwrap_or_default())
                .then_with(|| left.name.cmp(&right.name))
        }),
        SourceOrder::NameAsc => records.sort_by(|left, right| left.name.cmp(&right.name)),
    }
}

fn extract_items(value: &Value) -> Option<&Vec<Value>> {
    value
        .as_array()
        .or_else(|| value.get("items").and_then(Value::as_array))
        .or_else(|| value.get("results").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/items").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/results").and_then(Value::as_array))
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn unsigned_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

fn strings_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn agents_field(value: &Value) -> Vec<Agent> {
    let Some(agents) = value.get("agents").and_then(Value::as_array) else {
        return Vec::new();
    };

    agents
        .iter()
        .filter_map(|agent| {
            agent
                .as_str()
                .or_else(|| agent.get("name").and_then(Value::as_str))
                .map(Agent::enabled)
        })
        .collect()
}

fn scope_field(value: &Value) -> Option<SkillScope> {
    let scope = value.get("scope").and_then(Value::as_str)?;
    match scope.to_ascii_lowercase().as_str() {
        "local" => Some(SkillScope::Local),
        "project" => Some(SkillScope::Project),
        "global" | "internal" => Some(SkillScope::Global),
        _ => None,
    }
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceOrder, SourceQuery};

    #[test]
    fn search_builds_internal_scope_request_and_maps_records() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":{"items":[{"id":"skills:code-review","name":"code-review","description":"Review code","version":"1.0.0","scope":"internal","star_count":42,"agents":["codex"],"tags":["quality"],"installable":true}]}}"#,
        ))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);
        let mut query = SourceQuery::new("code review");
        query.order_by = SourceOrder::StarDesc;

        let records = adapter.search(&query).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "code-review");
        assert_eq!(records[0].source, Source::InternalRegistry);
        assert_eq!(records[0].state, SkillState::Installable);
        assert_eq!(
            records[0].path,
            PathBuf::from("agentbuddy://skills:code-review")
        );
        assert_eq!(
            records[0].command_plan.install[0],
            "agentbuddy skill add skills:code-review --all"
        );
        assert!(records[0].tags.contains(&"stars:42".to_string()));
        assert_eq!(
            adapter.client.urls()[0],
            "https://artifact-api.byted.org/api/marketplace/search?type=skill&scope=internal&q=code%20review&order_by=star_desc"
        );
    }

    #[test]
    fn search_returns_head_skills_by_star_count() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"items":[{"id":"low","name":"low","star_count":3},{"id":"top","name":"top","star_count":99},{"id":"mid","name":"mid","star_count":10}]}"#,
        ))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);

        let records = adapter.search(&SourceQuery::new("skill")).unwrap();

        assert_eq!(
            records
                .iter()
                .map(|record| record.name.as_str())
                .collect::<Vec<_>>(),
            vec!["top", "mid", "low"]
        );
        assert_eq!(records[0].metadata.star_count, Some(99));
        assert!(records[0].metadata.installable);
    }

    #[test]
    fn detail_maps_single_record_from_data_envelope() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":{"id":"skills:data","name":"data-analysis","summary":"Analyze data","agents":[{"name":"codex"}],"installable":false}}"#,
        ))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);

        let record = adapter
            .detail(&SourceDetailRequest::new("skills:data"))
            .unwrap();

        assert_eq!(record.name, "data-analysis");
        assert_eq!(record.state, SkillState::RemoteOnly);
        assert_eq!(record.description, "Analyze data");
        assert_eq!(record.agents_count(), 1);
    }

    #[test]
    fn auth_status_maps_to_auth_error() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(401, "unauthorized"))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);

        let error = adapter.search(&SourceQuery::new("anything")).unwrap_err();

        assert_eq!(error.kind, SourceErrorKind::Auth);
    }

    #[test]
    fn schema_drift_is_reported() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":{"items":[{"description":"missing name"}]}}"#,
        ))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);

        let error = adapter.search(&SourceQuery::new("broken")).unwrap_err();

        assert_eq!(error.kind, SourceErrorKind::Schema);
    }

    #[test]
    fn network_failure_is_retryable_degraded_error() {
        let client = MockHttpClient::new(vec![Err(HttpError::new("timeout"))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);

        let error = adapter.search(&SourceQuery::new("slow")).unwrap_err();

        assert_eq!(error.kind, SourceErrorKind::NetworkDegraded);
        assert!(error.retryable);
    }
}
