use std::{fs, path::Path};

use skillroom::{
    cache::{CacheLookup, CacheOptions, read_cache, write_cache},
    inventory::{LocalSkillData, merge_inventory},
    loaders::npx::parse_npx_skills_json,
    parser::parse_skill_markdown,
    scan::scan_skill_dir,
    skill::SkillState,
};

#[test]
fn fixture_local_inventory_covers_scan_parse_merge_and_cache() {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/local-skills");
    let good_path = fixture_root.join("good");
    let broken_path = fixture_root.join("broken");

    let listed = serde_json::json!([
        {
            "name": "good",
            "path": good_path,
            "scope": "local",
            "agents": ["listed-agent"]
        }
    ])
    .to_string();
    let listed_records = parse_npx_skills_json(listed).unwrap();

    let local = vec![local_skill_data(&good_path), local_skill_data(&broken_path)];
    let records = merge_inventory(listed_records, local);

    let good = records.iter().find(|record| record.name == "good").unwrap();
    assert_eq!(good.state, SkillState::Installed);
    assert_eq!(good.stats.files, 4);
    assert_eq!(good.agents_count(), 1);
    assert!(good.tags.iter().any(|tag| tag == "references/guide.md"));

    let broken = records
        .iter()
        .find(|record| record.name == "broken")
        .unwrap();
    assert_eq!(broken.state, SkillState::Error);
    assert!(broken.error.as_deref().unwrap().contains("empty"));

    let temp = tempfile::tempdir().unwrap();
    let cache_path = temp.path().join("skillroom/cache.json");
    write_cache(&cache_path, &records, 100).unwrap();

    match read_cache(&cache_path, 110, CacheOptions::new(60)) {
        CacheLookup::Hit(cached) => assert_eq!(cached.len(), 2),
        CacheLookup::Miss(reason) => panic!("expected cache hit, got {reason:?}"),
    }
}

fn local_skill_data(path: &Path) -> LocalSkillData {
    let skill_md = path.join("SKILL.md");
    let content = fs::read_to_string(skill_md).unwrap_or_default();
    LocalSkillData {
        path: path.to_path_buf(),
        scan: scan_skill_dir(path),
        parsed: parse_skill_markdown(&content),
    }
}
