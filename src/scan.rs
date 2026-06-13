use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::skill::SkillStats;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DirectoryScan {
    pub root: PathBuf,
    pub stats: SkillStats,
    pub scripts: Vec<String>,
    pub references: Vec<String>,
    pub assets: Vec<String>,
}

pub fn scan_skill_dir(root: impl AsRef<Path>) -> Result<DirectoryScan, ScanError> {
    let root = root.as_ref().to_path_buf();
    let mut scan = DirectoryScan {
        root: root.clone(),
        stats: SkillStats::default(),
        scripts: Vec::new(),
        references: Vec::new(),
        assets: Vec::new(),
    };

    scan_path(&root, &root, &mut scan)?;
    scan.scripts.sort();
    scan.references.sort();
    scan.assets.sort();

    Ok(scan)
}

fn scan_path(root: &Path, path: &Path, scan: &mut DirectoryScan) -> Result<(), ScanError> {
    let metadata = fs::metadata(path).map_err(|source| ScanError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    update_mtime(&metadata, &mut scan.stats);

    if metadata.is_dir() {
        if path != root {
            scan.stats.directories += 1;
        }

        let entries = fs::read_dir(path).map_err(|source| ScanError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| ScanError::Read {
                path: path.to_path_buf(),
                source,
            })?;
            scan_path(root, &entry.path(), scan)?;
        }

        return Ok(());
    }

    if metadata.is_file() {
        scan.stats.files += 1;
        let relative = relative_string(root, path);
        classify_file(path, &relative, scan);
        if let Ok(content) = fs::read_to_string(path) {
            scan.stats.line_count += content.lines().count();
        }
    }

    Ok(())
}

fn classify_file(path: &Path, relative: &str, scan: &mut DirectoryScan) {
    if path
        .components()
        .any(|component| component.as_os_str() == "scripts")
    {
        scan.scripts.push(relative.to_string());
    }

    if path
        .components()
        .any(|component| component.as_os_str() == "references")
    {
        scan.stats.references += 1;
        scan.references.push(relative.to_string());
    }

    if path
        .components()
        .any(|component| component.as_os_str() == "assets")
    {
        scan.stats.assets += 1;
        scan.assets.push(relative.to_string());
    }
}

fn update_mtime(metadata: &fs::Metadata, stats: &mut SkillStats) {
    let Ok(modified) = metadata.modified() else {
        return;
    };
    let Ok(seconds) = modified.duration_since(UNIX_EPOCH) else {
        return;
    };

    let seconds = seconds.as_secs();
    stats.modified_unix_seconds = Some(
        stats
            .modified_unix_seconds
            .map_or(seconds, |current| current.max(seconds)),
    );
}

fn relative_string(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug)]
pub enum ScanError {
    Read { path: PathBuf, source: io::Error },
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ScanError {}

#[cfg(test)]
mod tests {
    use std::{fs, thread, time::Duration};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn scans_skill_directory_structure() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("references")).unwrap();
        fs::create_dir(root.join("scripts")).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("SKILL.md"), "# Skill\n\nBody\n").unwrap();
        fs::write(root.join("references").join("guide.md"), "one\ntwo\n").unwrap();
        fs::write(root.join("scripts").join("run.sh"), "#!/usr/bin/env bash\n").unwrap();
        fs::write(root.join("assets").join("icon.txt"), "asset\n").unwrap();

        let scan = scan_skill_dir(root).unwrap();

        assert_eq!(scan.stats.files, 4);
        assert_eq!(scan.stats.directories, 3);
        assert_eq!(scan.stats.references, 1);
        assert_eq!(scan.stats.assets, 1);
        assert_eq!(scan.stats.line_count, 7);
        assert_eq!(scan.scripts, vec!["scripts/run.sh"]);
        assert_eq!(scan.references, vec!["references/guide.md"]);
        assert_eq!(scan.assets, vec!["assets/icon.txt"]);
        assert!(scan.stats.modified_unix_seconds.is_some());
    }

    #[test]
    fn mtime_uses_newest_entry() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("old.txt"), "old\n").unwrap();
        thread::sleep(Duration::from_millis(10));
        fs::write(root.join("new.txt"), "new\n").unwrap();

        let scan = scan_skill_dir(root).unwrap();

        assert!(scan.stats.modified_unix_seconds.is_some());
        assert_eq!(scan.stats.files, 2);
    }
}
