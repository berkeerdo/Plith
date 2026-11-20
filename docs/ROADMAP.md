# Roadmap: Plith in Rust

Design: `specs/2026-11-14-rust-rewrite-design.md`. WPF history: `wpf/ROADMAP.md` (frozen).
Status: `✅ done, actual` / `🔄 in progress` / `⬜ next` / `⬜ waiting on <x>`.

| Phase | Scope | Owner | Estimate | Status |
|---|---|---|---|---|
| R0 Setup | Docs layout, ADRs, design doc, workspace, lints, structure check, CI | master | 1 session | ✅ done, 1 session (CI green on 0155a6d) |
| R1 Engine slice | Empty island, spring morph, frame clock, hit testing, motion log, demo driver; measured on console at 240 Hz | lane `engine` | 4 to 6 sessions | ✅ done, 1 lane session (merged 1aabe4a; ADR 0003 to 0006) |
| R2 Services | `plith-audio`, `plith-media`, `plith-system`, headless, each with a hardware probe | lane `services` | 3 to 5 sessions | ✅ done, 1 lane session (merged 4b50e83; Voicemeeter and WMI unverified on hardware) |
| R1b Engine under load | Missed vblanks while a game runs behind, collapse and page durations, topmost IME skip | lane `engine` | 1 to 2 sessions | ✅ done, 1 lane session + console run (merged e20201c; ADR 0008) |
| R3 Visual direction | Resting pill, tab bar, four pages, type, icons (praxvon-direction, impeccable) | master + Berke | 2 sessions | ✅ done, 1 session (Ray Paneli, ADR 0007, mock in `design/r3/`) |
| R4 Pages | Tab bar, audio, media, system on the engine | lane `pages` | 4 sessions | 🔄 code merged (b6bd801, 1 lane session + 1 merge fix round; ADR 0010 to 0012); console run left |
| R4b Media output picker | Per-app output routing (undocumented IAudioPolicyConfig); R4 shows the route read-only | ⬜ | 1 session | ⬜ waiting on R4 |
| R5 Shelf | In-process OLE drag in and out | ⬜ | 2 sessions | ⬜ waiting on R4 |
| R6 OSD helper | UIAccess band window, pipe, signing, flyout hardening | lane `osd` | 2 to 3 sessions | 🔄 code merged (4aca03e, 1 lane session + 2 review rounds; ADR 0009); console run over a game left |
| R7a Karot idle look | Pill looks, welcome view, settings and play files, tr/en table, weather service | master + subagents | not estimated | 🔄 code merged (3ee8b05, 2 sessions incl. direction pass; ADR 0014); console run left |
| R7 languages | Eight languages from one table, OSD muted word over the pipe | master + subagents | not estimated | ✅ done (merged 3a281c2; ADR 0015; console 2026-11-18) |
| Console fix round | Home mark, speaker glyph, title scroll, ruler drag, OSD prewarm (from the 2026-11-18 console) | master + subagents | not estimated | ✅ done, 2 sessions incl. console check (merged 397e7d9; ADR 0016) |
| R7 Settings window | Genel, Boşta, OSD, Hakkında window; quick page behind a gear; live apply and atomic save | master + subagents | not estimated | 🔄 code merged (c7e693a, 1 session: brainstorm, spec, plan, 10 tasks, final review; ADR 0017); console run left |
| R7 Parity | Settings, tray, installer, parity check against WPF 0.3.1, WPF removed | master | 3 sessions | ⬜ waiting on R4 to R6 |

## Settled scope (2026-11-14)
- Inside exclusive fullscreen games only the OSD shows (ADR 0002).
- The accent picker is dropped; four fixed colour themes instead (ADR 0007). Weather came back
  with the idle look, off by default (ADR 0014, Berke 2026-11-17).
