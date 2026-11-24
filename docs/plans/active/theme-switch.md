# Theme switch Implementation Plan

> A ticked box means the step was taken, not that nothing deviated.

**Goal:** A Görünüm settings section with one Tema choice row (GECE / KAĞIT / YORK / SODYUM),
key `theme` in `settings.ini`, `--theme <key>` override, live apply to the island and the
settings window, atomic save. The OSD helper is out of scope.

**Spec:** `docs/specs/2026-11-24-theme-switch-design.md` (main checkout, not committed yet).

## Steps
- [x] 1. `plith-core::theme`: `Theme::ALL`, `key`, `from_key`; round trip and unknown key tests.
- [x] 2. `plith-core::settings`: `theme` field, parse, serialize, unknown value falls back and is
      reported; `SettingChange::Theme`, `Effects.theme` (a change asks for a page bitmap rebuild).
- [x] 3. `plith-core::launch`: `--theme <key>`.
- [x] 4. `plith-core::sheet`: `Section::Look` between General and Idle, the Tema row, hit test,
      focus order, arrow keys; strings in eight languages and fit budgets.
- [x] 5. `plith-render`: `SheetRender::set_palette`; island loop takes a palette from the owner
      (`Paint::set_palette`, `RelayoutHandle::set_palette`), resets warm pages, repaints at rest.
- [x] 6. `plith`: app applies the theme (island, window), `--theme` wins over the file and the
      file keeps its value until a pick, window opens in the current palette.
- [x] 7. Renders of the window and one page per theme in `docs/design/theme-switch/`.
- [x] 8. Checks: fmt, clippy, tests, structure-check.
