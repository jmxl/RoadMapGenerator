//! SVG generation and PNG rasterization for roadmap export.

use crate::data::{day_number, format_date, RoadmapData};
use crate::i18n::{self, Lang};

const W: f32 = 1200.0;
const X1: f32 = 1160.0; // timeline right edge
const NAME_X: f32 = 32.0; // project name left edge
const ROW_H: f32 = 58.0;
const HEADER_H: f32 = 34.0;
const TITLE_ZONE: f32 = 12.0; // small top margin above the tick header

/// XML-escape a string for safe embedding in SVG markup.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Truncate a string to `max_chars` characters, appending an ellipsis.
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max_chars {
        let mut t: String = chars[..max_chars].iter().collect();
        t.push('…');
        t
    } else {
        s.to_string()
    }
}

/// Build an SVG document for the roadmap dataset.
pub fn build_svg(data: &RoadmapData) -> Result<String, String> {
    let lang = Lang::from_code(&data.language);
    if data.projects.is_empty() {
        return Err(i18n::t_in(lang, "err-nothing-projects").into());
    }

    let all_days: Vec<i32> = data
        .projects
        .iter()
        .flat_map(|p| p.milestones.iter().map(|m| day_number(m.date)))
        .collect();
    if all_days.is_empty() {
        return Err(i18n::t_in(lang, "err-nothing-milestones").into());
    }

    let gmin = *all_days.iter().min().unwrap();
    let gmax = *all_days.iter().max().unwrap();
    let pad = ((gmax - gmin) / 20 + 3).max(3);
    let min_day = gmin - pad;
    let max_day = gmax + pad;
    let span = (max_day - min_day).max(1) as f32;

    let n = data.projects.len();
    let rows_top = TITLE_ZONE + HEADER_H;
    let height = rows_top + n as f32 * ROW_H + 24.0;

    // Timeline left edge follows the longest project name (13px bold):
    // wide enough to avoid overlap, tight enough to keep the timeline close.
    let max_name_w = data
        .projects
        .iter()
        .map(|p| crate::data::text_width(&truncate(&p.name, 18), 13.0))
        .fold(0.0_f32, f32::max);
    let x0 = NAME_X + max_name_w + 20.0; // 20px gap between name and timeline
    let x_of = |day: i32| x0 + (day as f32 - min_day as f32) / span * (X1 - x0);

    let mut s = String::new();
    s.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{height:.0}" viewBox="0 0 {W} {height:.0}" font-family="Segoe UI, Arial, sans-serif">"##
    ));
    s.push_str(&format!(r##"<rect x="0" y="0" width="{W}" height="{height:.0}" fill="#ffffff"/>"##));

    // project rows
    for (i, p) in data.projects.iter().enumerate() {
        let y = rows_top + i as f32 * ROW_H;

        // zebra background
        if i % 2 == 1 {
            s.push_str(&format!(
                r##"<rect x="{x0:.1}" y="{y:.1}" width="{:.1}" height="{ROW_H:.1}" fill="#f8fafc"/>"##,
                X1 - x0
            ));
        }

        // project name (left-aligned)
        s.push_str(&format!(
            r##"<text x="{NAME_X}" y="{:.1}" font-size="13" font-weight="700" fill="#111827">{}</text>"##,
            y + 22.0,
            esc(&truncate(&p.name, 18))
        ));

        // baseline
        let baseline = y + 29.0;
        s.push_str(&format!(
            r##"<line x1="{x0:.1}" y1="{baseline:.1}" x2="{X1}" y2="{baseline:.1}" stroke="#d1d5db" stroke-width="2"/>"##
        ));

        // milestones (sorted by date)
        let mut ms: Vec<_> = p.milestones.iter().collect();
        ms.sort_by_key(|m| m.date);
        for m in &ms {
            let x = x_of(day_number(m.date));
            // diamond marker (milestone color)
            s.push_str(&format!(
                r##"<rect x="{:.1}" y="{:.1}" width="9" height="9" rx="2.5" fill="{color}" stroke="#ffffff" stroke-width="1.5" transform="rotate(45 {x:.1} {baseline:.1})"/>"##,
                x - 4.5,
                baseline - 4.5,
                color = m.color
            ));
            // name above, date below (name uses the milestone color)
            s.push_str(&format!(
                r##"<text x="{x:.1}" y="{:.1}" font-size="11" font-weight="600" fill="{color}" text-anchor="middle">{}</text>"##,
                baseline - 13.0,
                esc(&m.name),
                color = m.color
            ));
            s.push_str(&format!(
                r##"<text x="{x:.1}" y="{:.1}" font-size="10" fill="#6b7280" text-anchor="middle">{}</text>"##,
                baseline + 21.0,
                esc(&format_date(m.date))
            ));
        }
    }

    // "We are here" marker: red dashed line drawn on top of row backgrounds
    let today = crate::data::day_number(chrono::Local::now().date_naive());
    if today >= min_day && today <= max_day {
        let x = x_of(today);
        // label sits right above the line, horizontally centered on it
        s.push_str(&format!(
            r##"<text x="{x:.1}" y="16" font-size="12" font-weight="700" fill="#dc2626" text-anchor="middle">{}</text>"##,
            i18n::t_in(lang, "today-label")
        ));
        // dashed line starts just below the label text (baseline 16 + descent ≈ 20)
        s.push_str(&format!(
            r##"<line x1="{x:.1}" y1="20" x2="{x:.1}" y2="{:.1}" stroke="#dc2626" stroke-width="2" stroke-dasharray="6,4"/>"##,
            rows_top + n as f32 * ROW_H
        ));
    }

    s.push_str("</svg>");
    Ok(s)
}

/// Rasterize an SVG document to PNG bytes at 2x scale for crisp output.
pub fn render_png(svg: &str) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &opt)
        .map_err(|e| format!("SVG parse failed: {e}"))?;

    let size = tree.size();
    let scale = 2.0_f32;
    let w = (size.width() * scale).round() as u32;
    let h = (size.height() * scale).round() as u32;

    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| "Failed to allocate pixmap".to_string())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let mut buf: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| format!("PNG header: {e}"))?;
        writer
            .write_image_data(pixmap.data())
            .map_err(|e| format!("PNG data: {e}"))?;
    }
    Ok(buf)
}

