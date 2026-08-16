//! Data model, JSON persistence, date parsing and tick computation.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::i18n;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MilestoneData {
    pub name: String,
    pub date: NaiveDate,
    #[serde(default = "default_color")]
    pub color: String,
}

/// Default milestone color (brand blue) used when a milestone has no color set.
pub fn default_color() -> String {
    "#2563eb".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectData {
    pub name: String,
    pub milestones: Vec<MilestoneData>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadmapData {
    pub projects: Vec<ProjectData>,
}

impl Default for RoadmapData {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
        }
    }
}

/// App settings (theme + language + data location), persisted separately
/// from the roadmap data in `config.json` under the user data directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigData {
    /// UI theme preference: "auto" (follow system), "light" or "dark".
    #[serde(default = "default_theme")]
    pub theme: String,
    /// UI language code: "en" (English) or "zh" (中文).
    #[serde(default = "default_language")]
    pub language: String,
    /// Custom directory for `data.json`. `None` = the default
    /// `~/Documents/RoadMaps` location.
    #[serde(default)]
    pub data_dir: Option<String>,
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            language: default_language(),
            data_dir: None,
        }
    }
}

/// Default UI theme (follow the system color scheme).
pub fn default_theme() -> String {
    "auto".into()
}

/// Default UI language (Chinese).
pub fn default_language() -> String {
    "zh".into()
}

/// Normalize an arbitrary theme string to one of "auto" / "light" / "dark".
pub fn normalize_theme(s: &str) -> String {
    match s {
        "light" | "dark" => s.to_string(),
        _ => default_theme(),
    }
}

/// Normalize an arbitrary language string to "en" / "zh", falling back to
/// the default language for anything else.
pub fn normalize_language(s: &str) -> String {
    match s {
        "en" | "zh" => s.to_string(),
        _ => default_language(),
    }
}

/// Load the roadmap from `path`. Returns an empty dataset if the file
/// does not exist or cannot be parsed (the error is logged to stderr).
pub fn load(path: &Path) -> RoadmapData {
    match load_result(path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("{e}");
            RoadmapData::default()
        }
    }
}

/// Load the roadmap from `path`, reporting read/parse failures as `Err`
/// instead of silently falling back to an empty dataset. Used by File > Open /
/// Import, where the user picked the file and must be told when it is invalid.
pub fn load_result(path: &Path) -> Result<RoadmapData, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

/// Persist the roadmap to `path` as pretty JSON, creating the parent
/// directory (the user data folder) on first save.
pub fn save(data: &RoadmapData, path: &Path) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Load the app settings from `path`. Returns the defaults if the file
/// does not exist or cannot be parsed (the error is logged to stderr).
pub fn load_config(path: &Path) -> ConfigData {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to parse {}: {e}", path.display());
                ConfigData::default()
            }
        },
        Err(_) => ConfigData::default(),
    }
}

/// Persist the app settings to `path` as pretty JSON, creating the parent
/// directory (the user data folder) on first save.
pub fn save_config(config: &ConfigData, path: &Path) -> Result<(), String> {
    ensure_parent_dir(path)?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Create the parent directory of `path` unless it already exists.
fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Roughly estimate the rendered width (px) of `s` at the given font size.
/// ASCII glyphs average ~0.6em; CJK/full-width glyphs are ~1.0em.
/// This lets the timeline's left edge follow the longest project name
/// instead of being a fixed constant.
pub fn text_width(s: &str, font_size: f32) -> f32 {
    s.chars()
        .map(|c| {
            let em = if (c as u32) > 0x2E80 { 1.0 } else { 0.6 };
            em * font_size
        })
        .sum()
}

/// Parse a user-entered date, accepting `yyyy/mm/dd`, `yyyy-mm-dd` and
/// `yyyy.mm.dd` (the `/` form is what the UI suggests).
pub fn parse_date(s: &str) -> Result<NaiveDate, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(i18n::t("err-date-required").into());
    }
    for fmt in ["%Y/%m/%d", "%Y-%m-%d", "%Y.%m.%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d);
        }
    }
    Err(i18n::sub(i18n::t("err-invalid-date"), &[("t", t.to_string())]))
}

