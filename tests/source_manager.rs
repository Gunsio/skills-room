use skillroom::{
    agentbuddy_marketplace::{
        AgentBuddyMarketplaceAdapter, AgentBuddyMarketplaceConfig, HttpError, HttpResponse,
        MockHttpClient,
    },
    skill::{SkillRecord, fixture_skills},
    source::{
        MemorySourceCache, SourceAdapter, SourceAdapterResult, SourceCache, SourceCheck,
        SourceDetailRequest, SourceError, SourceErrorKind, SourceQuery, search_all_sources,
    },
};

#[test]
fn m4_source_failure_modes_are_mockable_without_internal_network() {
    let auth = AgentBuddyMarketplaceAdapter::new(
        AgentBuddyMarketplaceConfig::default(),
        MockHttpClient::new(vec![Ok(HttpResponse::json(401, "unauthorized"))]),
    );
    assert_eq!(
        auth.search(&SourceQuery::new("review")).unwrap_err().kind,
        SourceErrorKind::Auth
    );

    let schema = AgentBuddyMarketplaceAdapter::new(
        AgentBuddyMarketplaceConfig::default(),
        MockHttpClient::new(vec![Ok(HttpResponse::json(
            200,
            r#"{"items":[{"description":"missing required id/name"}]}"#,
        ))]),
    );
    assert_eq!(
        schema.search(&SourceQuery::new("review")).unwrap_err().kind,
        SourceErrorKind::Schema
    );

    let timeout = AgentBuddyMarketplaceAdapter::new(
        AgentBuddyMarketplaceConfig::default(),
        MockHttpClient::new(vec![Err(HttpError::new("timeout"))]),
    );
    let error = timeout.search(&SourceQuery::new("review")).unwrap_err();
    assert_eq!(error.kind, SourceErrorKind::NetworkDegraded);
    assert!(error.retryable);
}

#[test]
fn m4_source_cache_fallback_is_available_through_public_api() {
    let adapter = FailingAdapter;
    let cache = MemorySourceCache::new();
    let query = SourceQuery::new("review");
    cache.put(
        &adapter.cache_key(&query),
        vec![
            fixture_skills()
                .into_iter()
                .find(|record| record.name == "code-review")
                .unwrap(),
        ],
    );

    let outcome = search_all_sources(&[&adapter as &dyn SourceAdapter], &query, &cache);

    assert_eq!(outcome.records[0].name, "code-review");
    assert!(outcome.reports[0].used_cache);
}

#[derive(Debug)]
struct FailingAdapter;

impl SourceAdapter for FailingAdapter {
    fn id(&self) -> &str {
        "agentbuddy"
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
