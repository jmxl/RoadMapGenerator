// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;
mod export;
mod i18n;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use data::{day_number, format_date, normalize_theme, ConfigData, RoadmapData};
use slint::{ComponentHandle, ModelRc, VecModel};
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

/// Default directory for the roadmap data: `~/Documents/RoadMaps`. Unlike the
/// settings (which stay in the hidden `~/.RoadMapGenerator` folder), the
/// roadmap data is a user document, so it lives under the user's Documents.
fn default_data_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("Documents")
        .join("RoadMaps")
}

/// Roadmap data (`projects`) lives in `data.json` inside the configured data
/// directory: a user-chosen folder from `ConfigData.data_dir`, or the default
/// `~/Documents/RoadMaps` when unset.
fn data_path(config: &ConfigData) -> PathBuf {
    match &config.data_dir {
        Some(dir) => PathBuf::from(dir).join("data.json"),
        None => default_data_dir().join("data.json"),
    }
}

/// App settings (theme, language, data location) live in
/// `~/.RoadMapGenerator/config.json`.
fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

/// Shared, mutable application state handed to every UI callback: the roadmap
/// data, the settings, and the path of the *current* data file (the one the
/// data was loaded from and will be saved back to). The data file starts as
/// the `data.json` under the configured data directory (`ConfigData.data_dir`)
/// and follows File > Open / Save As; the settings window's "Save Location"
/// switches it back to the configured directory's `data.json`. Callbacks
/// capture a single `Rc<AppContext>` instead of cloning state individually;
/// `ui` is only ever captured as a weak handle to avoid an `Rc` cycle.
struct AppContext {
    data: RefCell<RoadmapData>,
    config: RefCell<ConfigData>,
    data_file: RefCell<PathBuf>,
}

impl AppContext {
    /// Path of the current data file (what File > Save and the exit save
    /// write to).
    fn data_path(&self) -> PathBuf {
        self.data_file.borrow().clone()
    }

    /// Switch the current data file (e.g. after File > Open / Save As, or
    /// after the "Save Location" setting changed the data directory).
    fn set_data_file(&self, path: PathBuf) {
        *self.data_file.borrow_mut() = path;
    }

    /// Persist the roadmap, surfacing a failure in the status bar (and stderr).
    /// A failed save is otherwise invisible to the user, who would believe
    /// their data was persisted.
    fn save_data_or_status(&self, ui: &AppWindow, data: &RoadmapData) {
        let path = self.data_path();
        if let Err(e) = data::save(data, &path) {
            ui.set_status_text(i18n::sub(i18n::t("status-save-error"), &[("path", path.display().to_string()), ("error", e.clone())]).into());
            eprintln!("Failed to save roadmap data to {}: {e}", path.display());
        }
    }

    /// Persist the settings, surfacing a failure in the status bar (and stderr).
    fn save_config_or_status(&self, ui: &AppWindow, config: &ConfigData) {
        let path = config_path();
        if let Err(e) = data::save_config(config, &path) {
            ui.set_status_text(i18n::sub(i18n::t("status-save-error"), &[("path", path.display().to_string()), ("error", e.clone())]).into());
            eprintln!("Failed to save settings to {}: {e}", path.display());
        }
    }
}

/// Convert a Slint color to its `#RRGGBB` string form.
fn color_to_hex(c: slint::Color) -> String {
    format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue())
}

