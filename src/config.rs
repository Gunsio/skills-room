use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const CONFIG_PATH_ENV: &str = "SKILLROOM_CONFIG";

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub schema_version: u32,
    pub theme: ThemeName,
    pub language: Language,
    pub cache: CacheSettings,
    pub safety: SafetySettings,
    #[serde(default)]
    pub active_space: Option<String>,
    #[serde(default = "default_space_search_query")]
    pub space_search_query: String,
    #[serde(default)]
    pub spaces: Vec<SpaceSettings>,
    pub sources: Vec<SourceSettings>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            theme: ThemeName::TokyoNight,
            language: Language::EnUs,
            cache: CacheSettings::default(),
            safety: SafetySettings::default(),
            active_space: None,
            space_search_query: default_space_search_query(),
            spaces: Vec::new(),
            sources: vec![SourceSettings::bytedance()],
        }
    }
}

impl AppConfig {
    pub fn normalized(mut self) -> (Self, Vec<ConfigWarning>) {
        let mut warnings = Vec::new();

        if self.schema_version != CONFIG_SCHEMA_VERSION {
            warnings.push(ConfigWarning::SchemaMigrated {
                from: self.schema_version,
                to: CONFIG_SCHEMA_VERSION,
            });
            self.schema_version = CONFIG_SCHEMA_VERSION;
        }

        if !self.safety.delete_confirmation {
            warnings.push(ConfigWarning::SafetyLockRestored("delete_confirmation"));
            self.safety.delete_confirmation = true;
        }
        if !self.safety.home_delete_guard {
            warnings.push(ConfigWarning::SafetyLockRestored("home_delete_guard"));
            self.safety.home_delete_guard = true;
        }
        if self.sources.is_empty() {
            warnings.push(ConfigWarning::DefaultSourceRestored);
            self.sources.push(SourceSettings::bytedance());
        }
        if self
            .space_search_query
            .trim()
            .eq_ignore_ascii_case("qianchuan")
        {
            self.space_search_query = default_space_search_query();
        }
        if self.space_search_query.trim().is_empty() {
            self.space_search_query = default_space_search_query();
        }
        for space in &mut self.spaces {
            space.normalize();
        }
        if self.active_space.as_ref().is_none_or(|active| {
            !self
                .spaces
                .iter()
                .any(|space| space.enabled && &space.id == active)
        }) {
            self.active_space = None;
        }
        for source in &mut self.sources {
            if source.name == "skills.bytedance.net"
                || source.url.trim_end_matches('/') == "https://skills.bytedance.net"
            {
                warnings.push(ConfigWarning::SourceMigrated(source.name.clone()));
                *source = SourceSettings::bytedance();
            } else {
                source.normalize();
            }
        }

        (self, warnings)
    }
}

