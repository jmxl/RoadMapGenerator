# AGENTS.md

Desktop roadmap planner (Slint UI + Rust). Projects are rows with milestones on a shared timeline; data persists to `~/.RoadMapGenerator/data.json`, settings to `~/.RoadMapGenerator/config.json`; exports SVG/PNG.

## Build & verify

- `cargo build` / `cargo run` / `cargo test` — no CI, no clippy/fmt/rustfmt configs, no rust-toolchain file.
- All tests are unit tests in `src/data.rs` and `src/export.rs` (incl. a real SVG→PNG pipeline test via resvg).
- `[profile.release]` uses `lto = true, codegen-units = 1` — release builds are slow; iterate in debug.
- Edition 2024 → Rust ≥ 1.85 required.

## Architecture

- `src/data.rs` — pure data model (`RoadmapData`/`ProjectData`/`MilestoneData`/`ConfigData`), JSON load/save, date & color parsing, day-number math, `text_width` heuristic. No Slint dependency.
- `src/export.rs` — hand-built SVG string + PNG rasterization (resvg @2x) + `rfd` save dialogs.
- `src/main.rs` — everything else: `slint::include_modules!()`, converts `data` ↔ Slint models, all `on_request_*` callback wiring (incl. File > Open / Import (Merge) / Save, which load or merge a user-picked `data.json` via `data::load_result`/`data::merge_projects` and persist via `AppContext::save_data_or_status`). Callbacks capture one shared `Rc<AppContext>` (data + config `RefCell`s; the JSON paths are derived from `ConfigData`, so they can never drift from the configured data directory) instead of cloning the pairs individually; `ui` is only ever captured as a weak handle to avoid an `Rc` cycle. Saves go through `AppContext::save_data_or_status`/`save_config_or_status`.
- `src/i18n.rs` — EN/ZH dictionary + process-wide current language (`t`/`t_in`/`t_list`/`sub`). No Slint dependency.
- `ui/app-window.slint` — main component. `ui/about-dialog.slint` is pulled in via `import`/`export` in `app-window.slint` so `AboutDialog` gets a generated Rust type. `ui/i18n.slint` holds the shared `I18n` global (all UI strings).

### Slint codegen gotcha

`build.rs` compiles **only** `ui/app-window.slint` via `slint_build::compile`. Any new `.slint` file must be `import`ed from `app-window.slint` (or added to `build.rs`) or it is never compiled. All UI strings are localized (EN/ZH) — see the i18n invariant below.

## Non-obvious invariants (do not break)

- **Milestone matching is by NAME, never by index.** The Slint UI model is date-sorted (`build_models` in main.rs) while `RoadmapData` keeps insertion order. Selection, removal, and recolor all use case-insensitive name matching (`eq_ignore_ascii_case`). `add_milestone` rejects duplicate milestone names per project, which is what makes name matching safe. The recolor/remove helpers (`recolor_milestone`/`recolor_project`/`remove_milestone`/`remove_project`) live in `data.rs` as pure, unit-tested functions; main.rs callbacks pass the project *index* (comparable: the UI project list preserves insertion order) plus the milestone *name*. `merge_projects` (File > Import (Merge)) follows the same rule: projects and milestones are merged by case-insensitive name, existing projects only gain milestones that are not already present.
- **Save failures are never silent.** Every save in main.rs goes through `save_data_or_status`/`save_config_or_status`, which surface the error in the status bar (`status-save-error`) and stderr instead of being swallowed with `let _ =`. The exit-time saves in `main()` keep plain `eprintln!` since the window is closing.
- **Data & settings split**: roadmap data (`RoadmapData.projects`) lives in `data.json` under `~/Documents/RoadMaps` by default, or under the user-chosen `ConfigData.data_dir` ("Save Location" in the settings window); app settings (`ConfigData.theme` / `ConfigData.language` / `ConfigData.data_dir`) live in `~/.RoadMapGenerator/config.json`. `config_dir()` resolves the home dir from `%USERPROFILE%` (Windows) / `$HOME` (elsewhere), falling back to `current_dir()` when it is unknown — never `current_exe()`. Both files are auto-saved after every mutation and on exit; the directory is created lazily on first save (`data::save`/`data::save_config`). Changing the data directory reloads from the new location (no migration). `export.rs` reads the language from the process-wide `i18n::current()`, not from the data.
- **Dates**: JSON stores chrono `NaiveDate` (ISO `yyyy-mm-dd`); input parsing accepts `yyyy/mm/dd`, `yyyy-mm-dd`, `yyyy.mm.dd`; UI displays `yyyy/mm/dd`.
- **Colors** are normalized to uppercase `#RRGGBB` (`data::parse_color`; accepts `#RRGGBB`, `RRGGBB`, `r,g,b`). `data::parse_rgb` is the single `#RRGGBB` hex parser shared by `parse_color` and the UI's `parse_slint_color` — never add a second hex parser. Default is brand blue `#2563eb` via serde default. Milestone colors stored per-milestone; a selected project recolors all its milestones, a selected milestone recolors only itself.
- **i18n**: All user-visible strings (EN/ZH) live in the dictionary in `src/i18n.rs` (`t`/`t_in`/`t_list`; `{name}` placeholders are filled with `i18n::sub`, never `format!` — it rejects runtime format strings). The UI layer copies them into the `I18n` global in `ui/i18n.slint` via `apply_language` (main.rs); all three `.slint` files import it and re-export through `app-window.slint`. The language is persisted as `ConfigData.language` (`"en"`/`"zh"`, serde default `"zh"`); the process-wide current language is `i18n::set_current`, so data/export error strings translate without threading a language parameter. `build_models` fills `Project.milestone-count` per language, and `refresh_ui` is re-run on language switch to update those rows.
- **`data::text_width`** (0.6em ASCII / 1.0em CJK heuristic) sizes both the UI name column and the SVG export layout — keep both usages in sync when changing font metrics.

## Platform quirks

- Release builds set `windows_subsystem = "windows"` → no console window; `eprintln!` (e.g. parse errors from `data::load`) is invisible in release.
- Exe icon comes from `ui/icon/logo.ico` embedded by `winresource` in `build.rs` (`WindowsResource::set_icon`, no `app.rc` file; no-op on non-Windows targets). `ui/icon/logo.png` is the in-app window icon (`@image-url("icon/logo.png")`, resolved relative to the `.slint` file).
- PNG export relies on system fonts via `resvg`'s `fontdb.load_system_fonts()`.