/// Parse a `#RRGGBB` string into a Slint color, falling back to brand blue.
/// Delegates the hex parsing to `data::parse_rgb` (the single parser shared
/// with the data layer) instead of maintaining a second one here.
fn parse_slint_color(hex: &str) -> slint::Color {
    match data::parse_rgb(hex) {
        Some((r, g, b)) => slint::Color::from_rgb_u8(r, g, b),
        None => slint::Color::from_rgb_u8(0x25, 0x63, 0xeb),
    }
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

/// Center `child` on top of `parent` (physical pixels). Called right before
/// showing a secondary window so it opens in the middle of the main window.
fn center_on_parent(parent: &slint::Window, child: &slint::Window) {
    let p_pos = parent.position();
    let p_size = parent.size();
    let c_size = child.size();
    child.set_position(slint::PhysicalPosition::new(
        p_pos.x + (p_size.width as i32 - c_size.width as i32) / 2,
        p_pos.y + (p_size.height as i32 - c_size.height as i32) / 2,
    ));
}

/// Center the main window on the primary screen at startup. On platforms
/// without a screen-size query the OS default placement is kept.
fn center_on_screen(win: &slint::Window) {
    if let Some((sw, sh)) = screen_size() {
        let size = win.size();
        win.set_position(slint::PhysicalPosition::new(
            ((sw as i32 - size.width as i32) / 2).max(0),
            ((sh as i32 - size.height as i32) / 2).max(0),
        ));
    }
}

/// Primary screen size in physical pixels, via `user32` on Windows.
#[cfg(windows)]
fn screen_size() -> Option<(u32, u32)> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetSystemMetrics(n_index: std::ffi::c_int) -> std::ffi::c_int;
    }
    const SM_CXSCREEN: std::ffi::c_int = 0;
    const SM_CYSCREEN: std::ffi::c_int = 1;
    let w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if w > 0 && h > 0 {
        Some((w as u32, h as u32))
    } else {
        None
    }
}

