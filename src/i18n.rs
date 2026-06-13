use std::fmt;

use crate::config::Language;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum I18nKey {
    SearchPlaceholder,
    Focus,
    Sort,
    PanelCommand,
    PanelSkills,
    PanelDetails,
    PanelStats,
    PanelOutput,
    PanelSettings,
    PanelHelp,
    ColumnName,
    ColumnSource,
    ColumnScope,
    ColumnState,
    ColumnRisk,
    ColumnUpdate,
    DetailName,
    DetailScope,
    DetailState,
    DetailSource,
    DetailVersion,
    DetailPath,
    DetailAgents,
    DetailRisk,
    DetailFiles,
    DetailScripts,
    DetailActions,
    DetailError,
    DetailDescription,
    DetailTags,
    NoSkillSelected,
    StatFilters,
    StatSettings,
    StatVisible,
    StatTotal,
    StatLocal,
    StatUpdates,
    StatHighRisk,
    StatusActive,
    StatusFocused,
    StatusReady,
    StatusPlaceholder,
    KeyQuit,
    KeySearch,
    KeyHelp,
    KeySettings,
    KeyFocus,
    KeySelect,
    SettingsEscCancels,
    SettingsConfig,
    SettingsTheme,
    SettingsLanguage,
    SettingsCacheTtl,
    SettingsCache,
    SettingsSafety,
    SettingsSources,
    SettingsSave,
    SettingsSourcePrefix,
    SettingsTestPrefix,
    ValueConfiguredSources,
    ValueEnabled,
    ValueDisabled,
    ValueSavePersist,
    ValueSafetyLocked,
    ValueSafetyRestored,
    HintTheme,
    HintLanguage,
    HintCacheTtl,
    HintCache,
    HintSafety,
    HintSources,
    HintSourceToggle,
    HintSourceTest,
    HintSave,
    HelpNavigation,
    HelpMove,
    HelpPage,
    HelpTopBottom,
    HelpFocus,
    HelpSort,
    HelpSettings,
    HelpClose,
    HelpQuit,
}

impl I18nKey {
    const ALL: [Self; 83] = [
        Self::SearchPlaceholder,
        Self::Focus,
        Self::Sort,
        Self::PanelCommand,
        Self::PanelSkills,
        Self::PanelDetails,
        Self::PanelStats,
        Self::PanelOutput,
        Self::PanelSettings,
        Self::PanelHelp,
        Self::ColumnName,
        Self::ColumnSource,
        Self::ColumnScope,
        Self::ColumnState,
        Self::ColumnRisk,
        Self::ColumnUpdate,
        Self::DetailName,
        Self::DetailScope,
        Self::DetailState,
        Self::DetailSource,
        Self::DetailVersion,
        Self::DetailPath,
        Self::DetailAgents,
        Self::DetailRisk,
        Self::DetailFiles,
        Self::DetailScripts,
        Self::DetailActions,
        Self::DetailError,
        Self::DetailDescription,
        Self::DetailTags,
        Self::NoSkillSelected,
        Self::StatFilters,
        Self::StatSettings,
        Self::StatVisible,
        Self::StatTotal,
        Self::StatLocal,
        Self::StatUpdates,
        Self::StatHighRisk,
        Self::StatusActive,
        Self::StatusFocused,
        Self::StatusReady,
        Self::StatusPlaceholder,
        Self::KeyQuit,
        Self::KeySearch,
        Self::KeyHelp,
        Self::KeySettings,
        Self::KeyFocus,
        Self::KeySelect,
        Self::SettingsEscCancels,
        Self::SettingsConfig,
        Self::SettingsTheme,
        Self::SettingsLanguage,
        Self::SettingsCacheTtl,
        Self::SettingsCache,
        Self::SettingsSafety,
        Self::SettingsSources,
        Self::SettingsSave,
        Self::SettingsSourcePrefix,
        Self::SettingsTestPrefix,
        Self::ValueConfiguredSources,
        Self::ValueEnabled,
        Self::ValueDisabled,
        Self::ValueSavePersist,
        Self::ValueSafetyLocked,
        Self::ValueSafetyRestored,
        Self::HintTheme,
        Self::HintLanguage,
        Self::HintCacheTtl,
        Self::HintCache,
        Self::HintSafety,
        Self::HintSources,
        Self::HintSourceToggle,
        Self::HintSourceTest,
        Self::HintSave,
        Self::HelpNavigation,
        Self::HelpMove,
        Self::HelpPage,
        Self::HelpTopBottom,
        Self::HelpFocus,
        Self::HelpSort,
        Self::HelpSettings,
        Self::HelpClose,
        Self::HelpQuit,
    ];
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct I18nCatalog {
    language: Language,
    errors: Vec<I18nError>,
}

impl I18nCatalog {
    pub fn new(language: Language) -> Self {
        let errors = I18nKey::ALL
            .iter()
            .filter_map(|key| lookup(language, *key).error)
            .collect();

        Self { language, errors }
    }

