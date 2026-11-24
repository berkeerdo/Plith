# R7 parity, part 1: tray, exe identity, start with Windows

Date: 2026-11-24. Status: design, my call under Berke's "önerilerinle devam edelim" (2026-11-24);
he can veto any point. Facts: `research/2026-11-24-r7-parity.md` (gaps ranked; this part takes
blockers 2, 3 and 4). Part 2 is the installer and the WPF settings import.

## 1. Problem
The Rust app has no tray icon, no exe icon or version, and no way to start with Windows. The only
clean quit is a `WM_CLOSE` sent by a script. WPF 0.3.1 had all three (`src/Plith/Services/
TrayIconHost.cs`, `AutoStartService.cs`).

## 2. Goals, non-goals
Goals
- **Tray icon** (`Shell_NotifyIconW`, version 4) with the Plith icon and the tooltip `Plith`. Left
  click opens the settings window (the same path as the quick page's `Tüm ayarlar`). Right click
  opens a menu: `Ayarlar`, a separator, `Çıkış` (all eight languages). `Çıkış` closes the app the
  same way `WM_CLOSE` does today (clean `exit 0`, helper stopped). The icon comes back after an
  Explorer restart (`TaskbarCreated`). Removed on exit.
- **Exe identity:** `plith.exe` carries the Plith icon and a version resource. The workspace
  version becomes `0.4.0`, above the last WPF release, so Windows and an upgrade see it as newer.
  Hakkında shows the same version. The icon file moves to `crates/plith/assets/plith.ico`, so it
  survives the WPF removal.
- **Start with Windows:** a Genel row `Windows ile başlat`, a toggle. The truth is the HKCU
  `Software\Microsoft\Windows\CurrentVersion\Run` value `Plith` (WPF's name, so an upgrade takes it
  over); the row reads it when the window opens and writes or deletes it on a switch. The value is
  the quoted full path of the running `plith.exe`. Not stored in `settings.ini`.
Non-goals
- Installer, uninstall, WPF settings import (part 2). Diagnostic bundle, update check, hotkeys.
- A tray menu that mirrors the pages. The island is the app; the tray is a door and an exit.

## 3. Shape
- `plith-core`: the menu model (items, order, labels per language), the Run value text and its
  parse (is this value ours), unit tested.
- `plith-shell`: the notify icon on the island's UI thread (its own message-only window or the
  island window's message loop; no new thread, no timer), the popup menu with
  `TrackPopupMenuEx` after `SetForegroundWindow` as Windows requires.
- `plith`: menu commands to the existing settings-open and close paths; the Run key read and
  write (`RegSetValueExW` / `RegDeleteValueW`), the Genel row.
- Build: a `build.rs` in `crates/plith` that embeds the icon and the version resource through a
  maintained crate (check with context7 and crates.io; pin it).

## 4. Proof
- Unit tests for every pure rule; five checks green; idle check unchanged (no new wakes).
- Offscreen render of the Genel section with the new row.
- On screen (RDP is fine): the tray icon with its tooltip, left click opens settings, the menu
  in Turkish, `Çıkış` gives `exit 0` in the log, the row toggles the Run value (checked with
  `reg query`), Explorer restart brings the icon back, the exe shows its icon and `0.4.0` in
  Explorer's properties.
