// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;
mod export;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use data::{day_number, format_date, RoadmapData};
use slint::{ModelRc, VecModel};

slint::include_modules!();

/// JSON data file lives next to the working directory (for `cargo run` this
/// is the project root; for a double-clicked exe it is the exe folder).
fn data_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("roadmap.json")
}

/// Convert a Slint color to its `#RRGGBB` string form.
fn color_to_hex(c: slint::Color) -> String {
    format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue())
}

/// Parse a `#RRGGBB` string into a Slint color, falling back to brand blue.
fn parse_slint_color(hex: &str) -> slint::Color {
    let h = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if h.len() == 6
        && h.chars().all(|c| c.is_ascii_hexdigit())
        && let Ok(v) = u32::from_str_radix(h, 16)
    {
        return slint::Color::from_rgb_u8(
            ((v >> 16) & 0xFF) as u8,
            ((v >> 8) & 0xFF) as u8,
            (v & 0xFF) as u8,
        );
    }
    slint::Color::from_rgb_u8(0x25, 0x63, 0xeb)
}

/// Convert the persistent data into Slint models and the shared time range.
fn build_models(data: &RoadmapData) -> (ModelRc<Project>, i32, i32) {
    let projects = Rc::new(VecModel::<Project>::default());

    let mut all_days: Vec<i32> = data
        .projects
        .iter()
        .flat_map(|p| p.milestones.iter().map(|m| day_number(m.date)))
        .collect();
    all_days.sort_unstable();

    // Shared time axis across all projects, padded a little on both sides so
    // the first/last markers are not glued to the edges.
    let (min_day, max_day) = match (all_days.first(), all_days.last()) {
        (Some(&a), Some(&b)) => {
            let pad = ((b - a) / 20 + 2).max(2);
            (a - pad, b + pad)
        }
        _ => (0, 1),
    };

    for p in &data.projects {
        let mut sorted = p.milestones.clone();
        sorted.sort_by_key(|m| m.date);
        let ms_model = Rc::new(VecModel::<Milestone>::default());
        for m in &sorted {
            let color = parse_slint_color(&m.color);
            // Normalize to the uppercase #RRGGBB form so it matches the presets.
            let color_text = data::parse_color(&m.color).unwrap_or_else(|_| m.color.clone());
            ms_model.push(Milestone {
                name: m.name.clone().into(),
                date_text: format_date(m.date).into(),
                day: day_number(m.date),
                color,
                color_text: color_text.into(),
            });
        }
        projects.push(Project {
            name: p.name.clone().into(),
            milestones: ms_model.into(),
        });
    }

    (projects.into(), min_day, max_day)
}

