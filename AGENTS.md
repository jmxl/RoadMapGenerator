# AGENTS.md

Desktop roadmap planner (Slint UI + Rust). Projects are rows with milestones on a shared timeline; data persists to `roadmap.json`; exports SVG/PNG.

## Build & verify

- `cargo build` / `cargo run` / `cargo test` — no CI, no clippy/fmt/rustfmt configs, no rust-toolchain file.
- All tests are unit tests in `src/data.rs` and `src/export.rs` (incl. a real SVG→PNG pipeline test via resvg).
- `[profile.release]` uses `lto = true, codegen-units = 1` — release builds are slow; iterate in debug.
- Edition 2024 → Rust ≥ 1.85 required.

## Architecture

- `src/data.rs` — pure data model (`RoadmapData`/`ProjectData`/`MilestoneData`), JSON load/save, date & color parsing, day-number math, `text_width` heuristic. No Slint dependency.
- `src/export.rs` — hand-built SVG string + PNG rasterization (resvg @2x) + `rfd` save dialogs.
- `src/main.rs` — everything else: `slint::include_modules!()`, converts `data` ↔ Slint models, all `on_request_*` callback wiring.
- `ui/app-window.slint` — main component. `ui/about-dialog.slint` is pulled in via `import`/`export` in `app-window.slint` so `AboutDialog` gets a generated Rust type.

### Slint codegen gotcha

`build.rs` compiles **only** `ui/app-window.slint` via `slint_build::compile`. Any new `.slint` file must be `import`ed from `app-window.slint` (or added to `build.rs`) or it is never compiled. UI strings are English; the about dialog is partially Chinese.

## Non-obvious invariants (do not break)

- **Milestone matching is by NAME, never by index.** The Slint UI model is date-sorted (`build_models` in main.rs) while `RoadmapData` keeps insertion order. Selection, removal, and recolor all use case-insensitive name matching (`eq_ignore_ascii_case`). `add_milestone` rejects duplicate milestone names per project, which is what makes name matching safe.
- **`roadmap.json` location**: `data_path()` uses `current_dir()`, not `current_exe()` — project root for `cargo run`, exe folder when the exe is double-clicked. The file is gitignored; it's auto-saved after every mutation and on exit.
- **Dates**: JSON stores chrono `NaiveDate` (ISO `yyyy-mm-dd`); input parsing accepts `yyyy/mm/dd`, `yyyy-mm-dd`, `yyyy.mm.dd`; UI displays `yyyy/mm/dd`.
- **Colors** are normalized to uppercase `#RRGGBB` (`data::parse_color`; accepts `#RRGGBB`, `RRGGBB`, `r,g,b`). Default is brand blue `#2563eb` via serde default. Milestone colors stored per-milestone; a selected project recolors all its milestones, a selected milestone recolors only itself.
- **`data::text_width`** (0.6em ASCII / 1.0em CJK heuristic) sizes both the UI name column and the SVG export layout — keep both usages in sync when changing font metrics.

## Platform quirks

- Release builds set `windows_subsystem = "windows"` → no console window; `eprintln!` (e.g. parse errors from `data::load`) is invisible in release.
- Exe icon comes from `ui/icon/logo.ico` embedded via `app.rc` + `embed-resource` (no-op on non-Windows). `ui/icon/logo.png` is the in-app window icon (`@image-url("icon/logo.png")`, resolved relative to the `.slint` file).
- PNG export relies on system fonts via `resvg`'s `fontdb.load_system_fonts()`.