/// Show a save dialog and write the SVG file.
pub fn export_svg(data: &RoadmapData) -> Result<String, String> {
    let lang = Lang::from_code(&data.language);
    let svg = build_svg(data)?;
    let dialog = rfd::FileDialog::new()
        .add_filter("SVG image", &["svg"])
        .set_file_name("roadmap.svg");
    match dialog.save_file() {
        Some(path) => std::fs::write(&path, &svg)
            .map_err(|e| e.to_string())
            .map(|_| i18n::sub(i18n::t_in(lang, "status-svg-saved"), &[("path", path.display().to_string())])),
        None => Err(i18n::t_in(lang, "err-export-cancelled").into()),
    }
}

/// Show a save dialog and write the PNG file.
pub fn export_png(data: &RoadmapData) -> Result<String, String> {
    let lang = Lang::from_code(&data.language);
    let svg = build_svg(data)?;
    let bytes = render_png(&svg)?;
    let dialog = rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_file_name("roadmap.png");
    match dialog.save_file() {
        Some(path) => std::fs::write(&path, bytes)
            .map_err(|e| e.to_string())
            .map(|_| i18n::sub(i18n::t_in(lang, "status-png-saved"), &[("path", path.display().to_string())])),
        None => Err(i18n::t_in(lang, "err-export-cancelled").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{MilestoneData, ProjectData};
    use chrono::NaiveDate;

    fn sample() -> RoadmapData {
        RoadmapData {
            theme: "auto".into(),
            language: "en".into(),
            projects: vec![
                ProjectData {
                    name: "SPN".into(),
                    milestones: vec![
                        MilestoneData { name: "TR1".into(), date: NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(), color: "#2563eb".into() },
                        MilestoneData { name: "TR4A".into(), date: NaiveDate::from_ymd_opt(2026, 9, 25).unwrap(), color: "#dc2626".into() },
                    ],
                },
                ProjectData {
                    name: "Chip Project".into(),
                    milestones: vec![MilestoneData {
                        name: "TR3".into(),
                        date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                        color: "#16a34a".into(),
                    }],
                },
            ],
        }
    }

    #[test]
    fn svg_contains_expected_markup() {
        let svg = build_svg(&sample()).expect("svg");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">SPN</text>"));
        assert!(svg.contains(">TR1</text>"));
        assert!(svg.contains(">TR4A</text>"));
        assert!(svg.contains(">2026/08/20</text>"));
        assert!(svg.contains("rotate(45"));
    }

    #[test]
    fn svg_rejects_empty_data() {
        assert!(build_svg(&RoadmapData::default()).is_err());
        let no_ms = RoadmapData { theme: "auto".into(), language: "en".into(), projects: vec![ProjectData { name: "X".into(), milestones: vec![] }] };
        assert!(build_svg(&no_ms).is_err());
    }

    #[test]
    fn svg_escapes_markup() {
        let data = RoadmapData {
            theme: "auto".into(),
            language: "en".into(),
            projects: vec![ProjectData {
                name: "<A&B>".into(),
                milestones: vec![MilestoneData {
                    name: "\"Q\"".into(),
                    date: NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(),
                    color: "#2563eb".into(),
                }],
            }],
        };
        let svg = build_svg(&data).expect("svg");
        assert!(svg.contains("&lt;A&amp;B&gt;"));
        assert!(svg.contains("&quot;Q&quot;"));
        assert!(!svg.contains("<A&B>"));
    }

    #[test]
    fn png_pipeline_produces_valid_png() {
        let svg = build_svg(&sample()).expect("svg");
        let png = render_png(&svg).expect("png");
        assert!(png.len() > 1000, "png should not be tiny");
        // PNG magic header
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }
}
