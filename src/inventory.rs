use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::{
    parser::{ParseError, ParsedSkillMarkdown},
    scan::{DirectoryScan, ScanError},
    skill::{CommandPlan, RiskLevel, SkillRecord, SkillScope, SkillState, SkillStats, Source},
};

#[derive(Debug)]
pub struct LocalSkillData {
    pub path: PathBuf,
    pub scan: Result<DirectoryScan, ScanError>,
    pub parsed: Result<ParsedSkillMarkdown, ParseError>,
}

pub fn merge_inventory(
    list_records: Vec<SkillRecord>,
    local_skills: Vec<LocalSkillData>,
) -> Vec<SkillRecord> {
    let mut by_path: BTreeMap<PathBuf, SkillRecord> = list_records
        .into_iter()
        .map(|record| (normalize_path(&record.path), record))
        .collect();
    let mut seen_local = BTreeSet::new();

    for local in local_skills {
        let key = normalize_path(&local.path);
        seen_local.insert(key.clone());
        let base = by_path.remove(&key);
        let merged = merge_local_skill(base, local);
        by_path.insert(key, merged);
    }

    for (path, record) in &mut by_path {
        if !seen_local.contains(path) {
            record.state = SkillState::Unknown;
            record.error = Some("listed by npx skills but missing from local scan".to_string());
        }
    }

    let mut records: Vec<SkillRecord> = by_path.into_values().collect();
    records.sort_by(|left, right| left.name.cmp(&right.name));
    records
}

fn merge_local_skill(base: Option<SkillRecord>, local: LocalSkillData) -> SkillRecord {
    let mut record = base.unwrap_or_else(|| local_only_record(&local.path));
    record.path = local.path.clone();

    match local.scan {
        Ok(scan) => {
            record.stats = scan.stats;
            record.scripts = scan.scripts;
            record.tags.extend(scan.references);
            record.tags.extend(scan.assets);
        }
        Err(error) => {
            record.state = SkillState::Error;
            record.error = Some(error.to_string());
            return record;
        }
    }

    match local.parsed {
        Ok(parsed) => {
            if let Some(name) = parsed.frontmatter.get("name") {
                record.name = name.clone();
            }
            if let Some(description) = parsed.description {
                record.description = description;
            }
            if !parsed.agents.is_empty() {
                record.agents = parsed.agents;
            }
            record.tags.extend(parsed.referenced_resources);
            record.tags.sort();
            record.tags.dedup();

            if record.state != SkillState::Installed {
                record.state = SkillState::LocalOnly;
            }
            record.error = None;
        }
        Err(error) => {
            record.state = SkillState::Error;
            record.error = Some(error.to_string());
        }
    }

    record
}

fn local_only_record(path: &Path) -> SkillRecord {
    SkillRecord {
        name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string(),
        source: Source::LocalGit,
        scope: SkillScope::Local,
        agents: Vec::new(),
        state: SkillState::LocalOnly,
        risk: RiskLevel::None,
        version: None,
        update: Some("local".to_string()),
        path: path.to_path_buf(),
        description: String::new(),
        scripts: Vec::new(),
        tags: Vec::new(),
        command_plan: CommandPlan::default(),
        stats: SkillStats::default(),
        error: None,
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components().collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        parser::parse_skill_markdown,
        scan::DirectoryScan,
        skill::{Agent, Source},
    };

    use super::*;

    #[test]
    fn merges_list_scan_and_parser_into_installed_record() {
        let path = PathBuf::from("/tmp/skills/code-review");
        let list_record = listed_record("code-review", path.clone());
        let local = local_data(path.clone(), Ok(parsed("code-review")));

        let records = merge_inventory(vec![list_record], vec![local]);
        let record = &records[0];

        assert_eq!(record.state, SkillState::Installed);
        assert_eq!(record.name, "code-review");
        assert_eq!(record.description, "Review code.");
        assert_eq!(record.stats.files, 2);
        assert_eq!(record.scripts, vec!["scripts/run.sh"]);
        assert_eq!(record.agents_count(), 1);
    }

    #[test]
    fn local_only_skill_is_preserved() {
        let path = PathBuf::from("/tmp/skills/local-only");
        let records = merge_inventory(Vec::new(), vec![local_data(path, Ok(parsed("local-only")))]);

        assert_eq!(records[0].state, SkillState::LocalOnly);
        assert_eq!(records[0].source, Source::LocalGit);
    }

    #[test]
    fn parser_error_marks_only_that_skill_as_error() {
        let broken = PathBuf::from("/tmp/skills/broken");
        let good = PathBuf::from("/tmp/skills/good");
        let records = merge_inventory(
            vec![
                listed_record("broken", broken.clone()),
                listed_record("good", good.clone()),
            ],
            vec![
                local_data(broken, Err(ParseError::Empty)),
                local_data(good, Ok(parsed("good"))),
            ],
        );

        assert_eq!(records.len(), 2);
        assert!(
            records
                .iter()
                .any(|record| record.state == SkillState::Error)
        );
        assert!(
            records
                .iter()
                .any(|record| record.state == SkillState::Installed)
        );
    }

    #[test]
    fn list_only_skill_becomes_unknown() {
        let path = PathBuf::from("/tmp/skills/missing");
        let records = merge_inventory(vec![listed_record("missing", path)], Vec::new());

        assert_eq!(records[0].state, SkillState::Unknown);
        assert!(records[0].error.as_deref().unwrap().contains("missing"));
    }

    fn listed_record(name: &str, path: PathBuf) -> SkillRecord {
        SkillRecord {
            name: name.to_string(),
            source: Source::Npx,
            scope: SkillScope::Local,
            agents: vec![Agent::enabled("listed")],
            state: SkillState::Installed,
            risk: RiskLevel::None,
            version: None,
            update: Some("current".to_string()),
            path,
            description: String::new(),
            scripts: Vec::new(),
            tags: Vec::new(),
            command_plan: CommandPlan::default(),
            stats: SkillStats::default(),
            error: None,
        }
    }

    fn local_data(
        path: PathBuf,
        parsed: Result<ParsedSkillMarkdown, ParseError>,
    ) -> LocalSkillData {
        LocalSkillData {
            path: path.clone(),
            scan: Ok(DirectoryScan {
                root: path,
                stats: SkillStats {
                    files: 2,
                    directories: 1,
                    references: 1,
                    assets: 0,
                    line_count: 20,
                    modified_unix_seconds: Some(1),
                },
                scripts: vec!["scripts/run.sh".to_string()],
                references: vec!["references/guide.md".to_string()],
                assets: Vec::new(),
            }),
            parsed,
        }
    }

    fn parsed(name: &str) -> ParsedSkillMarkdown {
        parse_skill_markdown(&format!(
            r#"---
name: {name}
description: "Review code."
agents: ["agent-a"]
---

# Skill
"#
        ))
        .unwrap()
    }
}
