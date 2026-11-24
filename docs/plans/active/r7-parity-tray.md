# R7 parity part 1: tray, exe identity, start with Windows

> A ticked box means the step was taken, not that nothing deviated.

**Goal:** a tray icon with a menu (`Ayarlar`, `Çıkış`), the exe carrying the Plith icon and a
version resource (workspace version 0.4.0), and a Genel row `Windows ile başlat` backed by the
HKCU Run value `Plith`.

**Spec:** `docs/specs/2026-11-24-r7-parity-tray-design.md`. Research: `docs/research/2026-11-24-r7-parity.md`.

## Steps
- [x] 1. `plith-core::tray::menu`: the menu model (items, order, ids) and the label per language.
- [x] 2. `plith-core::tray::run_value`: the Run value text for an exe path and the "is ours" parse.
- [x] 3. `plith-core::lang::sheet`: `RowAutostart`, `SubAutostart` in eight languages; `SheetFacts.autostart`;
      `SheetAction::Autostart`; the toggle row in Genel (hit test, keyboard, text).
- [x] 4. `plith-shell::tray`: notify icon v4, tooltip, `TaskbarCreated` re-add, popup menu with
      `SetForegroundWindow` first, removal on destroy; pure decode of the callback and tip fill tested.
      The Win32 calls themselves are not unit tested (see "Not proven" below).
- [x] 5. `plith::autostart`: Run key read, write, delete under HKCU; tested on a test-only subkey.
- [x] 6. `plith` wiring: tray events to the settings window and to a clean quit; the Genel row
      reads the Run value when the window opens and writes on a switch; menu language follows the setting.
- [x] 7. Build: `crates/plith/assets/plith.ico`, `build.rs` embeds icon and version through
      `winresource`; workspace version 0.4.0; Hakkında shows it.
- [x] 8. Render of Genel with the new row into `docs/design/r7-parity/`.
- [x] 9. Checks: fmt, clippy, tests, structure-check.

## Not proven until on screen
Tray icon and tooltip visible, left click opens settings, the Turkish menu, `Çıkış` gives `exit 0`
in the log, the row toggles the Run value (`reg query`), Explorer restart brings the icon back,
Explorer's properties show the icon and 0.4.0, the menu is readable on a dark Windows theme.

## Progress log
- 2026-11-24: core menu model and Run value text (tests first), eight-language row strings, the
  Genel toggle row (`SheetAction::Autostart`), shell tray module (decode tested, Win32 calls
  not), `autostart` module tested on `HKCU\Software\Plith-test` only (key removed afterwards,
  checked), app wiring with fakes for quit and the registry, `winresource` 0.1.34 pinned,
  workspace 0.4.0; `plith.exe` debug build carries icon, FileVersion and ProductVersion 0.4.0
  (read with `VersionInfo`, exe not started). Render in `docs/design/r7-parity/`.
- Decisions the spec left open: see the report of this step (icon copied not moved, `is ours` =
  file name `plith.exe`, no rewrite of a foreign value at start, menu in sentence case, tray only
  in a normal run).
