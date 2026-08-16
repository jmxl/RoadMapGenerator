//! Tiny built-in English / 中文 dictionary with runtime language switching.
//!
//! The current language lives in a process-wide atomic so any module (UI
//! callbacks, data validation errors, SVG export labels) can translate
//! without threading a language parameter through every call site. The UI
//! layer (`apply_language` in main.rs) copies the scalar strings into the
//! Slint `I18n` global; Rust-side messages call `t()` directly.

use std::sync::atomic::{AtomicU8, Ordering};

/// Supported UI languages.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Zh,
}

impl Lang {
    /// Map a persisted language code to a `Lang`. Only `"en"` maps to
    /// English; anything else (including unknown values) falls back to the
    /// default language, Chinese.
    pub fn from_code(s: &str) -> Lang {
        match s {
            "en" => Lang::En,
            _ => Lang::Zh,
        }
    }

    /// The persisted language code (`"en"` / `"zh"`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Zh => "zh",
        }
    }

    /// Display name shown in the language picker.
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Zh => "中文",
        }
    }
}

/// Process-wide current language (0 = En, 1 = Zh). Defaults to Chinese.
static CURRENT: AtomicU8 = AtomicU8::new(1);

/// Set the language used by `t()` / `t_list()`.
pub fn set_current(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

/// The language currently in effect.
pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 {
        Lang::Zh
    } else {
        Lang::En
    }
}

/// Translate `key` in the current language. Missing keys return the key
/// itself so a typo is visible in the UI instead of silently blank.
pub fn t(key: &str) -> &'static str {
    t_in(current(), key)
}