fn default_space_search_query() -> String {
    String::new()
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SpaceSettings {
    pub id: String,
    pub label: String,
    pub scope: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub package_count: usize,
    pub enabled: bool,
}

impl SpaceSettings {
    pub fn qianchuan_fe() -> Self {
        Self {
            id: "qianchuan-fe".to_string(),
            label: "qianchuan/fe".to_string(),
            scope: "skills.byted.org/qianchuan/fe".to_string(),
            url: "https://skills.bytedance.net/space/skills.byted.org/qianchuan/fe".to_string(),
            package_count: 15,
            enabled: true,
        }
    }

    pub fn discovered(
        scope: impl Into<String>,
        label: impl Into<String>,
        package_count: usize,
    ) -> Self {
        let scope = scope.into();
        Self {
            id: space_id_from_scope(&scope),
            label: label.into(),
            url: format!("https://skills.bytedance.net/space/{scope}"),
            scope,
            package_count,
            enabled: true,
        }
    }

    fn normalize(&mut self) {
        if self.id.trim().is_empty() {
            self.id = space_id_from_scope(&self.scope);
        }
        if self.label.trim().is_empty() {
            self.label = space_label_from_scope(&self.scope);
        }
        if self.scope.trim().is_empty() {
            self.scope = "skills.byted.org/default/public".to_string();
        }
        if self.url.trim().is_empty() {
            self.url = format!("https://skills.bytedance.net/space/{}", self.scope);
        }
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
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

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ThemeName {
    #[default]
    #[serde(rename = "tokyo-night")]
    TokyoNight,
    #[serde(rename = "catppuccin-mocha")]
    CatppuccinMocha,
    #[serde(rename = "gruvbox-dark")]
    GruvboxDark,
}

impl ThemeName {
    pub const ALL: [Self; 3] = [Self::TokyoNight, Self::CatppuccinMocha, Self::GruvboxDark];

    pub const fn key(self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyo-night",
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::GruvboxDark => "gruvbox-dark",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::TokyoNight => "Tokyo Night",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::GruvboxDark => "Gruvbox Dark",
        }
    }

    pub fn next(self) -> Self {
        next_in(&Self::ALL, self)
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[default]
    #[serde(rename = "en-US")]
    EnUs,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::ZhCn, Self::EnUs];

    pub const fn key(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ZhCn => "中文",
            Self::EnUs => "English",
        }
    }

    pub fn next(self) -> Self {
        next_in(&Self::ALL, self)
    }
}

fn next_in<T: Copy + Eq>(items: &[T], current: T) -> T {
    let index = items.iter().position(|item| *item == current).unwrap_or(0);
    items[(index + 1) % items.len()]
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheSettings {
    pub ttl_seconds: u64,
    pub last_status: String,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            ttl_seconds: 1_800,
            last_status: "ready".to_string(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SafetySettings {
    pub delete_confirmation: bool,
    pub home_delete_guard: bool,
}

impl Default for SafetySettings {
    fn default() -> Self {
        Self {
            delete_confirmation: true,
            home_delete_guard: true,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceSettings {
    pub name: String,
    #[serde(default)]
    pub kind: SourceKind,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portal_url: Option<String>,
    pub enabled: bool,
    pub last_status: String,
}

impl SourceSettings {
    pub fn bytedance() -> Self {
        Self {
            name: "bytedance-agentbuddy".to_string(),
            kind: SourceKind::AgentBuddy,
            url: "https://artifact-api.byted.org".to_string(),
            portal_url: Some("https://skills.bytedance.net/".to_string()),
            enabled: true,
            last_status: "not-tested".to_string(),
        }
    }

    pub fn custom(index: usize) -> Self {
        Self {
            name: format!("custom-{index}"),
            kind: SourceKind::Custom,
            url: format!("https://example.invalid/skills/{index}"),
            portal_url: None,
            enabled: false,
            last_status: "not-tested".to_string(),
        }
    }

    pub fn well_known(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: SourceKind::WellKnown,
            url: url.into(),
            portal_url: None,
            enabled: false,
            last_status: "not-tested".to_string(),
        }
    }

    fn normalize(&mut self) {
        if self.name.trim().is_empty() {
            self.name = "custom-source".to_string();
        }
        if self.url.trim().is_empty() {
            self.url = "https://example.invalid/skills".to_string();
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceKind {
    #[serde(rename = "agentbuddy")]
    AgentBuddy,
    #[serde(rename = "well-known")]
    WellKnown,
    #[default]
    #[serde(rename = "custom")]
    Custom,
}

impl SourceKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AgentBuddy => "agentbuddy",
            Self::WellKnown => "well-known",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: AppConfig,
    pub warnings: Vec<ConfigWarning>,
}

pub fn load_from_env() -> LoadedConfig {
    let path = resolve_config_path();
    load_or_default(path)
}

pub fn resolve_config_path() -> PathBuf {
    let explicit = env::var_os(CONFIG_PATH_ENV).map(PathBuf::from);
    let xdg_config_home = env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = env::var_os("HOME").map(PathBuf::from);
    resolve_config_path_from(explicit, xdg_config_home, home)
}

pub fn resolve_config_path_from(
    explicit: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = xdg_config_home {
        return path.join("skillroom/config.toml");
    }
    if let Some(path) = home {
        return path.join(".config/skillroom/config.toml");
    }
    PathBuf::from("skillroom/config.toml")
}

pub fn load_or_default(path: PathBuf) -> LoadedConfig {
    let mut warnings = Vec::new();
    let config = match fs::read_to_string(&path) {
        Ok(content) => match toml::from_str::<AppConfig>(&content) {
            Ok(config) => config,
            Err(source) => {
                warnings.push(ConfigWarning::ParseFailed(source.to_string()));
                AppConfig::default()
            }
        },
        Err(source) if source.kind() == io::ErrorKind::NotFound => AppConfig::default(),
        Err(source) => {
            warnings.push(ConfigWarning::ReadFailed(source.to_string()));
            AppConfig::default()
        }
    };

    let (config, normalized_warnings) = config.normalized();
    warnings.extend(normalized_warnings);

    LoadedConfig {
        path,
        config,
        warnings,
    }
}

pub fn save(path: impl AsRef<Path>, config: &AppConfig) -> Result<(), ConfigError> {
    let (config, _) = config.clone().normalized();
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let content = toml::to_string_pretty(&config).map_err(ConfigError::Serialize)?;
    fs::write(path.as_ref(), content).map_err(|source| ConfigError::Write {
        path: path.as_ref().to_path_buf(),
        source,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConfigWarning {
    ParseFailed(String),
    ReadFailed(String),
    SchemaMigrated { from: u32, to: u32 },
    SafetyLockRestored(&'static str),
    DefaultSourceRestored,
    SourceMigrated(String),
}

impl fmt::Display for ConfigWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseFailed(source) => write!(formatter, "config parse failed: {source}"),
            Self::ReadFailed(source) => write!(formatter, "config read failed: {source}"),
            Self::SchemaMigrated { from, to } => {
                write!(formatter, "config schema migrated from {from} to {to}")
            }
            Self::SafetyLockRestored(key) => write!(formatter, "safety lock restored: {key}"),
            Self::DefaultSourceRestored => write!(formatter, "default source restored"),
            Self::SourceMigrated(name) => {
                write!(formatter, "source {name} migrated to bytedance-agentbuddy")
            }
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    CreateDir { path: PathBuf, source: io::Error },
    Serialize(toml::ser::Error),
    Write { path: PathBuf, source: io::Error },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDir { path, source } => {
                write!(
                    formatter,
                    "failed to create config dir {}: {source}",
                    path.display()
                )
            }
            Self::Serialize(source) => write!(formatter, "failed to serialize config: {source}"),
            Self::Write { path, source } => {
                write!(
                    formatter,
                    "failed to write config {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn resolves_config_path_by_precedence() {
        assert_eq!(
            resolve_config_path_from(
                Some(PathBuf::from("/tmp/explicit.toml")),
                Some(PathBuf::from("/tmp/xdg")),
                Some(PathBuf::from("/tmp/home"))
            ),
            PathBuf::from("/tmp/explicit.toml")
        );
        assert_eq!(
            resolve_config_path_from(None, Some(PathBuf::from("/tmp/xdg")), None),
            PathBuf::from("/tmp/xdg/skillroom/config.toml")
        );
        assert_eq!(
            resolve_config_path_from(None, None, Some(PathBuf::from("/tmp/home"))),
            PathBuf::from("/tmp/home/.config/skillroom/config.toml")
        );
    }

    #[test]
    fn default_config_has_safe_values_and_default_source() {
        let config = AppConfig::default();

        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.theme, ThemeName::TokyoNight);
        assert_eq!(config.language, Language::EnUs);
        assert!(config.safety.delete_confirmation);
        assert!(config.safety.home_delete_guard);
        assert_eq!(config.sources[0].name, "bytedance-agentbuddy");
        assert_eq!(config.sources[0].kind, SourceKind::AgentBuddy);
        assert_eq!(config.sources[0].url, "https://artifact-api.byted.org");
        assert_eq!(
            config.sources[0].portal_url.as_deref(),
            Some("https://skills.bytedance.net/")
        );
        assert_eq!(config.active_space, None);
        assert_eq!(config.space_search_query, "");
        assert!(config.spaces.is_empty());
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let config = AppConfig {
            theme: ThemeName::GruvboxDark,
            language: Language::ZhCn,
            ..AppConfig::default()
        };

        save(&path, &config).unwrap();
        let loaded = load_or_default(path);

        assert_eq!(loaded.config.theme, ThemeName::GruvboxDark);
        assert_eq!(loaded.config.language, Language::ZhCn);
        assert!(loaded.warnings.is_empty());
    }

    #[test]
    fn normalization_keeps_space_selection_empty_until_discovery() {
        let config = AppConfig {
            active_space: None,
            spaces: Vec::new(),
            ..AppConfig::default()
        };

        let (normalized, warnings) = config.normalized();

        assert!(warnings.is_empty());
        assert_eq!(normalized.active_space, None);
        assert!(normalized.spaces.is_empty());
    }

    #[test]
    fn corrupt_config_falls_back_to_default_with_warning() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "not = [valid").unwrap();

        let loaded = load_or_default(path);

        assert_eq!(loaded.config, AppConfig::default());
        assert!(matches!(
            loaded.warnings.as_slice(),
            [ConfigWarning::ParseFailed(_)]
        ));
    }

    #[test]
    fn legacy_skills_portal_source_migrates_to_agentbuddy_source() {
        let config = AppConfig {
            sources: vec![SourceSettings {
                name: "skills.bytedance.net".to_string(),
                kind: SourceKind::Custom,
                url: "https://skills.bytedance.net/".to_string(),
                portal_url: None,
                enabled: true,
                last_status: "ready".to_string(),
            }],
            ..AppConfig::default()
        };

        let (config, warnings) = config.normalized();

        assert_eq!(config.sources[0].name, "bytedance-agentbuddy");
        assert_eq!(config.sources[0].kind, SourceKind::AgentBuddy);
        assert!(
            warnings
                .iter()
                .any(|warning| matches!(warning, ConfigWarning::SourceMigrated(_)))
        );
    }

    #[test]
    fn normalization_restores_safety_locks_and_migrates_schema() {
        let config = AppConfig {
            schema_version: 0,
            safety: SafetySettings {
                delete_confirmation: false,
                home_delete_guard: false,
            },
            sources: Vec::new(),
            ..AppConfig::default()
        };

        let (normalized, warnings) = config.normalized();

        assert_eq!(normalized.schema_version, CONFIG_SCHEMA_VERSION);
        assert!(normalized.safety.delete_confirmation);
        assert!(normalized.safety.home_delete_guard);
        assert_eq!(normalized.sources.len(), 1);
        assert_eq!(warnings.len(), 4);
    }
}
