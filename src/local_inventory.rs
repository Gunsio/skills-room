use std::{collections::BTreeMap, env, fs, path::PathBuf};

use crate::{
    inventory::{LocalSkillData, merge_inventory},
    parser::parse_skill_markdown,
    scan::scan_skill_dir,
    skill::{SkillRecord, SkillScope},
};

pub const SKILL_ROOTS_ENV: &str = "SKILLROOM_SKILL_ROOTS";

pub fn load_local_inventory_from_env() -> Vec<SkillRecord> {
    load_local_inventory(discover_skill_roots())
}

pub fn load_local_inventory(roots: Vec<SkillRoot>) -> Vec<SkillRecord> {
    let scoped_paths: Vec<(SkillScope, PathBuf)> = roots
        .into_iter()
        .flat_map(|root| {
            discover_skill_dirs(&root.path)
                .into_iter()
                .map(move |path| (root.scope, path))
        })
        .collect();
    let scope_by_path: BTreeMap<PathBuf, SkillScope> = scoped_paths
        .iter()
        .map(|(scope, path)| (path.clone(), *scope))
        .collect();
    let local_skills = scoped_paths
        .into_iter()
        .map(|(_, path)| local_skill_data(path))
        .collect();

    let mut records = merge_inventory(Vec::new(), local_skills);
    for record in &mut records {
        if let Some(scope) = scope_by_path.get(&record.path) {
            record.scope = *scope;
        }
    }
    records
}

pub fn discover_skill_roots() -> Vec<SkillRoot> {
    if let Ok(value) = env::var(SKILL_ROOTS_ENV) {
        return env::split_paths(&value)
            .map(|path| SkillRoot {
                path,
                scope: SkillScope::Local,
            })
            .collect();
    }

    let mut roots = Vec::new();
    if let Ok(home) = env::var("HOME") {
        let home = PathBuf::from(home);
        roots.push(SkillRoot {
            path: home.join(".codex/skills"),
            scope: SkillScope::Local,
        });
        roots.push(SkillRoot {
            path: home.join(".agents/skills"),
            scope: SkillScope::Global,
        });
    }
    if let Ok(current_dir) = env::current_dir() {
        roots.push(SkillRoot {
            path: current_dir.join(".skills"),
            scope: SkillScope::Project,
        });
    }

    roots
}

pub fn discover_skill_dirs(root: &PathBuf) -> Vec<PathBuf> {
    if root.join("SKILL.md").is_file() {
        return vec![root.clone()];
    }

    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("SKILL.md").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn local_skill_data(path: PathBuf) -> LocalSkillData {
    let skill_md = path.join("SKILL.md");
    let content = fs::read_to_string(skill_md).unwrap_or_default();
    LocalSkillData {
        path: path.clone(),
        scan: scan_skill_dir(&path),
        parsed: parse_skill_markdown(&content),
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SkillRoot {
    pub path: PathBuf,
    pub scope: SkillScope,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn discovers_child_skill_directories() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("skills");
        fs::create_dir_all(root.join("one")).unwrap();
        fs::create_dir_all(root.join("two")).unwrap();
        fs::write(root.join("one/SKILL.md"), "# One\n\nBody\n").unwrap();
        fs::write(root.join("two/SKILL.md"), "# Two\n\nBody\n").unwrap();

        let dirs = discover_skill_dirs(&root);

        assert_eq!(dirs.len(), 2);
    }

    #[test]
    fn loads_local_inventory_without_npx() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("skills");
        fs::create_dir_all(root.join("one")).unwrap();
        fs::write(
            root.join("one/SKILL.md"),
            r#"---
name: one
description: "One skill"
agents: ["one-agent"]
---

# One
"#,
        )
        .unwrap();

        let records = load_local_inventory(vec![SkillRoot {
            path: root,
            scope: SkillScope::Local,
        }]);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "one");
        assert_eq!(records[0].state.label(), "LocalOnly");
        assert_eq!(records[0].scope, SkillScope::Local);
        assert_eq!(records[0].agents_count(), 1);
    }
}