/// Add a milestone to an existing project (case-insensitive match), or
/// create the project if it does not exist yet.
pub fn add_milestone(
    data: &mut RoadmapData,
    project: &str,
    milestone: &str,
    date: &str,
    color: &str,
) -> Result<(), String> {
    let pname = project.trim();
    let mname = milestone.trim();
    if pname.is_empty() {
        return Err(i18n::t("err-project-required").into());
    }
    if mname.is_empty() {
        return Err(i18n::t("err-milestone-required").into());
    }
    let date = parse_date(date)?;
    let color = parse_color(color)?;

    match data.projects.iter_mut().find(|p| p.name.eq_ignore_ascii_case(pname)) {
        Some(p) => {
            if p.milestones.iter().any(|m| m.name.eq_ignore_ascii_case(mname)) {
                return Err(i18n::sub(i18n::t("err-duplicate-milestone"), &[("m", mname.to_string()), ("p", pname.to_string())]));
            }
            p.milestones.push(MilestoneData { name: mname.to_string(), date, color });
        }
        None => {
            data.projects.push(ProjectData {
                name: pname.to_string(),
                milestones: vec![MilestoneData { name: mname.to_string(), date, color }],
            });
        }
    }
    Ok(())
}

/// Create an empty project (case-insensitive uniqueness check).
pub fn add_project(data: &mut RoadmapData, project: &str) -> Result<(), String> {
    let pname = project.trim();
    if pname.is_empty() {
        return Err(i18n::t("err-project-required").into());
    }
    if data.projects.iter().any(|p| p.name.eq_ignore_ascii_case(pname)) {
        return Err(i18n::sub(i18n::t("err-duplicate-project"), &[("p", pname.to_string())]));
    }
    data.projects.push(ProjectData { name: pname.to_string(), milestones: Vec::new() });
    Ok(())
}

/// Merge `imported` into `target` in place, matching projects and milestones
/// by name (case-insensitive, consistent with the rest of the app). A project
/// from `imported` whose name already exists in `target` only contributes the
/// milestones whose names are not already present; new projects are appended
/// whole. Returns the number of projects added to `target`.
pub fn merge_projects(target: &mut RoadmapData, imported: &RoadmapData) -> usize {
    let mut added = 0;
    for p in &imported.projects {
        match target.projects.iter_mut().find(|t| t.name.eq_ignore_ascii_case(&p.name)) {
            Some(t) => {
                for m in &p.milestones {
                    if !t.milestones.iter().any(|tm| tm.name.eq_ignore_ascii_case(&m.name)) {
                        t.milestones.push(m.clone());
                    }
                }
            }
            None => {
                target.projects.push(p.clone());
                added += 1;
            }
        }
    }
    added
}

/// Recolor every milestone whose name matches `ms_name` (case-insensitive)
/// inside the project at `project_idx`. Returns whether any milestone was
/// recolored. Matching is by name, never by index: the UI model is
/// date-sorted while the stored data keeps insertion order.
pub fn recolor_milestone(data: &mut RoadmapData, project_idx: usize, ms_name: &str, color: &str) -> bool {
    let Some(p) = data.projects.get_mut(project_idx) else {
        return false;
    };
    let mut recolored = false;
    for m in &mut p.milestones {
        if m.name.eq_ignore_ascii_case(ms_name) {
            m.color = color.to_string();
            recolored = true;
        }
    }
    recolored
}

/// Recolor every milestone of the project at `project_idx`. Returns whether
/// the project exists (out-of-range indices are a no-op).
pub fn recolor_project(data: &mut RoadmapData, project_idx: usize, color: &str) -> bool {
    match data.projects.get_mut(project_idx) {
        Some(p) => {
            for m in &mut p.milestones {
                m.color = color.to_string();
            }
            true
        }
        None => false,
    }
}

/// Remove the milestone named `ms_name` (case-insensitive) from the project
/// at `project_idx`, returning its name. Returns `None` when the project is
/// out of range or no milestone matches.
pub fn remove_milestone(data: &mut RoadmapData, project_idx: usize, ms_name: &str) -> Option<String> {
    let p = data.projects.get_mut(project_idx)?;
    let pos = p.milestones.iter().position(|m| m.name.eq_ignore_ascii_case(ms_name))?;
    Some(p.milestones.remove(pos).name)
}