#[cfg(not(windows))]
fn screen_size() -> Option<(u32, u32)> {
    None
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
    g.set_menu_file(t(lang, "menu-file").into());
    g.set_menu_open(t(lang, "menu-open").into());
    g.set_menu_import_merge(t(lang, "menu-import-merge").into());
    g.set_menu_save_as(t(lang, "menu-save-as").into());
    g.set_menu_save(t(lang, "menu-save").into());
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
    g.set_settings_data_dir(t(lang, "settings-data-dir").into());
    g.set_btn_pick_data_dir(t(lang, "btn-pick-data-dir").into());
    g.set_btn_reset_data_dir(t(lang, "btn-reset-data-dir").into());
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
    let config = data::load_config(&config_path());
    let data_file = data_path(&config);
    let app = Rc::new(AppContext {
        data: RefCell::new(data::load(&data_file)),
        config: RefCell::new(config),
        data_file: RefCell::new(data_file.clone()),
    });

    let ui = AppWindow::new()?;
    refresh_ui(&ui, &app.data.borrow());

    // Settings window (theme picker). Created before any callback wiring so the
    // initial theme can be applied to both windows up front.
    let settings = Rc::new(SettingsWindow::new()?);
    apply_theme(&ui, &settings, app.config.borrow().theme.as_str());

    // About dialog is created up front (like the settings window) so its
    // `I18n` global can be filled at startup and on language switch.
    let about = Rc::new(AboutDialog::new()?);

    // Language: normalize the persisted code, make it the process-wide current
    // language, then push every translated string into the UI.
    let lang = i18n::Lang::from_code(&data::normalize_language(&app.config.borrow().language));
    i18n::set_current(lang);
    apply_language(&ui, &settings, &about, lang);

    {
        let n = app.data.borrow().projects.len();
        if n > 0 {
            ui.set_status_text(i18n::sub(i18n::t("status-loaded"), &[("n", n.to_string()), ("path", data_file.display().to_string())]).into());
        } else {
            ui.set_status_text(i18n::sub(i18n::t("status-no-data"), &[("path", data_file.display().to_string())]).into());
        }
    }

    let ui_weak = ui.as_weak();

    // The winit window is created lazily, so `size()` reports (0,0) before the
    // window is shown. Recenter on the primary screen once the first frame has
    // rendered and the real size is known (no-op where it cannot be queried).
    let startup_center = slint::Timer::default();
    {
        let weak = ui_weak.clone();
        startup_center.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(100),
            move || {
                if let Some(ui) = weak.upgrade() {
                    center_on_screen(ui.window());
                }
            },
        );
    }

    // Add a milestone (creates the project if needed).
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_add_milestone(move |project, milestone, date| {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = app.data.borrow_mut();
            let color = color_to_hex(ui.get_preview_color());
            match data::add_milestone(&mut d, project.as_str(), milestone.as_str(), date.as_str(), &color) {
                Ok(()) => {
                    refresh_ui(&ui, &d);
                    ui.set_milestone_name("".into());
                    ui.set_milestone_date("".into());
                    ui.set_status_text(i18n::sub(i18n::t("status-added"), &[("milestone", milestone.to_string()), ("date", date.to_string()), ("project", project.to_string())]).into());
                    app.save_data_or_status(&ui, &d);
                }
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Apply a new color:
    // - a milestone is selected -> only that milestone changes color;
    // - only a project is selected -> every milestone of that project changes.
    // The parsed color is fed back to the UI. Milestone matching is by NAME
    // (in data.rs) because the UI model is date-sorted while the stored data
    // keeps insertion order; the project index IS comparable.
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_set_color(move |input| {
            let Some(ui) = weak.upgrade() else { return };
            match data::parse_color(input.as_str()) {
                Ok(color) => {
                    // Feed the normalized color back to the picker UI.
                    ui.set_preview_color(parse_slint_color(&color));
                    let pidx = ui.get_selected_project();
                    let midx = ui.get_selected_milestone();
                    let mut d = app.data.borrow_mut();
                    if pidx >= 0 {
                        let ms_name = ui.get_selected_milestone_name().to_string();
                        if !ms_name.is_empty() && data::recolor_milestone(&mut d, pidx as usize, &ms_name, &color) {
                            refresh_ui(&ui, &d);
                            // refresh_ui resets selection; restore it so the user
                            // can keep tweaking the same milestone.
                            ui.set_selected_project(pidx);
                            ui.set_selected_milestone(midx);
                            ui.set_selected_milestone_name(ms_name.into());
                            app.save_data_or_status(&ui, &d);
                            ui.set_status_text(i18n::sub(i18n::t("status-color-milestone"), &[("color", color.to_string())]).into());
                        } else if data::recolor_project(&mut d, pidx as usize, &color) {
                            // Only the project is selected: recolor all its milestones.
                            let name = d.projects[pidx as usize].name.clone();
                            refresh_ui(&ui, &d);
                            ui.set_selected_project(pidx);
                            ui.set_selected_milestone(-1);
                            app.save_data_or_status(&ui, &d);
                            ui.set_status_text(
                                i18n::sub(i18n::t("status-color-project"), &[("color", color.to_string()), ("name", name.to_string())]).into(),
                            );
                        } else {
                            ui.set_status_text(i18n::sub(i18n::t("status-color-noselect"), &[("color", color.to_string())]).into());
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
        let app = app.clone();
        ui.on_request_new_project(move |project| {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = app.data.borrow_mut();
            match data::add_project(&mut d, project.as_str()) {
                Ok(()) => {
                    refresh_ui(&ui, &d);
                    ui.set_project_name("".into());
                    ui.set_status_text(i18n::sub(i18n::t("status-created"), &[("project", project.to_string())]).into());
                    app.save_data_or_status(&ui, &d);
                }
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Remove the selected project.
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_remove_selected_project(move || {
            let Some(ui) = weak.upgrade() else { return };
            let idx = ui.get_selected_project();
            if idx < 0 {
                ui.set_status_text(i18n::t("status-select-project").into());
                return;
            }
            let mut d = app.data.borrow_mut();
            if let Some(name) = data::remove_project(&mut d, idx as usize) {
                refresh_ui(&ui, &d);
                ui.set_status_text(i18n::sub(i18n::t("status-removed-project"), &[("name", name.to_string())]).into());
                app.save_data_or_status(&ui, &d);
            }
        });
    }

    // Remove the selected milestone.
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_remove_selected_milestone(move || {
            let Some(ui) = weak.upgrade() else { return };
            let pidx = ui.get_selected_project();
            let ms_name = ui.get_selected_milestone_name().to_string();
            if pidx < 0 || ms_name.is_empty() {
                ui.set_status_text(i18n::t("status-select-milestone").into());
                return;
            }
            let mut d = app.data.borrow_mut();
            // Match by NAME (in data.rs): the UI model is date-sorted while the
            // stored data keeps insertion order, so indices are not comparable.
            let Some(p) = d.projects.get(pidx as usize) else { return };
            let project_name = p.name.clone();
            match data::remove_milestone(&mut d, pidx as usize, &ms_name) {
                Some(name) => {
                    refresh_ui(&ui, &d);
                    ui.set_status_text(i18n::sub(i18n::t("status-removed-milestone"), &[("name", name.to_string()), ("project", project_name.to_string())]).into());
                    app.save_data_or_status(&ui, &d);
                }
                None => ui.set_status_text(i18n::sub(i18n::t("status-ms-not-found"), &[("name", ms_name.to_string()), ("project", project_name.to_string())]).into()),
            }
        });
    }

    // Clear everything.
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_clear_all(move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut d = app.data.borrow_mut();
            d.projects.clear();
            refresh_ui(&ui, &d);
            ui.set_status_text(i18n::t("status-cleared").into());
            app.save_data_or_status(&ui, &d);
        });
    }

    // Export SVG.
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_export_svg(move || {
            let Some(ui) = weak.upgrade() else { return };
            let d = app.data.borrow();
            match export::export_svg(&d) {
                Ok(msg) => ui.set_status_text(msg.into()),
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Export PNG.
    {
        let weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_export_png(move || {
            let Some(ui) = weak.upgrade() else { return };
            let d = app.data.borrow();
            match export::export_png(&d) {
                Ok(msg) => ui.set_status_text(msg.into()),
                Err(e) => ui.set_status_text(e.into()),
            }
        });
    }

    // Show the settings window, centered on the main window. The window must be
// shown first: before that its winit window does not exist and `size()` is
// (0,0).
    {
        let ui_weak = ui_weak.clone();
        let settings = settings.clone();
        let app = app.clone();
        ui.on_request_settings(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            // Keep the "Save Location" row in sync with the current config.
            settings.set_data_dir_path(app.data_path().display().to_string().into());
            let _ = settings.show();
            center_on_parent(ui.window(), settings.window());
        });
    }

    // Theme changes from the settings window: persist to config.json, then
    // apply to both windows.
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        let settings_cb = settings.clone();
        settings.on_theme_changed(move |theme| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let theme = normalize_theme(theme.as_str());
            {
                let mut c = app.config.borrow_mut();
                if c.theme != theme {
                    c.theme = theme.clone();
                    app.save_config_or_status(&ui, &c);
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
        let app = app.clone();
        let settings_cb = settings.clone();
        let about_cb = about.clone();
        settings.on_language_changed(move |code| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let lang = i18n::Lang::from_code(code.as_str());
            i18n::set_current(lang);
            {
                let mut c = app.config.borrow_mut();
                if c.language != lang.code() {
                    c.language = lang.code().into();
                    app.save_config_or_status(&ui, &c);
                }
            }
            apply_language(&ui, &settings_cb, &about_cb, lang);
            refresh_ui(&ui, &app.data.borrow());
            ui.set_status_text(i18n::sub(i18n::t("status-language"), &[("label", lang.label().to_string())]).into());
        });
    }

    // About dialog, centered on the main window (shown first so its size is real).
    {
        let ui_weak = ui_weak.clone();
        let about = about.clone();
        ui.on_request_about(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let _ = about.show();
            center_on_parent(ui.window(), about.window());
        });
    }

    // File > Open: replace the current data with a picked data.json. The file is
    // loaded in memory only; nothing is persisted until File > Save (or exit).
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_open_file(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Some(path) = rfd::FileDialog::new()
                .set_title(i18n::t("dlg-open-data").to_string())
                .add_filter("JSON", &["json"])
                .pick_file()
            else { return };
            match data::load_result(&path) {
                Ok(loaded) => {
                    let n = loaded.projects.len();
                    // The opened file becomes the current data file, so File >
                    // Save (and the exit save) write back to it, not to the
                    // configured default.
                    app.set_data_file(path.clone());
                    *app.data.borrow_mut() = loaded;
                    refresh_ui(&ui, &app.data.borrow());
                    ui.set_status_text(i18n::sub(i18n::t("status-opened"), &[("n", n.to_string()), ("path", path.display().to_string())]).into());
                }
                Err(e) => {
                    ui.set_status_text(i18n::sub(i18n::t("err-load"), &[("path", path.display().to_string()), ("error", e.clone())]).into());
                    eprintln!("Failed to open {}: {e}", path.display());
                }
            }
        });
    }

    // File > Import (Merge): merge a picked data.json into the current data.
    // Projects are matched by name (case-insensitive); existing projects only
    // gain milestones that are not already present.
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_import_file(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Some(path) = rfd::FileDialog::new()
                .set_title(i18n::t("dlg-import-data").to_string())
                .add_filter("JSON", &["json"])
                .pick_file()
            else { return };
            match data::load_result(&path) {
                Ok(imported) => {
                    let added = data::merge_projects(&mut app.data.borrow_mut(), &imported);
                    refresh_ui(&ui, &app.data.borrow());
                    ui.set_status_text(i18n::sub(i18n::t("status-imported"), &[("n", added.to_string()), ("path", path.display().to_string())]).into());
                }
                Err(e) => {
                    ui.set_status_text(i18n::sub(i18n::t("err-load"), &[("path", path.display().to_string()), ("error", e.clone())]).into());
                    eprintln!("Failed to import {}: {e}", path.display());
                }
            }
        });
    }

    // File > Save: persist the current data immediately to the configured
    // location (the same save that otherwise runs after every mutation).
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        ui.on_request_save(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            app.save_data_or_status(&ui, &app.data.borrow());
            let path = app.data_path();
            ui.set_status_text(i18n::sub(i18n::t("status-saved"), &[("path", path.display().to_string())]).into());
        });
    }

    // File > Save As: save the current data to a user-picked path, then switch
    // the configured data directory to that file's folder (same effect as the
    // settings window's "Save Location"), so subsequent saves and the exit
    // save land there too.
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        let settings_cb = settings.clone();
        ui.on_request_save_as(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Some(path) = rfd::FileDialog::new()
                .set_title(i18n::t("dlg-save-data").to_string())
                .add_filter("JSON", &["json"])
                .set_file_name("data.json")
                .save_file()
            else { return };
            if let Err(e) = data::save(&app.data.borrow(), &path) {
                ui.set_status_text(i18n::sub(i18n::t("status-save-error"), &[("path", path.display().to_string()), ("error", e.clone())]).into());
                eprintln!("Failed to save roadmap data to {}: {e}", path.display());
                return;
            }
            // Switch the current data file and the configured data directory
            // (keeping the settings window's path in sync), so File > Save and
            // the exit save land in the picked file's folder.
            {
                let mut c = app.config.borrow_mut();
                if let Some(dir) = path.parent() {
                    c.data_dir = Some(dir.display().to_string());
                    app.save_config_or_status(&ui, &c);
                }
            }
            app.set_data_file(path.clone());
            settings_cb.set_data_dir_path(app.data_path().display().to_string().into());
            ui.set_status_text(i18n::sub(i18n::t("status-saved"), &[("path", path.display().to_string())]).into());
        });
    }

    // Pick a custom directory for data.json (settings window). The app reloads
    // from the new location; existing data is not migrated.
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        let settings_cb = settings.clone();
        settings.on_request_pick_data_dir(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Some(dir) = rfd::FileDialog::new()
                .set_title(i18n::t("dlg-pick-data-dir").to_string())
                .pick_folder()
            else { return };
            {
                let mut c = app.config.borrow_mut();
                c.data_dir = Some(dir.display().to_string());
                app.save_config_or_status(&ui, &c);
            }
            let data_file = data_path(&app.config.borrow());
            *app.data.borrow_mut() = data::load(&data_file);
            app.set_data_file(data_file.clone());
            refresh_ui(&ui, &app.data.borrow());
            settings_cb.set_data_dir_path(data_file.display().to_string().into());
            ui.set_status_text(i18n::sub(i18n::t("status-data-dir-changed"), &[("path", data_file.display().to_string())]).into());
        });
    }

    // Reset the data directory back to the default ~/Documents/RoadMaps.
    {
        let ui_weak = ui_weak.clone();
        let app = app.clone();
        let settings_cb = settings.clone();
        settings.on_request_reset_data_dir(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            {
                let mut c = app.config.borrow_mut();
                if c.data_dir.is_some() {
                    c.data_dir = None;
                    app.save_config_or_status(&ui, &c);
                }
            }
            let data_file = data_path(&app.config.borrow());
            *app.data.borrow_mut() = data::load(&data_file);
            app.set_data_file(data_file.clone());
            refresh_ui(&ui, &app.data.borrow());
            settings_cb.set_data_dir_path(data_file.display().to_string().into());
            ui.set_status_text(i18n::sub(i18n::t("status-data-dir-reset"), &[("path", data_file.display().to_string())]).into());
        });
    }

    let run_result = ui.run();

    // Auto-save on exit (both files are also saved after every change).
    if let Err(e) = data::save(&app.data.borrow(), &app.data_path()) {
        eprintln!("Failed to save roadmap data: {e}");
    }
    if let Err(e) = data::save_config(&app.config.borrow(), &config_path()) {
        eprintln!("Failed to save settings: {e}");
    }

    run_result?;
    Ok(())
}