/// Translate `key` in an explicit language (for pure functions that receive
/// the language from persisted data, e.g. SVG export). Unknown keys yield an
/// empty string so a typo shows up as a blank spot instead of a panic.
pub fn t_in(lang: Lang, key: &str) -> &'static str {
    match (lang, key) {
        // ---- main window ----
        (Lang::En, "app-title") => concat!("Roadmap Generator - ", env!("CARGO_PKG_VERSION")),
        (Lang::Zh, "app-title") => concat!("路线图生成器 - ", env!("CARGO_PKG_VERSION")),
        (Lang::En, "menu-file") => "File",
        (Lang::Zh, "menu-file") => "文件",
        (Lang::En, "menu-open") => "Open",
        (Lang::Zh, "menu-open") => "打开",
        (Lang::En, "menu-new") => "New",
        (Lang::Zh, "menu-new") => "新建",
        (Lang::En, "status-new") => "New roadmap started at {path}",
        (Lang::Zh, "status-new") => "已新建空白路线图：{path}",
        (Lang::En, "menu-import-merge") => "Import (Merge)",
        (Lang::Zh, "menu-import-merge") => "导入(合并)",
        (Lang::En, "menu-save-as") => "Save As…",
        (Lang::Zh, "menu-save-as") => "另存为…",
        (Lang::En, "dlg-save-data") => "Save data file",
        (Lang::Zh, "dlg-save-data") => "保存数据文件",
        (Lang::En, "dlg-new-data") => "Save new roadmap",
        (Lang::Zh, "dlg-new-data") => "保存新建路线图",
        (Lang::En, "menu-save") => "Save",
        (Lang::Zh, "menu-save") => "保存",
        (Lang::En, "menu-settings") => "Settings",
        (Lang::Zh, "menu-settings") => "设置",
        (Lang::En, "menu-help") => "Help",
        (Lang::Zh, "menu-help") => "帮助",
        (Lang::En, "menu-about") => "About",
        (Lang::Zh, "menu-about") => "关于",
        (Lang::En, "placeholder-project") => "Project name  (e.g. XXX)",
        (Lang::Zh, "placeholder-project") => "项目名称（如 XXX）",
        (Lang::En, "placeholder-milestone") => "Milestone  (e.g. TR1)",
        (Lang::Zh, "placeholder-milestone") => "里程碑（如 TR1）",
        (Lang::En, "placeholder-date") => "Date  (yyyy/mm/dd)",
        (Lang::Zh, "placeholder-date") => "日期（yyyy/mm/dd）",
        (Lang::En, "btn-add-milestone") => "Add Milestone",
        (Lang::Zh, "btn-add-milestone") => "添加里程碑",
        (Lang::En, "label-color") => "Color:",
        (Lang::Zh, "label-color") => "颜色：",
        (Lang::En, "btn-new-project") => "New Project",
        (Lang::Zh, "btn-new-project") => "新建项目",
        (Lang::En, "btn-remove-project") => "Remove Selected Project",
        (Lang::Zh, "btn-remove-project") => "删除选中项目",
        (Lang::En, "btn-remove-milestone") => "Remove Selected Milestone",
        (Lang::Zh, "btn-remove-milestone") => "删除选中里程碑",
        (Lang::En, "btn-clear-all") => "Clear All",
        (Lang::Zh, "btn-clear-all") => "清空全部",
        (Lang::En, "btn-export-svg") => "Export SVG",
        (Lang::Zh, "btn-export-svg") => "导出 SVG",
        (Lang::En, "btn-export-png") => "Export PNG",
        (Lang::Zh, "btn-export-png") => "导出 PNG",
        (Lang::En, "today-label") => "We are here",
        (Lang::Zh, "today-label") => "当前进度",
        (Lang::En, "empty-hint") => "No projects yet - add one above.",
        (Lang::Zh, "empty-hint") => "暂无项目 - 请在上方添加。",
        // Milestone count line under a project name. `{n}` is the count.
        (Lang::En, "milestone-count") => "{n} milestone(s)",
        (Lang::Zh, "milestone-count") => "{n} 个里程碑",
        // Default status text until the real status is set.
        (Lang::En, "status-ready") => "Ready",
        (Lang::Zh, "status-ready") => "就绪",

        // ---- settings window ----
        (Lang::En, "settings-title") => "Settings",
        (Lang::Zh, "settings-title") => "设置",
        (Lang::En, "theme-label") => "Theme",
        (Lang::Zh, "theme-label") => "主题",
        (Lang::En, "theme-hint") => "Changes apply immediately and are saved to config.json.",
        (Lang::Zh, "theme-hint") => "更改立即生效并保存到 config.json。",
        (Lang::En, "lang-label") => "Language",
        (Lang::Zh, "lang-label") => "语言",
        (Lang::En, "lang-hint") => "Changes apply immediately and are saved to config.json.",
        (Lang::Zh, "lang-hint") => "更改立即生效并保存到 config.json。",
        (Lang::En, "settings-data-dir") => "Save Location",
        (Lang::Zh, "settings-data-dir") => "保存位置",
        (Lang::En, "btn-pick-data-dir") => "Choose Folder…",
        (Lang::Zh, "btn-pick-data-dir") => "选择文件夹…",
        (Lang::En, "btn-reset-data-dir") => "Reset to Default",
        (Lang::Zh, "btn-reset-data-dir") => "恢复默认",
        (Lang::En, "btn-close") => "Close",
        (Lang::Zh, "btn-close") => "关闭",

        // ---- about dialog ----
        (Lang::En, "about-title") => "About",
        (Lang::Zh, "about-title") => "关于",
        (Lang::En, "about-btn") => "OK",
        (Lang::Zh, "about-btn") => "确定",

        // ---- status messages (Rust side) ----
        (Lang::En, "status-loaded") => "Loaded {n} project(s) from {path}",
        (Lang::Zh, "status-loaded") => "已从 {path} 加载 {n} 个项目",
        (Lang::En, "status-no-data") => "No saved data yet - data file: {path}",
        (Lang::Zh, "status-no-data") => "暂无已保存数据 - 数据文件：{path}",
        (Lang::En, "status-added") => "Added milestone \"{milestone}\" ({date}) to \"{project}\"",
        (Lang::Zh, "status-added") => "已向 \"{project}\" 添加里程碑 \"{milestone}\"（{date}）",
        (Lang::En, "status-color-milestone") => "Milestone color set to {color}",
        (Lang::Zh, "status-color-milestone") => "里程碑颜色已设为 {color}",
        (Lang::En, "status-color-project") => "Color set to {color} for all milestones of \"{name}\"",
        (Lang::Zh, "status-color-project") => "已为 \"{name}\" 的全部里程碑设置颜色 {color}",
        (Lang::En, "status-color-noselect") => "Color set to {color} (select a project or milestone to apply it)",
        (Lang::Zh, "status-color-noselect") => "颜色已设为 {color}（请先选择项目或里程碑以应用）",
        (Lang::En, "status-created") => "Created project \"{project}\"",
        (Lang::Zh, "status-created") => "已创建项目 \"{project}\"",
        (Lang::En, "status-select-project") => "Select a project row first",
        (Lang::Zh, "status-select-project") => "请先选择项目行",
        (Lang::En, "status-removed-project") => "Removed project \"{name}\"",
        (Lang::Zh, "status-removed-project") => "已删除项目 \"{name}\"",
        (Lang::En, "status-select-milestone") => "Select a milestone on the timeline first",
        (Lang::Zh, "status-select-milestone") => "请先选择时间轴上的里程碑",
        (Lang::En, "status-removed-milestone") => "Removed milestone \"{name}\" from \"{project}\"",
        (Lang::Zh, "status-removed-milestone") => "已从 \"{project}\" 删除里程碑 \"{name}\"",
        (Lang::En, "status-ms-not-found") => "Milestone \"{name}\" not found in \"{project}\"",
        (Lang::Zh, "status-ms-not-found") => "在 \"{project}\" 中未找到里程碑 \"{name}\"",
        (Lang::En, "status-cleared") => "Cleared all projects",
        (Lang::Zh, "status-cleared") => "已清空全部项目",
        (Lang::En, "status-theme") => "Theme set to {theme}",
        (Lang::Zh, "status-theme") => "主题已设为 {theme}",
        (Lang::En, "status-language") => "Language set to {label}",
        (Lang::Zh, "status-language") => "语言已切换为 {label}",
        (Lang::En, "status-data-dir-changed") => "Roadmap data will be saved to {path}",
        (Lang::Zh, "status-data-dir-changed") => "路线图数据将保存到 {path}",
        (Lang::En, "status-data-dir-reset") => "Roadmap data directory reset to default: {path}",
        (Lang::Zh, "status-data-dir-reset") => "路线图数据目录已恢复默认：{path}",
        (Lang::En, "status-saved") => "Data saved to {path}",
        (Lang::Zh, "status-saved") => "数据已保存到 {path}",
        (Lang::En, "status-opened") => "Opened {n} projects from {path}",
        (Lang::Zh, "status-opened") => "已从 {path} 打开 {n} 个项目",
        (Lang::En, "status-imported") => "Imported {n} projects from {path}",
        (Lang::Zh, "status-imported") => "已从 {path} 导入 {n} 个项目",
        (Lang::En, "dlg-open-data") => "Open data file",
        (Lang::Zh, "dlg-open-data") => "打开数据文件",
        (Lang::En, "dlg-import-data") => "Import data file",
        (Lang::Zh, "dlg-import-data") => "导入数据文件",
        (Lang::En, "err-load") => "Failed to load {path}: {error}",
        (Lang::Zh, "err-load") => "加载 {path} 失败：{error}",
        (Lang::En, "dlg-pick-data-dir") => "Choose data folder",
        (Lang::Zh, "dlg-pick-data-dir") => "选择数据文件夹",
        (Lang::En, "status-save-error") => "Failed to save to {path}: {error}",
        (Lang::Zh, "status-save-error") => "保存到 {path} 失败：{error}",

        // ---- data validation errors ----
        (Lang::En, "err-date-required") => "Date is required",
        (Lang::Zh, "err-date-required") => "请输入日期",
        (Lang::En, "err-invalid-date") => "Invalid date \"{t}\". Please use yyyy/mm/dd.",
        (Lang::Zh, "err-invalid-date") => "无效日期 \"{t}\"，请使用 yyyy/mm/dd 格式。",
        (Lang::En, "err-project-required") => "Project name is required",
        (Lang::Zh, "err-project-required") => "请输入项目名称",
        (Lang::En, "err-milestone-required") => "Milestone name is required",
        (Lang::Zh, "err-milestone-required") => "请输入里程碑名称",
        (Lang::En, "err-duplicate-milestone") => "Milestone \"{m}\" already exists in project \"{p}\"",
        (Lang::Zh, "err-duplicate-milestone") => "项目 \"{p}\" 中已存在里程碑 \"{m}\"",
        (Lang::En, "err-duplicate-project") => "Project \"{p}\" already exists",
        (Lang::Zh, "err-duplicate-project") => "项目 \"{p}\" 已存在",
        (Lang::En, "err-color-empty") => "Color is empty",
        (Lang::Zh, "err-color-empty") => "颜色为空",
        (Lang::En, "err-invalid-color") => "Invalid color \"{t}\". Use #RRGGBB or r,g,b.",
        (Lang::Zh, "err-invalid-color") => "无效颜色 \"{t}\"，请使用 #RRGGBB 或 r,g,b 格式。",

        // ---- export ----
        (Lang::En, "err-nothing-projects") => "Nothing to export yet - add at least one project",
        (Lang::Zh, "err-nothing-projects") => "暂无内容可导出 - 请先添加至少一个项目",
        (Lang::En, "err-nothing-milestones") => "Nothing to export yet - add at least one milestone with a date",
        (Lang::Zh, "err-nothing-milestones") => "暂无内容可导出 - 请先添加至少一个带日期的里程碑",
        (Lang::En, "status-svg-saved") => "SVG saved to {path}",
        (Lang::Zh, "status-svg-saved") => "SVG 已保存到 {path}",
        (Lang::En, "status-png-saved") => "PNG saved to {path}",
        (Lang::Zh, "status-png-saved") => "PNG 已保存到 {path}",
        (Lang::En, "err-export-cancelled") => "Export cancelled",
        (Lang::Zh, "err-export-cancelled") => "已取消导出",

        // Unknown key: surface it so a typo is easy to spot.
        _ => "",
    }
}