/// Remove the project at `project_idx`, returning its name. Returns `None`
/// when the index is out of range.
pub fn remove_project(data: &mut RoadmapData, project_idx: usize) -> Option<String> {
    if project_idx < data.projects.len() {
        Some(data.projects.remove(project_idx).name)
    } else {
        None
    }
}

/// Days since the Unix epoch (1970-01-01). Used as the Slint `int` coordinate.
pub fn day_number(date: NaiveDate) -> i32 {
    date.signed_duration_since(epoch()).num_days() as i32
}

/// Format a date in the user's `yyyy/mm/dd` style.
pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y/%m/%d").to_string()
}

/// Parse a color string as `#RRGGBB`, `RRGGBB` or `r,g,b` (0-255 each),
/// returning the normalized `#RRGGBB` form.
pub fn parse_color(s: &str) -> Result<String, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(i18n::t("err-color-empty").into());
    }
    // rgb(r,g,b) or r,g,b
    if t.contains(',') {
        let parts: Vec<&str> = t.split(',').collect();
        if parts.len() != 3 {
            return Err(i18n::sub(i18n::t("err-invalid-color"), &[("t", t.to_string())]));
        }
        let mut rgb = [0u8; 3];
        for (i, p) in parts.iter().enumerate() {
            let v: u8 = p.trim().parse().map_err(|_| i18n::sub(i18n::t("err-invalid-color"), &[("t", t.to_string())]))?;
            rgb[i] = v;
        }
        return Ok(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
    }
    // #RRGGBB or RRGGBB
    match parse_rgb(t) {
        Some((r, g, b)) => Ok(format!("#{r:02X}{g:02X}{b:02X}")),
        None => Err(i18n::sub(i18n::t("err-invalid-color"), &[("t", t.to_string())])),
    }
}

