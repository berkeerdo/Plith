# Status

Updated 2026-11-14. Rewritten, not appended; old content goes to `journal/`.

## Now
- R0, R1 and R2 are done and merged (`1aabe4a`): engine slice and headless services, 235 tests.
  Console session with Berke 2026-11-14: "aşırı akıcıydı", 240 fps and 0 hitches without a game,
  0 render wake-ups at rest, parks on lock, display off and fullscreen. Decisions: ADR 0003 frame
  clock (DComp), 0004 hit testing (region 3 px outside the shape), 0005 swap chain, 0006 scene
  handoff. UI layer shape (P5) is decided at the start of R4.
- Reviews before merge: `quality/2026-11-14-services-review.md` and
  `quality/2026-11-14-engine-review.md`, 0 blocking, 24 later items between them.
- **Next:** R1b in lane `engine` (missed vblanks with a game behind, durations, topmost IME
  skip), and R3 visual direction with Berke in the master.

## Open risks
- G1 under game load: with Deadlock behind, 13 of 25 motions missed 1 or 2 vblanks (worst 8.3 ms);
  collapse 812 ms and page turns up to 775 ms, slower than Oyenza (660 ms). R1b.
- Engine review: the render thread has no `catch_unwind` (a panic freezes the island while the
  idle check still passes); GPU recovery never run on hardware; R4 needs a generic wake message
  for `set_waker` and a Scene that carries page content.
- Not tested: pen and touch, a second borderless game, the split and mask hit modes (dropped).
- Two `main` commit subjects are 52 and 53 chars (5887b06, 36ee2b3); left as pushed.
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