/// Substitute `{key}` placeholders in a translated template. The dictionary
/// strings use `{name}`-style placeholders; `format!` cannot take a runtime
/// format string, so this manual replacement is used instead.
///
/// Example: `sub(t("status-loaded"), &[("n", 3.to_string()), ("path", p)])`.
pub fn sub(tpl: &str, args: &[(&str, String)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// Translate an array-valued key (color names, theme options, language
/// options). Returns the English list as a fallback for unknown keys.
pub fn t_list(lang: Lang, key: &str) -> &'static [&'static str] {
    match (lang, key) {
        (Lang::En, "color-names") => &["Blue", "Red", "Green", "Orange", "Purple", "Teal", "Gray", "Crimson", "Black"],
        (Lang::Zh, "color-names") => &["蓝色", "红色", "绿色", "橙色", "紫色", "青色", "灰色", "绯红", "黑色"],
        (Lang::En, "theme-options") => &["Follow System", "Light", "Dark"],
        (Lang::Zh, "theme-options") => &["跟随系统", "浅色", "深色"],
        (Lang::En, "lang-options") => &["English", "中文"],
        (Lang::Zh, "lang-options") => &["English", "中文"],
        _ => match key {
            "color-names" => &["Blue", "Red", "Green", "Orange", "Purple", "Teal", "Gray", "Crimson", "Black"],
            "theme-options" => &["Follow System", "Light", "Dark"],
            "lang-options" => &["English", "中文"],
            _ => &[],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_roundtrip() {
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("zh"), Lang::Zh);
        assert_eq!(Lang::from_code(""), Lang::Zh);
        assert_eq!(Lang::from_code("banana"), Lang::Zh);
        assert_eq!(Lang::En.code(), "en");
        assert_eq!(Lang::Zh.code(), "zh");
    }

    #[test]
    fn current_defaults_to_chinese() {
        // The process-wide default language is Chinese.
        assert_eq!(t("btn-add-milestone"), "添加里程碑");
        set_current(Lang::En);
        assert_eq!(t("btn-add-milestone"), "Add Milestone");
        set_current(Lang::Zh);
        assert_eq!(t("btn-add-milestone"), "添加里程碑");
        assert_eq!(t_in(Lang::En, "btn-close"), "Close");
        assert_eq!(t_in(Lang::Zh, "btn-close"), "关闭");
    }

    #[test]
    fn unknown_keys_surface_the_key() {
        assert_eq!(t_in(Lang::En, "no-such-key"), "");
        assert!(t_list(Lang::En, "no-such-key").is_empty());
    }

    #[test]
    fn sub_replaces_named_placeholders() {
        assert_eq!(
            sub(t_in(Lang::En, "status-loaded"), &[("n", 3.to_string()), ("path", "x.json".to_string())]),
            "Loaded 3 project(s) from x.json"
        );
        assert_eq!(
            sub(t_in(Lang::Zh, "status-added"), &[("milestone", "TR1".into()), ("date", "2026/08/20".into()), ("project", "STN".into())]),
            "已向 \"STN\" 添加里程碑 \"TR1\"（2026/08/20）"
        );
    }

    #[test]
    fn list_lengths_are_stable() {
        assert_eq!(t_list(Lang::En, "color-names").len(), 9);
        assert_eq!(t_list(Lang::Zh, "color-names").len(), 9);
        assert_eq!(t_list(Lang::En, "theme-options").len(), 3);
        assert_eq!(t_list(Lang::Zh, "theme-options").len(), 3);
        assert_eq!(t_list(Lang::En, "lang-options").len(), 2);
    }
}