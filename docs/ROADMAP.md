# Roadmap: Plith in Rust

Design: `specs/2026-11-14-rust-rewrite-design.md`. WPF history: `wpf/ROADMAP.md` (frozen).
Status: `✅ done, actual` / `🔄 in progress` / `⬜ next` / `⬜ waiting on <x>`.

| Phase | Scope | Owner | Estimate | Status |
|---|---|---|---|---|
| R0 Setup | Docs layout, ADRs, design doc, workspace, lints, structure check, CI | master | 1 session | ✅ done, 1 session (CI green on 0155a6d) |
| R1 Engine slice | Empty island, spring morph, frame clock, hit testing, motion log, demo driver; measured on console at 240 Hz | lane `engine` | 4 to 6 sessions | ✅ done, 1 lane session (merged 1aabe4a; ADR 0003 to 0006) |
| R2 Services | `plith-audio`, `plith-media`, `plith-system`, headless, each with a hardware probe | lane `services` | 3 to 5 sessions | ✅ done, 1 lane session (merged 4b50e83; Voicemeeter and WMI unverified on hardware) |
| R1b Engine under load | Missed vblanks while a game runs behind, collapse and page durations, topmost IME skip | lane `engine` | 1 to 2 sessions | ⬜ next |
| R3 Visual direction | Resting pill, tab bar, four pages, type, icons (praxvon-direction, impeccable) | master + Berke | 2 sessions | ⬜ next |
| R4 Pages | Tab bar, audio, media, system on the engine | ⬜ | 4 sessions | ⬜ waiting on R1, R2, R3 |
| R5 Shelf | In-process OLE drag in and out | ⬜ | 2 sessions | ⬜ waiting on R4 |
| R6 OSD helper | UIAccess band window, pipe, signing | ⬜ | 2 to 3 sessions | ⬜ next after R1b |
| R7 Parity | Settings, tray, installer, parity check against WPF 0.3.1, WPF removed | master | 3 sessions | ⬜ waiting on R4 to R6 |

## Settled scope (2026-11-14)
- Inside exclusive fullscreen games only the OSD shows (ADR 0002).
- Weather and the accent picker are dropped; one accent (lime).
