use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::skill::SkillRecord;

pub const CACHE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CacheLookup {
    Hit(Vec<SkillRecord>),
    Miss(CacheMissReason),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CacheMissReason {
    Missing,
    ForcedRefresh,
    Expired { age_seconds: u64, ttl_seconds: u64 },
    SchemaMismatch { expected: u32, found: u32 },
    Corrupt(String),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CacheOptions {
    pub ttl_seconds: u64,
    pub force_refresh: bool,
}

impl CacheOptions {
    pub const fn new(ttl_seconds: u64) -> Self {
        Self {
            ttl_seconds,
            force_refresh: false,
        }
    }

    pub const fn force_refresh(ttl_seconds: u64) -> Self {
        Self {
            ttl_seconds,
            force_refresh: true,
        }
    }
}

pub fn read_cache(path: impl AsRef<Path>, now_seconds: u64, options: CacheOptions) -> CacheLookup {
    if options.force_refresh {
        return CacheLookup::Miss(CacheMissReason::ForcedRefresh);
    }

    let content = match fs::read_to_string(path.as_ref()) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return CacheLookup::Miss(CacheMissReason::Missing);
        }
        Err(error) => return CacheLookup::Miss(CacheMissReason::Corrupt(error.to_string())),
    };

    let cache: CacheFile = match serde_json::from_str(&content) {
        Ok(cache) => cache,
        Err(error) => return CacheLookup::Miss(CacheMissReason::Corrupt(error.to_string())),
    };

    if cache.schema_version != CACHE_SCHEMA_VERSION {
        return CacheLookup::Miss(CacheMissReason::SchemaMismatch {
            expected: CACHE_SCHEMA_VERSION,
            found: cache.schema_version,
        });
    }

    let age_seconds = now_seconds.saturating_sub(cache.generated_at_unix_seconds);
    if age_seconds > options.ttl_seconds {
        return CacheLookup::Miss(CacheMissReason::Expired {
            age_seconds,
            ttl_seconds: options.ttl_seconds,
        });
    }

    CacheLookup::Hit(cache.records)
}

pub fn write_cache(
    path: impl AsRef<Path>,
    records: &[SkillRecord],
    now_seconds: u64,
) -> Result<(), CacheError> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|source| CacheError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let cache = CacheFile {
        schema_version: CACHE_SCHEMA_VERSION,
        generated_at_unix_seconds: now_seconds,
        records: records.to_vec(),
    };
    let content = serde_json::to_string_pretty(&cache).map_err(CacheError::Serialize)?;
    fs::write(path.as_ref(), content).map_err(|source| CacheError::Write {
        path: path.as_ref().to_path_buf(),
        source,
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    schema_version: u32,
    generated_at_unix_seconds: u64,
    records: Vec<SkillRecord>,
}

#[derive(Debug)]
pub enum CacheError {
    CreateDir { path: PathBuf, source: io::Error },
    Serialize(serde_json::Error),
    Write { path: PathBuf, source: io::Error },
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(
                    formatter,
                    "failed to create cache dir {}: {source}",
                    path.display()
                )
            }
            Self::Serialize(source) => write!(formatter, "failed to serialize cache: {source}"),
            Self::Write { path, source } => {
                write!(
                    formatter,
                    "failed to write cache {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CacheError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::skill::fixture_skills;

    #[test]
    fn missing_cache_is_miss() {
        let temp = tempdir().unwrap();
        let lookup = read_cache(temp.path().join("missing.json"), 100, CacheOptions::new(60));

        assert_eq!(lookup, CacheLookup::Miss(CacheMissReason::Missing));
    }

    #[test]
    fn write_and_read_cache_hit() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("cache").join("skills.json");
        let records = fixture_skills();

        write_cache(&path, &records, 100).unwrap();
        let lookup = read_cache(&path, 120, CacheOptions::new(60));

        match lookup {
            CacheLookup::Hit(records) => assert_eq!(records.len(), 5),
            CacheLookup::Miss(reason) => panic!("expected cache hit, got {reason:?}"),
        }
    }

    #[test]
    fn expired_cache_is_miss() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("skills.json");
        write_cache(&path, &fixture_skills(), 100).unwrap();

        let lookup = read_cache(&path, 200, CacheOptions::new(60));

        assert_eq!(
            lookup,
            CacheLookup::Miss(CacheMissReason::Expired {
                age_seconds: 100,
                ttl_seconds: 60,
            })
        );
    }

    #[test]
    fn force_refresh_bypasses_cache() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("skills.json");
        write_cache(&path, &fixture_skills(), 100).unwrap();

        let lookup = read_cache(&path, 100, CacheOptions::force_refresh(60));

        assert_eq!(lookup, CacheLookup::Miss(CacheMissReason::ForcedRefresh));
    }

    #[test]
    fn schema_mismatch_is_miss() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("skills.json");
        fs::write(
            &path,
            r#"{"schema_version":0,"generated_at_unix_seconds":100,"records":[]}"#,
        )
        .unwrap();

        let lookup = read_cache(&path, 100, CacheOptions::new(60));

        assert_eq!(
            lookup,
            CacheLookup::Miss(CacheMissReason::SchemaMismatch {
                expected: CACHE_SCHEMA_VERSION,
                found: 0,
            })
        );
    }
}
