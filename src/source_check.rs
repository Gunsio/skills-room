use crate::{
    agentbuddy::{AgentBuddyCli, SystemCommandRunner},
    config::SourceSettings,
    source::{SourceCheck, SourceCheckKind},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SourceCheckReport {
    pub checks: Vec<SourceCheck>,
}

impl SourceCheckReport {
    pub fn status_line(&self) -> String {
        self.checks
            .iter()
            .map(|check| format!("{:?}:{}", check.kind, check.status.label()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn output_line(&self) -> String {
        self.checks
            .iter()
            .map(|check| {
                format!(
                    "{:?}={} ({})",
                    check.kind,
                    check.status.label(),
                    check.message
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub fn check_source_settings(source: &SourceSettings) -> SourceCheckReport {
    if is_agentbuddy_portal(source) {
        check_agentbuddy_source()
    } else {
        check_custom_source(source)
    }
}

fn check_agentbuddy_source() -> SourceCheckReport {
    let cli = AgentBuddyCli::default();
    let runner = SystemCommandRunner;
    let cli_report = cli.detect(&runner);
    let mut checks = Vec::new();
    checks.extend(cli_report.checks);
    checks.push(SourceCheck::warn(
        SourceCheckKind::Api,
        "artifact-api.byted.org requires authenticated AgentBuddy API access",
    ));
    checks.push(SourceCheck::skipped(
        SourceCheckKind::Download,
        "download/install fallback is out of M4 source test scope",
    ));

    SourceCheckReport { checks }
}

fn check_custom_source(source: &SourceSettings) -> SourceCheckReport {
    SourceCheckReport {
        checks: vec![
            SourceCheck::skipped(
                SourceCheckKind::Cli,
                "custom source does not require AgentBuddy CLI",
            ),
            SourceCheck::skipped(
                SourceCheckKind::Auth,
                "custom source auth is not configured",
            ),
            SourceCheck::warn(
                SourceCheckKind::Api,
                format!("{} is configured but not verified", source.url),
            ),
            SourceCheck::skipped(
                SourceCheckKind::Download,
                "download/install fallback is out of M4 source test scope",
            ),
        ],
    }
}

fn is_agentbuddy_portal(source: &SourceSettings) -> bool {
    source.name == "bytedance-agentbuddy"
        || source.url.trim_end_matches('/') == "https://skills.bytedance.net"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SourceSettings;

    #[test]
    fn custom_source_check_never_claims_verified_remote() {
        let report = check_source_settings(&SourceSettings::custom(1));

        assert!(report.status_line().contains("Api:warn"));
        assert!(report.output_line().contains("not verified"));
    }

    #[test]
    fn agentbuddy_portal_check_includes_api_and_download_boundaries() {
        let report = check_source_settings(&SourceSettings::bytedance());

        assert!(
            report
                .checks
                .iter()
                .any(|check| check.kind == SourceCheckKind::Api)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.kind == SourceCheckKind::Download)
        );
    }
}
