use std::fmt;

use crate::{
    local_inventory::{SkillRoot, load_local_inventory},
    skill::SkillRecord,
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
}
