# Status

Updated 2026-11-16. Rewritten, not appended; old content goes to `journal/`.

## Now
- R0, R1, R1b, R2 and R3 are done. R6 code is merged as `4aca03e` (451 tests, five checks green
  on the merge); its console run over a game (plan Task 9) is the last R6 step.
- R6: `plith-osd.exe` draws in a `CreateWindowInBand` window at band 2 (ADR 0009), installed and
  signed under `C:\Program Files\Plith\osd\`; `plith-ipc` pipe with identity checks;
  `plith-system` hook containment, flyout journal, key repeat collapse. Plan and Record:
  `plans/active/osd-r6.md`; reviews `quality/2026-11-16-osd-r6-merge-review.md` and
  `quality/2026-11-16-osd-r6-recheck.md`. The helper is not wired into `crates/plith` yet.
- **Running:** lane `pages` (R4) on `feature/pages-r4`; state in `LANES.md`.
- Engine: frame latency 2 (ADR 0008), tune `brisk`. R3 direction Ray Paneli
  (`specs/2026-11-14-r3-visual-direction.md`, mock `design/r3/ray-paneli.html`, ADR 0007).
- Label rule (Berke 2026-11-16): Turkish UI strings upper-case with Turkish rules; names from
  Windows or Voicemeeter with invariant rules ("DISPLAY", never "DİSPLAY").
- **Next after R4 merges:** the wiring job (helper and pipe into `crates/plith`, notes in
  `crates/plith-ipc/README.md` "Wiring" and the osd Record), then one console session with Berke
  for every owed on-screen check (osd Task 9, pages console run).

## Open risks
- Owed on screen (console, Berke present): osd Task 9 (visibility over borderless, exclusive and
  elevated windows, motion, device-loss rebuild M7, click-through M8, flyout Hide plus a kill
  needs his separate yes); pages idle check, square-top look, draw p99 with page bitmaps.
- Not measured: what reaches the display at latency 2; the game cost of anything.
- Standard (non-admin) user: the UIAccess helper's integrity level is unmeasured; the client
  trusts only a server strictly above its own level, so it may fail closed. An elevated notch
  fails closed too. Check before release (R7).
- Brightness flyout suppression (G5, review I3) is unverified (no built-in panel here) and may
  hide Quick Settings while its slider moves: keep it off until measured on a laptop.
- `crates/plith` must stay single-instance before flyout Hide is wired.
- Residuals, low: pipe name squattable by another user (SID is not secret; random `--pipe` per
  launch is the fix); a junction swapped on the log folder can leave an empty file.
- Collapse full 812 ms against Oyenza's 658 ms. One game, one PC, one refresh rate.
- Services contract for `crates/plith`: `start()` blocks, call it off the UI thread; subscribe
  first, then `snapshot()`; `set_waker` posts one WM_APP message.
- Unverified on hardware: Voicemeeter (not installed here), WMI brightness, a second monitor.
- Branches `feature/shelf-one-surface` and `feature/notch-in-loft` hold WPF work; not merged.

## Recent decisions
- ADR 0009 OSD helper draws in a window created in the UIAccess band
- ADR 0008 Frame latency 2, no priority levers
- ADR 0007 Themes are palettes over one visual language
- ADR 0006 Scene handoff between threads
- ADR 0005 Swap chain (latency line superseded by 0008)
