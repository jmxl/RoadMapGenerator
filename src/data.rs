//! Data model, JSON persistence, date parsing and tick computation.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoadmapData {
    pub projects: Vec<ProjectData>,
    /// UI theme preference: "auto" (follow system), "light" or "dark".
    #[serde(default = "default_theme")]
    pub theme: String,
}

/// Default UI theme (follow the system color scheme).
pub fn default_theme() -> String {
    "auto".into()
}

/// Normalize an arbitrary theme string to one of "auto" / "light" / "dark".
pub fn normalize_theme(s: &str) -> String {
    match s {
        "light" | "dark" => s.to_string(),
        _ => default_theme(),
    }
}

/// Load the roadmap from `path`. Returns an empty dataset if the file
/// does not exist or cannot be parsed (the error is logged to stderr).
pub fn load(path: &Path) -> RoadmapData {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to parse {}: {e}", path.display());
                RoadmapData::default()
            }
        },
        Err(_) => RoadmapData::default(),
    }
}

/// Persist the roadmap to `path` as pretty JSON.
pub fn save(data: &RoadmapData, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
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
        return Err("Date is required".into());
    }
    for fmt in ["%Y/%m/%d", "%Y-%m-%d", "%Y.%m.%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(t, fmt) {
            return Ok(d);
        }
    }
    Err(format!("Invalid date \"{t}\". Please use yyyy/mm/dd."))
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
        return Err("Project name is required".into());
    }
    if mname.is_empty() {
        return Err("Milestone name is required".into());
    }
    let date = parse_date(date)?;
    let color = parse_color(color)?;

    match data.projects.iter_mut().find(|p| p.name.eq_ignore_ascii_case(pname)) {
        Some(p) => {
            if p.milestones.iter().any(|m| m.name.eq_ignore_ascii_case(mname)) {
                return Err(format!("Milestone \"{mname}\" already exists in project \"{pname}\""));
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
        return Err("Project name is required".into());
    }
    if data.projects.iter().any(|p| p.name.eq_ignore_ascii_case(pname)) {
        return Err(format!("Project \"{pname}\" already exists"));
    }
    data.projects.push(ProjectData { name: pname.to_string(), milestones: Vec::new() });
    Ok(())
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
        return Err("Color is empty".into());
    }
    // rgb(r,g,b) or r,g,b
    if t.contains(',') {
        let parts: Vec<&str> = t.split(',').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid color \"{t}\". Use #RRGGBB or r,g,b."));
        }
        let mut rgb = [0u8; 3];
        for (i, p) in parts.iter().enumerate() {
            let v: u8 = p.trim().parse().map_err(|_| format!("Invalid color \"{t}\". Use #RRGGBB or r,g,b."))?;
            rgb[i] = v;
        }
        return Ok(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
    }
    // #RRGGBB or RRGGBB
    let h = t.strip_prefix('#').unwrap_or(t);
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Invalid color \"{t}\". Use #RRGGBB or r,g,b."));
    }
    let v = u32::from_str_radix(h, 16).map_err(|_| format!("Invalid color \"{t}\""))?;
    Ok(format!("#{:06X}", v & 0xFFFFFF))
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
        add_milestone(&mut data, "SPN", "TR1", "2026/08/20", "#2563eb").unwrap();
        add_milestone(&mut data, "spn", "TR4A", "2026/09/25", "#dc2626").unwrap();
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].milestones.len(), 2);
        assert_eq!(data.projects[0].milestones[1].color, "#DC2626");

        // duplicate milestone rejected
        assert!(add_milestone(&mut data, "SPN", "tr1", "2026/10/01", "#2563eb").is_err());
        // invalid date rejected
        assert!(add_milestone(&mut data, "SPN", "TR9", "bad", "#2563eb").is_err());
        // invalid color rejected
        assert!(add_milestone(&mut data, "SPN", "TR9", "2026/10/01", "notacolor").is_err());
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
    fn add_project_rejects_duplicates() {
        let mut data = RoadmapData::default();
        add_project(&mut data, "SPN").unwrap();
        assert!(add_project(&mut data, "spn").is_err());
    }

    #[test]
    fn save_load_roundtrip() {
        let path = std::env::temp_dir().join(format!("roadmap_test_{}.json", std::process::id()));
        let mut data = RoadmapData::default();
        data.theme = "dark".into();
        add_milestone(&mut data, "SPN", "TR1", "2026/08/20", "#2563eb").unwrap();
        save(&data, &path).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].milestones[0].date, date(2026, 8, 20));
        assert_eq!(loaded.theme, "dark");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_json_without_theme_defaults_to_auto() {
        // JSON written before the theme field existed must still load.
        let json = r##"{"projects":[{"name":"SPN","milestones":[{"name":"TR1","date":"2026-08-20","color":"#2563EB"}]}]}"##;
        let data: RoadmapData = serde_json::from_str(json).unwrap();
        assert_eq!(data.theme, "auto");
        assert_eq!(data.projects.len(), 1);
    }

    #[test]
    fn normalize_theme_maps_unknown_values_to_auto() {
        assert_eq!(normalize_theme("light"), "light");
        assert_eq!(normalize_theme("dark"), "dark");
        assert_eq!(normalize_theme(""), "auto");
        assert_eq!(normalize_theme("banana"), "auto");
    }
}