/// Parse a `#RRGGBB` or `RRGGBB` string into its `(r, g, b)` components.
/// The single hex parser shared by the data layer (`parse_color`) and the
/// UI layer (Slint `Color` conversion) so both accept exactly the same input.
pub fn parse_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())
        && let Ok(v) = u32::from_str_radix(h, 16)
    {
        return Some((((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8));
    }
    None
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 is a valid date")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn parses_common_date_formats() {
        assert_eq!(parse_date("2026/08/20"), Ok(date(2026, 8, 20)));
        assert_eq!(parse_date("2026-08-20"), Ok(date(2026, 8, 20)));
        assert_eq!(parse_date("2026.8.20"), Ok(date(2026, 8, 20)));
        assert!(parse_date("20/08/2026").is_err());
        assert!(parse_date("").is_err());
        assert!(parse_date("not-a-date").is_err());
    }

    #[test]
    fn add_milestone_creates_and_reuses_project() {
        let mut data = RoadmapData::default();
        add_milestone(&mut data, "STN", "TR1", "2026/08/20", "#2563eb").unwrap();
        add_milestone(&mut data, "stn", "TR4A", "2026/09/25", "#dc2626").unwrap();
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].milestones.len(), 2);
        assert_eq!(data.projects[0].milestones[1].color, "#DC2626");

        // duplicate milestone rejected
        assert!(add_milestone(&mut data, "STN", "tr1", "2026/10/01", "#2563eb").is_err());
        // invalid date rejected
        assert!(add_milestone(&mut data, "STN", "TR9", "bad", "#2563eb").is_err());
        // invalid color rejected
        assert!(add_milestone(&mut data, "STN", "TR9", "2026/10/01", "notacolor").is_err());
        // blank fields rejected
        assert!(add_milestone(&mut data, "  ", "TR9", "2026/10/01", "#2563eb").is_err());
    }

    #[test]
    fn parses_color_formats() {
        assert_eq!(parse_color("#ff0000"), Ok("#FF0000".into()));
        assert_eq!(parse_color("00ff00"), Ok("#00FF00".into()));
        assert_eq!(parse_color("10,20,30"), Ok("#0A141E".into()));
        assert_eq!(parse_color("255, 0, 128"), Ok("#FF0080".into()));
        assert!(parse_color("xyz").is_err());
        assert!(parse_color("#12345").is_err());
        assert!(parse_color("").is_err());
    }

    #[test]
    fn parses_hex_rgb_components() {
        assert_eq!(parse_rgb("#FF0000"), Some((255, 0, 0)));
        assert_eq!(parse_rgb("00ff00"), Some((0, 255, 0)));
        assert_eq!(parse_rgb("#123456"), Some((0x12, 0x34, 0x56)));
        assert_eq!(parse_rgb(""), None);
        assert_eq!(parse_rgb("xyz"), None);
        assert_eq!(parse_rgb("#12345"), None);
    }

    #[test]
    fn add_project_rejects_duplicates() {
        let mut data = RoadmapData::default();
        add_project(&mut data, "STN").unwrap();
        assert!(add_project(&mut data, "stn").is_err());
    }

    #[test]
    fn recolor_milestone_matches_by_name_case_insensitively() {
        let mut data = RoadmapData::default();
        add_milestone(&mut data, "STN", "TR1", "2026/08/20", "#2563eb").unwrap();
        add_milestone(&mut data, "STN", "TR2", "2026/09/01", "#2563eb").unwrap();

        assert!(recolor_milestone(&mut data, 0, "tr1", "#FF0000"));
        assert_eq!(data.projects[0].milestones[0].color, "#FF0000");
        // The sibling milestone is untouched.
        assert_eq!(data.projects[0].milestones[1].color, "#2563EB");

        // Unknown name: no-op, reports nothing recolored.
        assert!(!recolor_milestone(&mut data, 0, "nope", "#00FF00"));
        // Out-of-range project: no-op.
        assert!(!recolor_milestone(&mut data, 9, "TR1", "#000000"));
    }

    #[test]
    fn recolor_project_recolors_all_milestones() {
        let mut data = RoadmapData::default();
        add_milestone(&mut data, "STN", "TR1", "2026/08/20", "#2563eb").unwrap();
        add_milestone(&mut data, "STN", "TR2", "2026/09/01", "#2563eb").unwrap();

        assert!(recolor_project(&mut data, 0, "#FF0000"));
        assert!(data.projects[0].milestones.iter().all(|m| m.color == "#FF0000"));
        // Out-of-range project: no-op.
        assert!(!recolor_project(&mut data, 9, "#00FF00"));
    }

    #[test]
    fn remove_milestone_matches_by_name_case_insensitively() {
        let mut data = RoadmapData::default();
        add_milestone(&mut data, "STN", "TR1", "2026/08/20", "#2563eb").unwrap();
        add_milestone(&mut data, "STN", "TR2", "2026/09/01", "#2563eb").unwrap();

        assert_eq!(remove_milestone(&mut data, 0, "tr2"), Some("TR2".into()));
        assert_eq!(data.projects[0].milestones.len(), 1);
        // Already removed: not found again.
        assert_eq!(remove_milestone(&mut data, 0, "tr2"), None);
        assert_eq!(remove_milestone(&mut data, 0, "nope"), None);
        // Out-of-range project: not found.
        assert_eq!(remove_milestone(&mut data, 9, "TR1"), None);
    }

    #[test]
    fn remove_project_removes_by_index() {
        let mut data = RoadmapData::default();
        add_project(&mut data, "STN").unwrap();
        add_project(&mut data, "TRA").unwrap();

        assert_eq!(remove_project(&mut data, 1), Some("TRA".into()));
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "STN");
        assert_eq!(remove_project(&mut data, 9), None);
    }

    #[test]
    fn save_load_roundtrip() {
        let path = std::env::temp_dir().join(format!("roadmap_test_{}.json", std::process::id()));
        let mut data = RoadmapData::default();
        add_milestone(&mut data, "STN", "TR1", "2026/08/20", "#2563eb").unwrap();
        save(&data, &path).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].milestones[0].date, date(2026, 8, 20));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_result_reports_missing_and_invalid_files() {
        let missing = std::env::temp_dir().join(format!("roadmap_missing_{}.json", std::process::id()));
        assert!(load_result(&missing).is_err());
        let _ = std::fs::remove_file(&missing);

        let invalid = std::env::temp_dir().join(format!("roadmap_invalid_{}.json", std::process::id()));
        std::fs::write(&invalid, "not json").unwrap();
        assert!(load_result(&invalid).is_err());
        let _ = std::fs::remove_file(&invalid);
    }

    #[test]
    fn merge_projects_adds_new_and_merges_existing() {
        let mut target = RoadmapData::default();
        add_milestone(&mut target, "STN", "TR1", "2026/08/20", "#2563eb").unwrap();
        add_project(&mut target, "TRA").unwrap();

        let mut imported = RoadmapData::default();
        // New project -> appended whole.
        add_milestone(&mut imported, "NEW", "N1", "2026/10/01", "#dc2626").unwrap();
        // Existing project (case-insensitive match) -> only new milestones;
        // "TR1" already exists in the *target*, so it must not be duplicated.
        add_milestone(&mut imported, "stn", "TR2", "2026/09/01", "#16a34a").unwrap();
        add_milestone(&mut imported, "STN", "TR1", "2026/11/01", "#7c3aed").unwrap();
        // Existing empty project -> gets milestones.
        add_milestone(&mut imported, "tra", "M1", "2026/12/01", "#0d9488").unwrap();

        let added = merge_projects(&mut target, &imported);
        assert_eq!(added, 1);
        assert_eq!(target.projects.len(), 3);
        assert_eq!(target.projects[0].name, "STN");
        assert_eq!(target.projects[0].milestones.len(), 2); // TR1 + TR2, no dup
        assert_eq!(target.projects[0].milestones[1].name, "TR2");
        assert_eq!(target.projects[1].name, "TRA");
        assert_eq!(target.projects[1].milestones.len(), 1);
        assert_eq!(target.projects[2].name, "NEW");
    }

    #[test]
    fn legacy_combined_json_still_loads_projects() {
        // JSON written before the data/settings split (projects + settings in
        // one file) must still load; serde ignores the unknown settings keys.
        let json = r##"{"projects":[{"name":"STN","milestones":[{"name":"TR1","date":"2026-08-20","color":"#2563EB"}]}],"theme":"light","language":"zh"}"##;
        let data: RoadmapData = serde_json::from_str(json).unwrap();
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].name, "STN");
    }

    #[test]
    fn config_roundtrip() {
        let path = std::env::temp_dir().join(format!("config_test_{}.json", std::process::id()));
        let mut config = ConfigData::default();
        config.theme = "dark".into();
        config.language = "zh".into();
        config.data_dir = Some("/data".into());
        save_config(&config, &path).unwrap();
        let loaded = load_config(&path);
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.language, "zh");
        assert_eq!(loaded.data_dir, Some("/data".into()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn config_with_missing_fields_defaults() {
        // JSON written before the theme/language/data_dir fields existed must
        // still load.
        let json = r##"{}"##;
        let config: ConfigData = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, "auto");
        assert_eq!(config.language, "zh");
        assert_eq!(config.data_dir, None);
    }

    #[test]
    fn save_creates_missing_parent_dir() {
        let dir = std::env::temp_dir().join(format!("roadmap_dir_test_{}", std::process::id()));
        let path = dir.join("data.json");
        save(&RoadmapData::default(), &path).unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_language_maps_unknown_values_to_zh() {
        assert_eq!(normalize_language("en"), "en");
        assert_eq!(normalize_language("zh"), "zh");
        assert_eq!(normalize_language(""), "zh");
        assert_eq!(normalize_language("banana"), "zh");
    }

    #[test]
    fn default_language_is_chinese() {
        assert_eq!(ConfigData::default().language, "zh");
    }

    #[test]
    fn normalize_theme_maps_unknown_values_to_auto() {
        assert_eq!(normalize_theme("light"), "light");
        assert_eq!(normalize_theme("dark"), "dark");
        assert_eq!(normalize_theme(""), "auto");
        assert_eq!(normalize_theme("banana"), "auto");
    }
}