fn refresh_ui(ui: &AppWindow, data: &RoadmapData) {
    let (projects, min_day, max_day) = build_models(data);
    ui.set_projects(projects);
    ui.set_min_day(min_day);
    ui.set_max_day(max_day);
    ui.set_today_day(day_number(chrono::Local::now().date_naive()));
    // Name column follows the longest project name (14px bold in the UI,
    // plus 8px side padding on each edge and a small breathing gap).
    let max_name_w = data
        .projects
        .iter()
        .map(|p| data::text_width(&p.name, 14.0) * 1.05)
        .fold(0.0_f32, f32::max);
    ui.set_name_column_width((max_name_w + 24.0).max(120.0));
    ui.set_selected_project(-1);
    ui.set_selected_milestone(-1);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = data_path();
    let data = Rc::new(RefCell::new(data::load(&path)));

    let ui = AppWindow::new()?;
    refresh_ui(&ui, &data.borrow());

    {
        let n = data.borrow().projects.len();
        if n > 0 {
            ui.set_status_text(format!("Loaded {n} project(s) from {}", path.display()).into());
        } else {
            ui.set_status_text(format!("No saved data yet - data file: {}", path.display()).into());
        }
    }

    let ui_weak = ui.as_weak();

    // Add a milestone (creates the project if needed).
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let path = path.clone();
        ui.on_request_add_milestone(move |project, milestone, date| {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = data.borrow_mut();
            let color = color_to_hex(ui.get_preview_color());
            match data::add_milestone(&mut d, project.as_str(), milestone.as_str(), date.as_str(), &color) {
                Ok(()) => {
                    refresh_ui(&ui, &d);
                    ui.set_milestone_name("".into());
                    ui.set_milestone_date("".into());
                    ui.set_status_text(format!("Added milestone \"{milestone}\" ({date}) to \"{project}\"").into());
                    let _ = data::save(&d, &path);
                }
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Apply a new color:
    // - a milestone is selected -> only that milestone changes color;
    // - only a project is selected -> every milestone of that project changes.
    // The parsed color is fed back to the UI.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let path = path.clone();
        ui.on_request_set_color(move |input| {
            let Some(ui) = weak.upgrade() else { return };
            match data::parse_color(input.as_str()) {
                Ok(color) => {
                    // Feed the normalized color back to the picker UI.
                    ui.set_preview_color(parse_slint_color(&color));
                    let pidx = ui.get_selected_project();
                    let midx = ui.get_selected_milestone();
                    let mut d = data.borrow_mut();
                    if pidx >= 0 && (pidx as usize) < d.projects.len() {
                        let ms_name = ui.get_selected_milestone_name().to_string();
                        if !ms_name.is_empty()
                            && d.projects[pidx as usize]
                                .milestones
                                .iter()
                                .any(|m| m.name.eq_ignore_ascii_case(&ms_name))
                        {
                            // Milestone selected: recolor just this one. Match by NAME,
                            // not by index, because the UI model is date-sorted while
                            // the stored data keeps insertion order.
                            for m in &mut d.projects[pidx as usize].milestones {
                                if m.name.eq_ignore_ascii_case(&ms_name) {
                                    m.color = color.clone();
                                }
                            }
                            refresh_ui(&ui, &d);
                            // refresh_ui resets selection; restore it so the user
                            // can keep tweaking the same milestone.
                            ui.set_selected_project(pidx);
                            ui.set_selected_milestone(midx);
                            ui.set_selected_milestone_name(ms_name.into());
                            let _ = data::save(&d, &path);
                            ui.set_status_text(format!("Milestone color set to {color}").into());
                        } else {
                            // Only the project is selected: recolor all its milestones.
                            let name = d.projects[pidx as usize].name.clone();
                            for m in &mut d.projects[pidx as usize].milestones {
                                m.color = color.clone();
                            }
                            refresh_ui(&ui, &d);
                            ui.set_selected_project(pidx);
                            ui.set_selected_milestone(-1);
                            let _ = data::save(&d, &path);
                            ui.set_status_text(
                                format!("Color set to {color} for all milestones of \"{name}\"").into(),
                            );
                        }
                    } else {
                        ui.set_status_text(format!("Color set to {color} (select a project or milestone to apply it)").into());
                    }
                }
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Create an empty project.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let path = path.clone();
        ui.on_request_new_project(move |project| {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = data.borrow_mut();
            match data::add_project(&mut d, project.as_str()) {
                Ok(()) => {
                    refresh_ui(&ui, &d);
                    ui.set_project_name("".into());
                    ui.set_status_text(format!("Created project \"{project}\"").into());
                    let _ = data::save(&d, &path);
                }
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Remove the selected project.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let path = path.clone();
        ui.on_request_remove_selected_project(move || {
            let Some(ui) = weak.upgrade() else { return };
            let idx = ui.get_selected_project();
            if idx < 0 {
                ui.set_status_text("Select a project row first".into());
                return;
            }
            let mut d = data.borrow_mut();
            if (idx as usize) < d.projects.len() {
                let name = d.projects[idx as usize].name.clone();
                d.projects.remove(idx as usize);
                refresh_ui(&ui, &d);
                ui.set_status_text(format!("Removed project \"{name}\"").into());
                let _ = data::save(&d, &path);
            }
        });
    }

    // Remove the selected milestone.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let path = path.clone();
        ui.on_request_remove_selected_milestone(move || {
            let Some(ui) = weak.upgrade() else { return };
            let pidx = ui.get_selected_project();
            let ms_name = ui.get_selected_milestone_name().to_string();
            if pidx < 0 || ms_name.is_empty() {
                ui.set_status_text("Select a milestone on the timeline first".into());
                return;
            }
            let mut d = data.borrow_mut();
            if let Some(p) = d.projects.get_mut(pidx as usize) {
                // Match by NAME: the UI model is date-sorted while the stored
                // data keeps insertion order, so indices are not comparable.
                let project_name = p.name.clone();
                match p.milestones.iter().position(|m| m.name.eq_ignore_ascii_case(&ms_name)) {
                    Some(pos) => {
                        let name = p.milestones[pos].name.clone();
                        p.milestones.remove(pos);
                        refresh_ui(&ui, &d);
                        ui.set_status_text(format!("Removed milestone \"{name}\" from \"{project_name}\"").into());
                        let _ = data::save(&d, &path);
                    }
                    None => ui.set_status_text(format!("Milestone \"{ms_name}\" not found in \"{project_name}\"").into()),
                }
            }
        });
    }

    // Clear everything.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let path = path.clone();
        ui.on_request_clear_all(move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = data.borrow_mut();
            d.projects.clear();
            refresh_ui(&ui, &d);
            ui.set_status_text("Cleared all projects".into());
            let _ = data::save(&d, &path);
        });
    }

    // Export SVG.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        ui.on_request_export_svg(move || {
            let Some(ui) = weak.upgrade() else { return };
            let d = data.borrow();
            match export::export_svg(&d) {
                Ok(msg) => ui.set_status_text(msg.into()),
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Export PNG.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        ui.on_request_export_png(move || {
            let Some(ui) = weak.upgrade() else { return };
            let d = data.borrow();
            match export::export_png(&d) {
                Ok(msg) => ui.set_status_text(msg.into()),
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // About dialog.
    {
        let about = Rc::new(AboutDialog::new()?);
        let about = about.clone(); // strong ref moved into the closure
        ui.on_request_about(move || {
            let _ = about.show();
        });
    }

    let run_result = ui.run();

    // Auto-save on exit (data is also saved after every change).
    if let Err(e) = data::save(&data.borrow(), &path) {
        eprintln!("Failed to save roadmap data: {e}");
    }

    run_result?;
    Ok(())
}
