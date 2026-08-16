// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;
mod export;
mod i18n;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use data::{day_number, format_date, normalize_theme, RoadmapData};
use slint::{ModelRc, VecModel};
use slint::private_unstable_api::re_exports::ColorScheme;

slint::include_modules!();

/// User data directory: `~/.RoadMapGenerator` (Windows: `%USERPROFILE%`,
/// elsewhere: `$HOME`). Falls back to the working directory when the home
/// dir cannot be determined. Created lazily on first save.
fn config_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(".RoadMapGenerator")
}

/// Best-effort home directory, avoiding the deprecated `std::env::home_dir`.
fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Roadmap data (`projects`) lives in `~/.RoadMapGenerator/data.json`.
fn data_path() -> PathBuf {
    config_dir().join("data.json")
}

/// App settings (theme, language) live in `~/.RoadMapGenerator/config.json`.
fn config_path() -> PathBuf {
    config_dir().join("config.json")
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
            milestone_count: i18n::sub(i18n::t("milestone-count"), &[("n", p.milestones.len().to_string())]).into(),
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

/// Force the color scheme of both windows via the widget style's `Palette`
/// global. `ColorScheme::Unknown` restores "follow the system" (the widget
/// styles fall back to the OS scheme in that case).
fn apply_theme(ui: &AppWindow, settings: &SettingsWindow, theme: &str) {
    let scheme = match theme {
        "light" => ColorScheme::Light,
        "dark" => ColorScheme::Dark,
        _ => ColorScheme::Unknown,
    };
    ui.global::<Palette>().set_color_scheme(scheme);
    settings.global::<Palette>().set_color_scheme(scheme);
    settings.set_theme(theme.into());
}

/// Push the translated strings for `lang` into the `I18n` global of every
/// window and sync the settings window's language picker.
///
/// Slint globals are per component instance: each `::new()`'d window owns a
/// separate copy of the `I18n` global (the same reason `apply_theme` sets
/// `Palette` on both windows explicitly), so updating through one window
/// never reaches the others. All three must be written explicitly.
fn apply_language(ui: &AppWindow, settings: &SettingsWindow, about: &AboutDialog, lang: i18n::Lang) {
    set_i18n_globals(&ui.global::<I18n>(), lang);
    set_i18n_globals(&settings.global::<I18n>(), lang);
    set_i18n_globals(&about.global::<I18n>(), lang);
    settings.set_language(lang.code().into());
}

/// Write every translated string into one window's `I18n` global instance.
fn set_i18n_globals(g: &I18n, lang: i18n::Lang) {
    use i18n::t_in as t;
    g.set_app_title(t(lang, "app-title").into());
    g.set_menu_settings(t(lang, "menu-settings").into());
    g.set_menu_help(t(lang, "menu-help").into());
    g.set_menu_about(t(lang, "menu-about").into());
    g.set_placeholder_project(t(lang, "placeholder-project").into());
    g.set_placeholder_milestone(t(lang, "placeholder-milestone").into());
    g.set_placeholder_date(t(lang, "placeholder-date").into());
    g.set_btn_add_milestone(t(lang, "btn-add-milestone").into());
    g.set_label_color(t(lang, "label-color").into());
    g.set_btn_new_project(t(lang, "btn-new-project").into());
    g.set_btn_remove_project(t(lang, "btn-remove-project").into());
    g.set_btn_remove_milestone(t(lang, "btn-remove-milestone").into());
    g.set_btn_clear_all(t(lang, "btn-clear-all").into());
    g.set_btn_export_svg(t(lang, "btn-export-svg").into());
    g.set_btn_export_png(t(lang, "btn-export-png").into());
    g.set_today_label(t(lang, "today-label").into());
    g.set_empty_hint(t(lang, "empty-hint").into());
    g.set_settings_title(t(lang, "settings-title").into());
    g.set_theme_label(t(lang, "theme-label").into());
    g.set_theme_hint(t(lang, "theme-hint").into());
    g.set_lang_label(t(lang, "lang-label").into());
    g.set_lang_hint(t(lang, "lang-hint").into());
    g.set_btn_close(t(lang, "btn-close").into());
    g.set_about_title(t(lang, "about-title").into());
    g.set_about_btn(t(lang, "about-btn").into());

    let to_model = |list: &'static [&'static str]| -> ModelRc<slint::SharedString> {
        Rc::new(VecModel::from(
            list.iter().map(|s| slint::SharedString::from(*s)).collect::<Vec<_>>(),
        ))
        .into()
    };
    g.set_color_names(to_model(i18n::t_list(lang, "color-names")));
    g.set_theme_options(to_model(i18n::t_list(lang, "theme-options")));
    g.set_lang_options(to_model(i18n::t_list(lang, "lang-options")));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_file = data_path();
    let cfg_file = config_path();

    let data = Rc::new(RefCell::new(data::load(&data_file)));
    let config = Rc::new(RefCell::new(data::load_config(&cfg_file)));

    let ui = AppWindow::new()?;
    refresh_ui(&ui, &data.borrow());

    // Settings window (theme picker). Created before any callback wiring so the
    // initial theme can be applied to both windows up front.
    let settings = Rc::new(SettingsWindow::new()?);
    apply_theme(&ui, &settings, config.borrow().theme.as_str());

    // About dialog is created up front (like the settings window) so its
    // `I18n` global can be filled at startup and on language switch.
    let about = Rc::new(AboutDialog::new()?);

    // Language: normalize the persisted code, make it the process-wide current
    // language, then push every translated string into the UI.
    let lang = i18n::Lang::from_code(&data::normalize_language(&config.borrow().language));
    i18n::set_current(lang);
    apply_language(&ui, &settings, &about, lang);

    {
        let n = data.borrow().projects.len();
        if n > 0 {
            ui.set_status_text(i18n::sub(i18n::t("status-loaded"), &[("n", n.to_string()), ("path", data_file.display().to_string())]).into());
        } else {
            ui.set_status_text(i18n::sub(i18n::t("status-no-data"), &[("path", data_file.display().to_string())]).into());
        }
    }

    let ui_weak = ui.as_weak();

    // Add a milestone (creates the project if needed).
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let data_file = data_file.clone();
        ui.on_request_add_milestone(move |project, milestone, date| {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = data.borrow_mut();
            let color = color_to_hex(ui.get_preview_color());
            match data::add_milestone(&mut d, project.as_str(), milestone.as_str(), date.as_str(), &color) {
                Ok(()) => {
                    refresh_ui(&ui, &d);
                    ui.set_milestone_name("".into());
                    ui.set_milestone_date("".into());
                    ui.set_status_text(i18n::sub(i18n::t("status-added"), &[("milestone", milestone.to_string()), ("date", date.to_string()), ("project", project.to_string())]).into());
                    let _ = data::save(&d, &data_file);
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
        let data_file = data_file.clone();
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
                            let _ = data::save(&d, &data_file);
                            ui.set_status_text(i18n::sub(i18n::t("status-color-milestone"), &[("color", color.to_string())]).into());
                        } else {
                            // Only the project is selected: recolor all its milestones.
                            let name = d.projects[pidx as usize].name.clone();
                            for m in &mut d.projects[pidx as usize].milestones {
                                m.color = color.clone();
                            }
                            refresh_ui(&ui, &d);
                            ui.set_selected_project(pidx);
                            ui.set_selected_milestone(-1);
                            let _ = data::save(&d, &data_file);
                            ui.set_status_text(
                                i18n::sub(i18n::t("status-color-project"), &[("color", color.to_string()), ("name", name.to_string())]).into(),
                            );
                        }
                    } else {
                        ui.set_status_text(i18n::sub(i18n::t("status-color-noselect"), &[("color", color.to_string())]).into());
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
        let data_file = data_file.clone();
        ui.on_request_new_project(move |project| {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = data.borrow_mut();
            match data::add_project(&mut d, project.as_str()) {
                Ok(()) => {
                    refresh_ui(&ui, &d);
                    ui.set_project_name("".into());
                    ui.set_status_text(i18n::sub(i18n::t("status-created"), &[("project", project.to_string())]).into());
                    let _ = data::save(&d, &data_file);
                }
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Remove the selected project.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let data_file = data_file.clone();
        ui.on_request_remove_selected_project(move || {
            let Some(ui) = weak.upgrade() else { return };
            let idx = ui.get_selected_project();
            if idx < 0 {
                ui.set_status_text(i18n::t("status-select-project").into());
                return;
            }
            let mut d = data.borrow_mut();
            if (idx as usize) < d.projects.len() {
                let name = d.projects[idx as usize].name.clone();
                d.projects.remove(idx as usize);
                refresh_ui(&ui, &d);
                ui.set_status_text(i18n::sub(i18n::t("status-removed-project"), &[("name", name.to_string())]).into());
                let _ = data::save(&d, &data_file);
            }
        });
    }

    // Remove the selected milestone.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let data_file = data_file.clone();
        ui.on_request_remove_selected_milestone(move || {
            let Some(ui) = weak.upgrade() else { return };
            let pidx = ui.get_selected_project();
            let ms_name = ui.get_selected_milestone_name().to_string();
            if pidx < 0 || ms_name.is_empty() {
                ui.set_status_text(i18n::t("status-select-milestone").into());
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
                        ui.set_status_text(i18n::sub(i18n::t("status-removed-milestone"), &[("name", name.to_string()), ("project", project_name.to_string())]).into());
                        let _ = data::save(&d, &data_file);
                    }
                    None => ui.set_status_text(i18n::sub(i18n::t("status-ms-not-found"), &[("name", ms_name.to_string()), ("project", project_name.to_string())]).into()),
                }
            }
        });
    }

    // Clear everything.
    {
        let weak = ui_weak.clone();
        let data = data.clone();
        let data_file = data_file.clone();
        ui.on_request_clear_all(move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = data.borrow_mut();
            d.projects.clear();
            refresh_ui(&ui, &d);
            ui.set_status_text(i18n::t("status-cleared").into());
            let _ = data::save(&d, &data_file);
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

    // Show the settings window (like the About dialog).
    {
        let settings = settings.clone();
        ui.on_request_settings(move || {
            let _ = settings.show();
        });
    }

    // Theme changes from the settings window: persist to config.json, then
    // apply to both windows.
    {
        let ui_weak = ui_weak.clone();
        let config = config.clone();
        let cfg_file = cfg_file.clone();
        let settings_cb = settings.clone();
        settings.on_theme_changed(move |theme| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let theme = normalize_theme(theme.as_str());
            {
                let mut c = config.borrow_mut();
                if c.theme != theme {
                    c.theme = theme.clone();
                    let _ = data::save_config(&c, &cfg_file);
                }
            }
            apply_theme(&ui, &settings_cb, &theme);
            ui.set_status_text(i18n::sub(i18n::t("status-theme"), &[("theme", theme.to_string())]).into());
        });
    }

    // Language changes from the settings window: persist to config.json, then
    // re-apply every translated string to all three windows and rebuild the
    // models (milestone counts are localized).
    {
        let ui_weak = ui_weak.clone();
        let data = data.clone();
        let config = config.clone();
        let cfg_file = cfg_file.clone();
        let settings_cb = settings.clone();
        let about_cb = about.clone();
        settings.on_language_changed(move |code| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let lang = i18n::Lang::from_code(code.as_str());
            i18n::set_current(lang);
            {
                let mut c = config.borrow_mut();
                if c.language != lang.code() {
                    c.language = lang.code().into();
                    let _ = data::save_config(&c, &cfg_file);
                }
            }
            apply_language(&ui, &settings_cb, &about_cb, lang);
            refresh_ui(&ui, &data.borrow());
            ui.set_status_text(i18n::sub(i18n::t("status-language"), &[("label", lang.label().to_string())]).into());
        });
    }

    // About dialog.
    {
        let about = about.clone();
        ui.on_request_about(move || {
            let _ = about.show();
        });
    }

    let run_result = ui.run();

    // Auto-save on exit (both files are also saved after every change).
    if let Err(e) = data::save(&data.borrow(), &data_file) {
        eprintln!("Failed to save roadmap data: {e}");
    }
    if let Err(e) = data::save_config(&config.borrow(), &cfg_file) {
        eprintln!("Failed to save settings: {e}");
    }

    run_result?;
    Ok(())
}
