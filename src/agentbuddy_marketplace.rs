use std::{cell::RefCell, collections::VecDeque, path::PathBuf, process::Command};

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
pub const DEFAULT_AGENTBUDDY_SEARCH_PATH: &str = "/api/v1/package/search/skills";
pub const DEFAULT_AGENTBUDDY_DETAIL_PATH: &str = "/api/v1/package";
pub const DEFAULT_AGENTBUDDY_SPACE_SEARCH_PATH: &str = "/api/v1/group/search/skills";
pub const DEFAULT_AGENTBUDDY_SCOPE: &str = "skills.byted.org/default/public";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AgentBuddyMarketplaceConfig {
    pub id: String,
    pub api_base: String,
    pub portal_url: String,
    pub search_path: String,
    pub detail_path: String,
    pub space_search_path: String,
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
            space_search_path: DEFAULT_AGENTBUDDY_SPACE_SEARCH_PATH.to_string(),
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

impl<C: HttpClient> AgentBuddyMarketplaceAdapter<C> {
    pub fn search_spaces(&self, term: &str) -> SourceAdapterResult<Vec<AgentBuddySpace>> {
        let url = self.space_search_url(term);
        let response = self
            .client
            .get(&url)
            .map_err(|error| SourceError::new(SourceErrorKind::NetworkDegraded, error.message))?;
        if response.status >= 400 {
            return Err(SourceError::from_http_status(
                response.status,
                "agentbuddy space search",
            ));
        }

        let value: Value = serde_json::from_str(&response.body)
            .map_err(|error| SourceError::new(SourceErrorKind::Schema, error.to_string()))?;
        let items = extract_items(&value).ok_or_else(|| {
            SourceError::new(
                SourceErrorKind::Schema,
                "space search response missing items",
            )
        })?;
        let mut spaces = items
            .iter()
            .map(agentbuddy_space)
            .collect::<SourceAdapterResult<Vec<_>>>()?;
        spaces.sort_by(|left, right| {
            right
                .package_count
                .cmp(&left.package_count)
                .then_with(|| left.label.cmp(&right.label))
        });
        Ok(spaces)
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
        let group = query.scope.as_deref().unwrap_or(&self.config.scope);
        let keyword = encode_query(query.term.trim());
        format!(
            "{}{}/{}?group={}&page=1&page_size=30&order_by={}",
            self.config.api_base.trim_end_matches('/'),
            self.config.search_path,
            keyword,
            encode_query(group),
            query.order_by.api_value()
        )
    }

    fn detail_url(&self, request: &SourceDetailRequest) -> String {
        format!(
            "{}{}/{}",
            self.config.api_base.trim_end_matches('/'),
            self.config.detail_path,
            encode_query(&request.id)
        )
    }

    fn space_search_url(&self, term: &str) -> String {
        format!(
            "{}{}?q={}&page=1&page_size=100",
            self.config.api_base.trim_end_matches('/'),
            self.config.space_search_path,
            encode_query(term.trim())
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AgentBuddySpace {
    pub id: String,
    pub label: String,
    pub scope: String,
    pub url: String,
    pub package_count: usize,
    pub can_view: bool,
    pub can_download: bool,
}

pub trait HttpClient {
    fn get(&self, url: &str) -> Result<HttpResponse, HttpError>;
}

impl<T: HttpClient + ?Sized> HttpClient for &T {
    fn get(&self, url: &str) -> Result<HttpResponse, HttpError> {
        (**self).get(url)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CurlHttpClient {
    bin: String,
    timeout_seconds: u64,
}

impl Default for CurlHttpClient {
    fn default() -> Self {
        Self {
            bin: "curl".to_string(),
            timeout_seconds: 4,
        }
    }
}

impl CurlHttpClient {
    pub fn new(bin: impl Into<String>, timeout_seconds: u64) -> Self {
        Self {
            bin: bin.into(),
            timeout_seconds,
        }
    }
}

impl HttpClient for CurlHttpClient {
    fn get(&self, url: &str) -> Result<HttpResponse, HttpError> {
        let timeout = self.timeout_seconds.to_string();
        let output = Command::new(&self.bin)
            .args([
                "-sS",
                "-L",
                "--max-time",
                &timeout,
                "-w",
                "\n__SKILLROOM_STATUS__:%{http_code}",
                url,
            ])
            .output()
            .map_err(|error| HttpError::new(format!("{} failed: {error}", self.bin)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !output.status.success() {
            return Err(HttpError::new(if stderr.is_empty() {
                format!("{} exited without status", self.bin)
            } else {
                stderr
            }));
        }

        let Some((body, status)) = stdout.rsplit_once("\n__SKILLROOM_STATUS__:") else {
            return Err(HttpError::new("curl status marker missing"));
        };
        let status = status
            .trim()
            .parse::<u16>()
            .map_err(|error| HttpError::new(format!("invalid curl http status: {error}")))?;
        Ok(HttpResponse::json(status, body))
    }
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
    let id = string_field(item, &["identifier", "resource_id", "artifact_id", "id"])
        .ok_or_else(|| SourceError::new(SourceErrorKind::Schema, "marketplace skill missing id"))?;
    let name = string_field(item, &["name", "display_name", "title"]).ok_or_else(|| {
        SourceError::new(SourceErrorKind::Schema, "marketplace skill missing name")
    })?;
    let description = non_empty_string_field(item, &["description", "description_ai", "summary"])
        .unwrap_or_default();
    let version = version_field(item);
    let star_count = unsigned_field(item, &["star_count", "stars"]);
    let download_total = unsigned_field(item, &["download_total", "downloads"]);
    let installable = bool_field(item, &["installable"])
        .unwrap_or_else(|| !bool_field(item, &["no_permission"]).unwrap_or(false));
    let agents = agents_field(item);
    let compatible_agents = agents.iter().map(|agent| agent.name.clone()).collect();
    let mut tags = strings_field(item, "tags");
    tags.extend(strings_field(item, "labels"));
    tags.extend(strings_field(item, "categories"));
    if let Some(stars) = star_count {
        tags.push(format!("stars:{stars}"));
    }
    if let Some(downloads) = download_total {
        tags.push(format!("downloads:{downloads}"));
    }
    if let Some(namespace) = string_field(item, &["namespace", "group"]) {
        tags.push(format!("space:{namespace}"));
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
        update: download_total
            .map(|downloads| downloads.to_string())
            .or_else(|| Some("remote".to_string())),
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
        error: None,
    })
}

fn agentbuddy_space(item: &Value) -> SourceAdapterResult<AgentBuddySpace> {
    let scope = string_field(item, &["name", "namespace"]).ok_or_else(|| {
        SourceError::new(
            SourceErrorKind::Schema,
            "agentbuddy space missing namespace",
        )
    })?;
    let label = space_label_from_scope(&scope);
    let package_count = unsigned_field(item, &["package_count"]).unwrap_or_default() as usize;
    let can_view = item
        .pointer("/user_permission/view")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let can_download = item
        .pointer("/user_permission/download")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(AgentBuddySpace {
        id: space_id_from_scope(&scope),
        label,
        url: format!("https://skills.bytedance.net/space/{scope}"),
        scope,
        package_count,
        can_view,
        can_download,
    })
}

fn space_id_from_scope(scope: &str) -> String {
    space_label_from_scope(scope)
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn space_label_from_scope(scope: &str) -> String {
    scope
        .strip_prefix("skills.byted.org/")
        .unwrap_or(scope)
        .to_string()
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
        .or_else(|| value.get("data").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/items").and_then(Value::as_array))
        .or_else(|| value.pointer("/data/results").and_then(Value::as_array))
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| string_at(value.get(*key)?))
}

fn string_at(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .or_else(|| value.as_i64().map(|value| value.to_string()))
}

fn non_empty_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| string_at(value.get(*key)?))
        .find(|value| !value.trim().is_empty())
}

fn version_field(value: &Value) -> Option<String> {
    string_field(value, &["version"])
        .or_else(|| value.pointer("/newest_version/version").and_then(string_at))
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
    fn search_builds_space_group_request_and_maps_records() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"count":15,"data":[{"id":609863,"identifier":"skills:skills.byted.org/qianchuan/fe/qc-component-workflow","name":"qc-component-workflow","description":"Component workflow","newest_version":{"version":"1.0.1"},"namespace":"skills.byted.org/qianchuan/fe","group":"skills.byted.org/qianchuan/fe","stars":2,"download_total":96,"labels":["sequence::RD"],"no_permission":false}]}"#,
        ))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);
        let mut query = SourceQuery::new("code review");
        query.scope = Some("skills.byted.org/qianchuan/fe".to_string());
        query.order_by = SourceOrder::StarDesc;

        let records = adapter.search(&query).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "qc-component-workflow");
        assert_eq!(records[0].source, Source::InternalRegistry);
        assert_eq!(records[0].state, SkillState::Installable);
        assert_eq!(records[0].error, None);
        assert_eq!(records[0].version.as_deref(), Some("1.0.1"));
        assert_eq!(records[0].update.as_deref(), Some("96"));
        assert_eq!(
            records[0].path,
            PathBuf::from(
                "agentbuddy://skills:skills.byted.org/qianchuan/fe/qc-component-workflow"
            )
        );
        assert_eq!(
            records[0].command_plan.install[0],
            "agentbuddy skill add skills:skills.byted.org/qianchuan/fe/qc-component-workflow --all"
        );
        assert!(records[0].tags.contains(&"stars:2".to_string()));
        assert!(records[0].tags.contains(&"downloads:96".to_string()));
        assert!(records[0].tags.contains(&"sequence::RD".to_string()));
        assert!(
            records[0]
                .tags
                .contains(&"space:skills.byted.org/qianchuan/fe".to_string())
        );
        assert_eq!(
            adapter.client.urls()[0],
            "https://artifact-api.byted.org/api/v1/package/search/skills/code%20review?group=skills.byted.org%2Fqianchuan%2Ffe&page=1&page_size=30&order_by=star_desc"
        );
    }

    #[test]
    fn search_returns_head_skills_by_star_count() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":[{"identifier":"low","name":"low","stars":3},{"identifier":"top","name":"top","stars":99},{"identifier":"mid","name":"mid","stars":10}]}"#,
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
    fn search_spaces_builds_group_search_request_and_maps_permissions() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":[{"name":"skills.byted.org/qianchuan/pc","package_count":0,"user_permission":{"view":false,"download":false}},{"name":"skills.byted.org/qianchuan/fe","package_count":15,"user_permission":{"view":true,"download":true}}]}"#,
        ))]);
        let adapter =
            AgentBuddyMarketplaceAdapter::new(AgentBuddyMarketplaceConfig::default(), client);

        let spaces = adapter.search_spaces("qianchuan").unwrap();

        assert_eq!(spaces.len(), 2);
        assert_eq!(spaces[0].id, "qianchuan-fe");
        assert_eq!(spaces[0].label, "qianchuan/fe");
        assert_eq!(spaces[0].scope, "skills.byted.org/qianchuan/fe");
        assert_eq!(spaces[0].package_count, 15);
        assert!(spaces[0].can_view);
        assert!(spaces[0].can_download);
        assert_eq!(
            adapter.client.urls()[0],
            "https://artifact-api.byted.org/api/v1/group/search/skills?q=qianchuan&page=1&page_size=100"
        );
    }

    #[test]
    fn detail_maps_single_record_from_data_envelope() {
        let client = MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"data":{"identifier":"skills:data","name":"data-analysis","summary":"Analyze data","agents":[{"name":"codex"}],"installable":false}}"#,
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
