use std::{cell::RefCell, collections::BTreeMap, fmt};

use crate::{
    local_inventory::{SkillRoot, load_local_inventory},
    skill::{SkillRecord, SkillState},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceQuery {
    pub term: String,
    pub order_by: SourceOrder,
    pub scope: Option<String>,
    pub compatible_agent: Option<String>,
}

impl SourceQuery {
    pub fn new(term: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            order_by: SourceOrder::StarDesc,
            scope: None,
            compatible_agent: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceOrder {
    StarDesc,
    NameAsc,
}

impl SourceOrder {
    pub const fn api_value(self) -> &'static str {
        match self {
            Self::StarDesc => "star_desc",
            Self::NameAsc => "name_asc",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceDetailRequest {
    pub id: String,
}

impl SourceDetailRequest {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

pub type SourceAdapterResult<T> = Result<T, SourceError>;

pub fn state_from_source_error(error: &SourceError) -> SkillState {
    match error.kind {
        SourceErrorKind::Auth => SkillState::AuthError,
        SourceErrorKind::Schema => SkillState::SchemaError,
        SourceErrorKind::NetworkDegraded | SourceErrorKind::CliMissing => {
            SkillState::NetworkDegraded
        }
        SourceErrorKind::Unsupported => SkillState::Error,
    }
}

pub trait SourceAdapter {
    fn id(&self) -> &str;
    fn health(&self) -> SourceAdapterResult<Vec<SourceCheck>>;
    fn auth(&self) -> SourceAdapterResult<Vec<SourceCheck>>;
    fn schema(&self) -> SourceAdapterResult<Vec<SourceCheck>>;
    fn search(&self, query: &SourceQuery) -> SourceAdapterResult<Vec<SkillRecord>>;
    fn detail(&self, request: &SourceDetailRequest) -> SourceAdapterResult<SkillRecord>;

    fn cache_key(&self, query: &SourceQuery) -> String {
        format!(
            "{}:{}:{}:{}",
            self.id(),
            query.term,
            query.order_by.api_value(),
            query.scope.as_deref().unwrap_or("all")
        )
    }
}

pub fn search_all_sources(
    adapters: &[&dyn SourceAdapter],
    query: &SourceQuery,
    cache: &dyn SourceCache,
) -> SourceSearchOutcome {
    let mut records = Vec::new();
    let mut reports = Vec::new();

    for adapter in adapters {
        let cache_key = adapter.cache_key(query);
        match adapter.search(query) {
            Ok(mut source_records) => {
                cache.put(&cache_key, source_records.clone());
                reports.push(SourceReport::ready(
                    adapter.id(),
                    format!("{} records", source_records.len()),
                ));
                records.append(&mut source_records);
            }
            Err(error) => {
                if let Some(mut cached_records) = cache.get(&cache_key) {
                    reports.push(SourceReport::cached(adapter.id(), error.to_string()));
                    records.append(&mut cached_records);
                } else {
                    reports.push(SourceReport::failed(adapter.id(), error));
                }
            }
        }
    }

    SourceSearchOutcome { records, reports }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceSearchOutcome {
    pub records: Vec<SkillRecord>,
    pub reports: Vec<SourceReport>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceReport {
    pub source_id: String,
    pub status: SourceCheckStatus,
    pub used_cache: bool,
    pub message: String,
}

impl SourceReport {
    fn ready(source_id: &str, message: impl Into<String>) -> Self {
        Self {
            source_id: source_id.to_string(),
            status: SourceCheckStatus::Pass,
            used_cache: false,
            message: message.into(),
        }
    }

    fn cached(source_id: &str, message: impl Into<String>) -> Self {
        Self {
            source_id: source_id.to_string(),
            status: SourceCheckStatus::Warn,
            used_cache: true,
            message: message.into(),
        }
    }

    fn failed(source_id: &str, error: SourceError) -> Self {
        Self {
            source_id: source_id.to_string(),
            status: SourceCheckStatus::Fail,
            used_cache: false,
            message: error.to_string(),
        }
    }
}

pub trait SourceCache {
    fn get(&self, key: &str) -> Option<Vec<SkillRecord>>;
    fn put(&self, key: &str, records: Vec<SkillRecord>);
}

#[derive(Debug, Default)]
pub struct MemorySourceCache {
    entries: RefCell<BTreeMap<String, Vec<SkillRecord>>>,
}

impl MemorySourceCache {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SourceCache for MemorySourceCache {
    fn get(&self, key: &str) -> Option<Vec<SkillRecord>> {
        self.entries.borrow().get(key).cloned()
    }

    fn put(&self, key: &str, records: Vec<SkillRecord>) {
        self.entries.borrow_mut().insert(key.to_string(), records);
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LocalSourceAdapter {
    id: String,
    records: Vec<SkillRecord>,
}

impl LocalSourceAdapter {
    pub fn new(records: Vec<SkillRecord>) -> Self {
        Self {
            id: "local".to_string(),
            records,
        }
    }

    pub fn from_roots(roots: Vec<SkillRoot>) -> Self {
        Self::new(load_local_inventory(roots))
    }

    pub fn records(&self) -> &[SkillRecord] {
        &self.records
    }
}

impl SourceAdapter for LocalSourceAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn health(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::pass(
            SourceCheckKind::Api,
            format!("{} local skills", self.records.len()),
        )])
    }

    fn auth(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        Ok(vec![SourceCheck::skipped(
            SourceCheckKind::Auth,
            "local source does not require auth",
        )])
    }

    fn schema(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
        let invalid = self
            .records
            .iter()
            .filter(|record| record.name.trim().is_empty())
            .count();
        if invalid == 0 {
            Ok(vec![SourceCheck::pass(
                SourceCheckKind::Schema,
                "local records are normalized",
            )])
        } else {
            Err(SourceError::new(
                SourceErrorKind::Schema,
                format!("{invalid} local records are missing names"),
            ))
        }
    }

    fn search(&self, query: &SourceQuery) -> SourceAdapterResult<Vec<SkillRecord>> {
        let term = query.term.trim();
        let mut records: Vec<SkillRecord> = if term.is_empty() {
            self.records.clone()
        } else {
            self.records
                .iter()
                .filter(|record| local_record_matches(record, term))
                .cloned()
                .collect()
        };
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }

    fn detail(&self, request: &SourceDetailRequest) -> SourceAdapterResult<SkillRecord> {
        self.records
            .iter()
            .find(|record| record.name == request.id)
            .cloned()
            .ok_or_else(|| {
                SourceError::new(
                    SourceErrorKind::Unsupported,
                    format!("local skill {} was not found", request.id),
                )
            })
    }
}

fn local_record_matches(record: &SkillRecord, term: &str) -> bool {
    contains_case_insensitive(&record.name, term)
        || contains_case_insensitive(record.source.label(), term)
        || contains_case_insensitive(&record.description, term)
        || record
            .tags
            .iter()
            .any(|tag| contains_case_insensitive(tag, term))
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceCheck {
    pub kind: SourceCheckKind,
    pub status: SourceCheckStatus,
    pub message: String,
}

impl SourceCheck {
    pub fn pass(kind: SourceCheckKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: SourceCheckStatus::Pass,
            message: message.into(),
        }
    }

    pub fn warn(kind: SourceCheckKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: SourceCheckStatus::Warn,
            message: message.into(),
        }
    }

    pub fn fail(kind: SourceCheckKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: SourceCheckStatus::Fail,
            message: message.into(),
        }
    }

    pub fn skipped(kind: SourceCheckKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: SourceCheckStatus::Skipped,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceCheckKind {
    Cli,
    Api,
    Auth,
    Schema,
    Download,
    Cache,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceCheckStatus {
    Pass,
    Warn,
    Fail,
    Skipped,
}

impl SourceCheckStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "fail",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceError {
    pub kind: SourceErrorKind,
    pub message: String,
    pub retryable: bool,
}

impl SourceError {
    pub fn new(kind: SourceErrorKind, message: impl Into<String>) -> Self {
        Self {
            retryable: kind.retryable(),
            kind,
            message: message.into(),
        }
    }

    pub fn from_http_status(status: u16, context: impl Into<String>) -> Self {
        let context = context.into();
        let kind = match status {
            401 | 403 => SourceErrorKind::Auth,
            408 | 429 | 500..=599 => SourceErrorKind::NetworkDegraded,
            _ => SourceErrorKind::Schema,
        };
        Self::new(kind, format!("{context}: http {status}"))
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.kind.label(), self.message)
    }
}

impl std::error::Error for SourceError {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceErrorKind {
    Auth,
    Schema,
    NetworkDegraded,
    CliMissing,
    Unsupported,
}

impl SourceErrorKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auth => "auth error",
            Self::Schema => "schema error",
            Self::NetworkDegraded => "network degraded",
            Self::CliMissing => "cli missing",
            Self::Unsupported => "unsupported source",
        }
    }

    pub const fn retryable(self) -> bool {
        match self {
            Self::Auth | Self::Schema | Self::CliMissing | Self::Unsupported => false,
            Self::NetworkDegraded => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::fixture_skills;

    #[derive(Debug)]
    struct EmptyAdapter;

    impl SourceAdapter for EmptyAdapter {
        fn id(&self) -> &str {
            "mock"
        }

        fn health(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
            Ok(vec![SourceCheck::pass(SourceCheckKind::Api, "ok")])
        }

        fn auth(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
            Ok(vec![SourceCheck::skipped(
                SourceCheckKind::Auth,
                "not required",
            )])
        }

        fn schema(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
            Ok(vec![SourceCheck::pass(SourceCheckKind::Schema, "ok")])
        }

        fn search(&self, _query: &SourceQuery) -> SourceAdapterResult<Vec<SkillRecord>> {
            Ok(Vec::new())
        }

        fn detail(&self, _request: &SourceDetailRequest) -> SourceAdapterResult<SkillRecord> {
            Err(SourceError::new(SourceErrorKind::Unsupported, "no detail"))
        }
    }

    #[derive(Debug)]
    struct FailingAdapter;

    impl SourceAdapter for FailingAdapter {
        fn id(&self) -> &str {
            "remote"
        }

        fn health(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
            Ok(Vec::new())
        }

        fn auth(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
            Ok(Vec::new())
        }

        fn schema(&self) -> SourceAdapterResult<Vec<SourceCheck>> {
            Ok(Vec::new())
        }

        fn search(&self, _query: &SourceQuery) -> SourceAdapterResult<Vec<SkillRecord>> {
            Err(SourceError::new(
                SourceErrorKind::NetworkDegraded,
                "timeout",
            ))
        }

        fn detail(&self, _request: &SourceDetailRequest) -> SourceAdapterResult<SkillRecord> {
            Err(SourceError::new(
                SourceErrorKind::NetworkDegraded,
                "timeout",
            ))
        }
    }

    #[test]
    fn cache_key_includes_source_query_order_and_scope() {
        let mut query = SourceQuery::new("review");
        query.scope = Some("internal".to_string());

        assert_eq!(
            EmptyAdapter.cache_key(&query),
            "mock:review:star_desc:internal"
        );
    }

    #[test]
    fn http_error_mapping_is_stable() {
        assert_eq!(
            SourceError::from_http_status(401, "agentbuddy").kind,
            SourceErrorKind::Auth
        );
        assert_eq!(
            SourceError::from_http_status(503, "agentbuddy").kind,
            SourceErrorKind::NetworkDegraded
        );
        assert_eq!(
            SourceError::from_http_status(422, "agentbuddy").kind,
            SourceErrorKind::Schema
        );
    }

    #[test]
    fn retryability_matches_error_boundaries() {
        assert!(SourceErrorKind::NetworkDegraded.retryable());
        assert!(!SourceErrorKind::Auth.retryable());
        assert!(!SourceErrorKind::Schema.retryable());
    }

    #[test]
    fn source_errors_map_to_visible_skill_states() {
        assert_eq!(
            state_from_source_error(&SourceError::new(SourceErrorKind::Auth, "auth")),
            SkillState::AuthError
        );
        assert_eq!(
            state_from_source_error(&SourceError::new(SourceErrorKind::Schema, "schema")),
            SkillState::SchemaError
        );
        assert_eq!(
            state_from_source_error(&SourceError::new(
                SourceErrorKind::NetworkDegraded,
                "timeout"
            )),
            SkillState::NetworkDegraded
        );
    }

    #[test]
    fn local_adapter_searches_existing_inventory_fields() {
        let adapter = LocalSourceAdapter::new(fixture_skills());
        let results = adapter.search(&SourceQuery::new("review")).unwrap();

        assert_eq!(results[0].name, "code-review");
    }

    #[test]
    fn local_adapter_detail_reads_by_skill_name() {
        let adapter = LocalSourceAdapter::new(fixture_skills());
        let detail = adapter
            .detail(&SourceDetailRequest::new("web-scraper"))
            .unwrap();

        assert_eq!(detail.name, "web-scraper");
        assert_eq!(detail.source.label(), "github");
    }

    #[test]
    fn local_adapter_checks_are_non_networked() {
        let adapter = LocalSourceAdapter::new(fixture_skills());

        assert_eq!(adapter.health().unwrap()[0].status, SourceCheckStatus::Pass);
        assert_eq!(
            adapter.auth().unwrap()[0].status,
            SourceCheckStatus::Skipped
        );
        assert_eq!(adapter.schema().unwrap()[0].status, SourceCheckStatus::Pass);
    }

    #[test]
    fn source_manager_keeps_local_results_when_remote_fails() {
        let local = LocalSourceAdapter::new(fixture_skills());
        let remote = FailingAdapter;
        let cache = MemorySourceCache::new();
        let outcome = search_all_sources(
            &[&local as &dyn SourceAdapter, &remote as &dyn SourceAdapter],
            &SourceQuery::new("review"),
            &cache,
        );

        assert!(
            outcome
                .records
                .iter()
                .any(|record| record.name == "code-review")
        );
        assert_eq!(outcome.reports.len(), 2);
        assert_eq!(outcome.reports[1].status, SourceCheckStatus::Fail);
    }

    #[test]
    fn source_manager_uses_cache_when_source_degrades() {
        let remote = FailingAdapter;
        let cache = MemorySourceCache::new();
        let query = SourceQuery::new("review");
        cache.put(
            &remote.cache_key(&query),
            vec![
                fixture_skills()
                    .into_iter()
                    .find(|record| record.name == "code-review")
                    .unwrap(),
            ],
        );

        let outcome = search_all_sources(&[&remote as &dyn SourceAdapter], &query, &cache);

        assert_eq!(outcome.records[0].name, "code-review");
        assert!(outcome.reports[0].used_cache);
        assert_eq!(outcome.reports[0].status, SourceCheckStatus::Warn);
    }
}
