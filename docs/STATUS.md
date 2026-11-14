# Status

Updated 2026-11-15. Rewritten, not appended; old content goes to `journal/`.

## Now
- R0, R1, R1b, R2 and R3 are done. R1b merged as `e20201c` (288 tests); R3 is a mock, not code.
- Engine: frame latency 2 removes the game-load stall (90 of 90 motions clean with Deadlock
  behind, ADR 0008); tune `brisk` is the default; priority levers measured and rejected. Plan and
  handoff: `plans/done/engine-r1b.md`; reviews `quality/2026-11-14-engine-review.md` and
  `quality/2026-11-14-engine-r1b-review.md`.
- R3 direction: **Ray Paneli**, spec `specs/2026-11-14-r3-visual-direction.md`, mock
  `design/r3/ray-paneli.html` (open with `#themes` for the theme sheet), renders
  `design/r3/renders-6/`. Four themes ship (ADR 0007), default Gece.
- **Next:** R4 Pages on the engine (tab bar, audio, media, system in Ray Paneli) and R6 OSD
  helper. They own disjoint crates; the master decides lanes at the start (skill `parallel-lanes`).
  R4 opens with the UI layer shape (P5), review items L11 to L14 and a Scene that carries page
  content.

## Open risks
- Not measured on screen: what reaches the display at latency 2 (no DWM counters, PresentMon not
  run); the hit-region cover during collapses with a game behind (Berke's look owed, record in
  ADR 0004 or 0008); the game cost of anything (the island parks while a game is in front, so
  this moves to R4 and R6).
- Collapse full 812 ms against Oyenza's 658 ms (visible 346 ms).
- Panic paths, the 3 s close backstop and the forced exit run only in unit tests; Alt+F4 cannot
  reach the island (it never takes activation), a posted `WM_CLOSE` was tested instead.
- One game, one PC, one refresh rate; pen and touch untested.
- Services contract for `crates/plith`: `start()` blocks (brightness up to 15 s, media about
  8 s), call it off the UI thread; `subscribe()` never replays, so subscribe first, then
  `snapshot()`; `set_waker` posts one WM_APP message.
- G5: brightness flyout suppression needs `open_window` on `WmiMonitorBrightnessEvent` (R4).
- Review L8 and M7: hook procs have no `catch_unwind`, and forced exits skip cleanup, so a crash
  could leave the Windows volume flyout hidden. Fix before suppression ships on by default.
- Key auto-repeat: one held volume key gave 135 events; the OSD must collapse repeats.
- Unverified on hardware: Voicemeeter (not installed here), WMI brightness, a second monitor.
- R3 open items 1 to 6 in the spec (radius, Potato density, levels API, route writes).
- Branches `feature/shelf-one-surface` and `feature/notch-in-loft` hold WPF work; not merged.

## Recent decisions
- ADR 0008 Frame latency 2, no priority levers
- ADR 0007 Themes are palettes over one visual language
- ADR 0006 Scene handoff between threads
- ADR 0005 Swap chain (latency line superseded by 0008)
- ADR 0004 Hit testing