    pub fn text(&self, key: I18nKey) -> &'static str {
        lookup(self.language, key).text
    }

    pub fn errors(&self) -> &[I18nError] {
        &self.errors
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct I18nLookup {
    pub text: &'static str,
    pub error: Option<I18nError>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct I18nError {
    pub language: Language,
    pub key: I18nKey,
}

impl fmt::Display for I18nError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "missing i18n key {:?} for {}",
            self.key,
            self.language.key()
        )
    }
}

pub fn lookup(language: Language, key: I18nKey) -> I18nLookup {
    lookup_from(language, zh_cn, en_us, key)
}

pub fn lookup_from(
    language: Language,
    localized: fn(I18nKey) -> Option<&'static str>,
    fallback: fn(I18nKey) -> &'static str,
    key: I18nKey,
) -> I18nLookup {
    match language {
        Language::EnUs => I18nLookup {
            text: fallback(key),
            error: None,
        },
        Language::ZhCn => match localized(key) {
            Some(text) => I18nLookup { text, error: None },
            None => I18nLookup {
                text: fallback(key),
                error: Some(I18nError { language, key }),
            },
        },
    }
}

fn en_us(key: I18nKey) -> &'static str {
    match key {
        I18nKey::SearchPlaceholder => "[/] Search skills...",
        I18nKey::Focus => " focus=",
        I18nKey::Sort => " sort=",
        I18nKey::PanelCommand => "Command",
        I18nKey::PanelSkills => "Skills",
        I18nKey::PanelDetails => "Details",
        I18nKey::PanelStats => "Stats",
        I18nKey::PanelOutput => "Output",
        I18nKey::PanelSettings => "Settings",
        I18nKey::PanelHelp => "Help",
        I18nKey::ColumnName => "Name",
        I18nKey::ColumnSource => "Source",
        I18nKey::ColumnScope => "Scope",
        I18nKey::ColumnState => "State",
        I18nKey::ColumnRisk => "Risk",
        I18nKey::ColumnUpdate => "Update",
        I18nKey::DetailName => "Name: ",
        I18nKey::DetailScope => "Scope: ",
        I18nKey::DetailState => "State: ",
        I18nKey::DetailSource => "Source: ",
        I18nKey::DetailVersion => "Version: ",
        I18nKey::DetailPath => "Path: ",
        I18nKey::DetailAgents => "Agents: ",
        I18nKey::DetailRisk => "Risk: ",
        I18nKey::DetailFiles => "Files: ",
        I18nKey::DetailScripts => "Scripts: ",
        I18nKey::DetailActions => "Actions: ",
        I18nKey::DetailError => "Error: ",
        I18nKey::DetailDescription => "Description: ",
        I18nKey::DetailTags => "Tags: ",
        I18nKey::NoSkillSelected => "No skill selected",
        I18nKey::StatFilters => "Filters ",
        I18nKey::StatSettings => "Settings ",
        I18nKey::StatVisible => "Visible ",
        I18nKey::StatTotal => "Total ",
        I18nKey::StatLocal => "Local ",
        I18nKey::StatUpdates => "Updates ",
        I18nKey::StatHighRisk => "High risk ",
        I18nKey::StatusActive => "active",
        I18nKey::StatusFocused => "focused",
        I18nKey::StatusReady => "ready",
        I18nKey::StatusPlaceholder => "placeholder",
        I18nKey::KeyQuit => "quit ",
        I18nKey::KeySearch => "search ",
        I18nKey::KeyHelp => "help ",
        I18nKey::KeySettings => "settings ",
        I18nKey::KeyFocus => "focus ",
        I18nKey::KeySelect => "select",
        I18nKey::SettingsEscCancels => "Esc cancels",
        I18nKey::SettingsConfig => "Config: ",
        I18nKey::SettingsTheme => "Theme",
        I18nKey::SettingsLanguage => "Language",
        I18nKey::SettingsCacheTtl => "Cache TTL",
        I18nKey::SettingsCache => "Cache",
        I18nKey::SettingsSafety => "Safety",
        I18nKey::SettingsSources => "Sources",
        I18nKey::SettingsSave => "Save",
        I18nKey::SettingsSourcePrefix => "Source ",
        I18nKey::SettingsTestPrefix => "Test ",
        I18nKey::ValueConfiguredSources => " configured",
        I18nKey::ValueEnabled => "enabled",
        I18nKey::ValueDisabled => "disabled",
        I18nKey::ValueSavePersist => "persist config.toml",
        I18nKey::ValueSafetyLocked => "locked",
        I18nKey::ValueSafetyRestored => "restored on save",
        I18nKey::HintTheme => "Enter cycles theme",
        I18nKey::HintLanguage => "Enter cycles language",
        I18nKey::HintCacheTtl => "Enter cycles TTL",
        I18nKey::HintCache => "Enter clears cache state",
        I18nKey::HintSafety => "Cannot disable guard rails",
        I18nKey::HintSources => "Enter adds disabled source",
        I18nKey::HintSourceToggle => "Enter toggles source",
        I18nKey::HintSourceTest => "Enter dry-runs source",
        I18nKey::HintSave => "Enter writes config",
        I18nKey::HelpNavigation => "Navigation",
        I18nKey::HelpMove => "j/k or arrows: move selection",
        I18nKey::HelpPage => "PageUp/PageDown: page selection",
        I18nKey::HelpTopBottom => "g/G: jump to top/bottom",
        I18nKey::HelpFocus => "Tab / Shift+Tab: cycle focus",
        I18nKey::HelpSort => "s/S: cycle sort column / reverse sort",
        I18nKey::HelpSettings => ",: open settings",
        I18nKey::HelpClose => "?: close help",
        I18nKey::HelpQuit => "q: quit",
    }
}

