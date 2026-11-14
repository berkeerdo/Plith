# Status

Updated 2026-11-14. Rewritten, not appended; old content goes to `journal/`.

## Now
- Phase: **R1 and R2 in lanes, R3 next in the master.** Plith is being rewritten from scratch in
  Rust (ADR 0001). WPF 0.3.1 is the frozen working reference.
- R0 done: docs layout, ADR 0001 and 0002, design doc
  `specs/2026-11-14-rust-rewrite-design.md` (approved by Berke 2026-11-14), Cargo workspace with
  nine crates, `scripts/check-deps.mjs`, CI `.github/workflows/rust.yml` (green on 0155a6d).
- R2 done and merged (`4b50e83`): audio, media, system crates, 105 tests, hardware writes
  verified except Voicemeeter (not installed) and WMI brightness (desktop). Pre-merge review:
  `quality/2026-11-14-services-review.md`, 0 blocking, 10 later items for R4 wiring.
- Lane `engine` (R1) is in Task 10, the console session with Berke.
- **Next (master):** R3 visual direction with Berke (praxvon-direction), starting from the Loft
  palette on branch `feature/notch-in-loft`.

## Open risks
- Services contract notes for `crates/plith` (from lane services, 2026-11-14): every service has
  new/start/stop/status/snapshot/subscribe/set_waker; `start()` blocks (brightness up to 15 s,
  media about 8 s), so call it off the UI thread; `set_waker` posts one WM_APP message;
  `subscribe()` never replays, so subscribe first, then `snapshot()`.
- G5 gap: brightness flyout suppression needs `crates/plith` to call `open_window` on
  `WmiMonitorBrightnessEvent` (R4).
- Unverified on hardware: Voicemeeter (not installed here), WMI brightness (desktop), a second
  monitor's DDC mapping.
- Review L8: the keyboard and WinEvent hook procs have no `catch_unwind`; a panic aborts and leaves
  the shell volume flyout hidden. Fix before R4 ships suppression on by default.
- Key auto-repeat: one held volume key gave 135 events; the OSD must collapse repeats (R4).
- Branch `feature/shelf-one-surface` (worktree `.claude/worktrees/shelf-one-surface`) holds WPF
  shelf work the rewrite makes obsolete; it is not merged.
- Branch `feature/notch-in-loft` (worktree `C:\Projects\plith-notch-loft`) holds the Loft palette
  that R3 starts from.

## Recent decisions
- ADR 0001 Rewrite in Rust, in this repo
- ADR 0002 Medium-integrity notch, UIAccess OSD helper
