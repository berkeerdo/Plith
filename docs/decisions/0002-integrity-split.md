# 0002 The notch runs at medium integrity; only the OSD is a UIAccess helper

Date: 2026-11-14. Status: accepted 2026-11-14.

## Context
WPF Release runs the whole app with UIAccess (high integrity) so the OSD shows over exclusive
fullscreen games. OLE drag and drop carries nothing from a normal app into a high integrity
window, so the shelf needed a second process (`Plith.DropCatcher`) and a window swap on every
page turn onto it. Measured in `docs/SHELF-VERIFICATION.md`.

## Decision
- `plith.exe` (notch, pages, services, shelf, tray, settings) runs at medium integrity.
- `plith-osd.exe` is a small signed UIAccess helper under Program Files that draws only the
  volume, brightness and mute OSD, fed over a named pipe.

## Consequences
- The shelf is one in-process surface; the drop catcher process goes away.
- The notch hides in exclusive fullscreen and may not take clicks while an elevated window is in
  front (UIPI). Borderless games keep the notch. Oyenza has the same limits.
- Berke confirmed (2026-11-14) that inside exclusive fullscreen games the OSD is enough; the
  full notch is not needed there.