fn zh_cn(key: I18nKey) -> Option<&'static str> {
    Some(match key {
        I18nKey::SearchPlaceholder => "[/] 搜索 skills...",
        I18nKey::Focus => " 焦点=",
        I18nKey::Sort => " 排序=",
        I18nKey::PanelCommand => "命令",
        I18nKey::PanelSkills => "Skills",
        I18nKey::PanelDetails => "详情",
        I18nKey::PanelStats => "统计",
        I18nKey::PanelOutput => "输出",
        I18nKey::PanelSettings => "设置",
        I18nKey::PanelHelp => "帮助",
        I18nKey::ColumnName => "名称",
        I18nKey::ColumnSource => "来源",
        I18nKey::ColumnScope => "范围",
        I18nKey::ColumnState => "状态",
        I18nKey::ColumnRisk => "风险",
        I18nKey::ColumnUpdate => "更新",
        I18nKey::DetailName => "名称: ",
        I18nKey::DetailScope => "范围: ",
        I18nKey::DetailState => "状态: ",
        I18nKey::DetailSource => "来源: ",
        I18nKey::DetailVersion => "版本: ",
        I18nKey::DetailPath => "路径: ",
        I18nKey::DetailAgents => "Agents: ",
        I18nKey::DetailRisk => "风险: ",
        I18nKey::DetailFiles => "文件: ",
        I18nKey::DetailScripts => "脚本: ",
        I18nKey::DetailActions => "操作: ",
        I18nKey::DetailError => "错误: ",
        I18nKey::DetailDescription => "描述: ",
        I18nKey::DetailTags => "标签: ",
        I18nKey::NoSkillSelected => "未选择 skill",
        I18nKey::StatFilters => "过滤 ",
        I18nKey::StatSettings => "设置 ",
        I18nKey::StatVisible => "可见 ",
        I18nKey::StatTotal => "总数 ",
        I18nKey::StatLocal => "本地 ",
        I18nKey::StatUpdates => "更新 ",
        I18nKey::StatHighRisk => "高风险 ",
        I18nKey::StatusActive => "启用",
        I18nKey::StatusFocused => "聚焦",
        I18nKey::StatusReady => "就绪",
        I18nKey::StatusPlaceholder => "占位",
        I18nKey::KeyQuit => "退出 ",
        I18nKey::KeySearch => "搜索 ",
        I18nKey::KeyHelp => "帮助 ",
        I18nKey::KeySettings => "设置 ",
        I18nKey::KeyFocus => "焦点 ",
        I18nKey::KeySelect => "选择",
        I18nKey::SettingsEscCancels => "Esc 取消",
        I18nKey::SettingsConfig => "配置: ",
        I18nKey::SettingsTheme => "主题",
        I18nKey::SettingsLanguage => "语言",
        I18nKey::SettingsCacheTtl => "缓存 TTL",
        I18nKey::SettingsCache => "缓存",
        I18nKey::SettingsSafety => "安全",
        I18nKey::SettingsSources => "来源",
        I18nKey::SettingsSave => "保存",
        I18nKey::SettingsSourcePrefix => "来源 ",
        I18nKey::SettingsTestPrefix => "测试 ",
        I18nKey::ValueConfiguredSources => " 个已配置",
        I18nKey::ValueEnabled => "启用",
        I18nKey::ValueDisabled => "禁用",
        I18nKey::ValueSavePersist => "持久化 config.toml",
        I18nKey::ValueSafetyLocked => "已锁定",
        I18nKey::ValueSafetyRestored => "保存时恢复",
        I18nKey::HintTheme => "Enter 切换主题",
        I18nKey::HintLanguage => "Enter 切换语言",
        I18nKey::HintCacheTtl => "Enter 切换 TTL",
        I18nKey::HintCache => "Enter 清理缓存状态",
        I18nKey::HintSafety => "不能关闭安全底线",
        I18nKey::HintSources => "Enter 新增禁用来源",
        I18nKey::HintSourceToggle => "Enter 启用/禁用来源",
        I18nKey::HintSourceTest => "Enter 本地测试来源",
        I18nKey::HintSave => "Enter 写入配置",
        I18nKey::HelpNavigation => "导航",
        I18nKey::HelpMove => "j/k 或方向键: 移动选择",
        I18nKey::HelpPage => "PageUp/PageDown: 翻页",
        I18nKey::HelpTopBottom => "g/G: 跳到顶部/底部",
        I18nKey::HelpFocus => "Tab / Shift+Tab: 切换焦点",
        I18nKey::HelpSort => "s/S: 切换排序列 / 反向排序",
        I18nKey::HelpSettings => ",: 打开设置",
        I18nKey::HelpClose => "?: 关闭帮助",
        I18nKey::HelpQuit => "q: 退出",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_cn_and_en_us_catalogs_are_complete() {
        assert!(I18nCatalog::new(Language::EnUs).errors().is_empty());
        assert!(I18nCatalog::new(Language::ZhCn).errors().is_empty());
    }

    #[test]
    fn missing_localized_key_falls_back_to_en_us_and_records_error() {
        let lookup = lookup_from(Language::ZhCn, |_| None, en_us, I18nKey::KeyQuit);

        assert_eq!(lookup.text, "quit ");
        assert_eq!(
            lookup.error,
            Some(I18nError {
                language: Language::ZhCn,
                key: I18nKey::KeyQuit
            })
        );
    }
}
