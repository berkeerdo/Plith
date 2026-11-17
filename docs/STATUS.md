# Status

Updated 2026-11-17. Rewritten, not appended; old content goes to `journal/`.

## Now
- R0, R1, R1b, R2 and R3 are done. R6 code is merged as `4aca03e`; its console run over a game
  (plan Task 9) is the last R6 step.
- R4 code is merged as `b6bd801` (786 tests, five checks green on the trial merge). Tab bar,
  audio, media, system, shelf and pill pages as display lists drawn from cached page bitmaps
  with prewarm (ADR 0010), click to expand and leave to collapse (ADR 0011), upper-case rules
  (ADR 0012). Plan and progress log: `plans/active/pages-r4.md`; review
  `quality/2026-11-16-pages-r4-merge-review.md` (3 Important fixed, re-reviewed). Nothing of R4
  has been on the real screen yet. Lane `pages` stopped, worktree kept for the console run.
- R6: `plith-osd.exe` draws in a `CreateWindowInBand` window at band 2 (ADR 0009), installed and
  signed under `C:\Program Files\Plith\osd\`; `plith-ipc` pipe with identity checks;
  `plith-system` hook containment, flyout journal, key repeat collapse. Plan and Record:
  `plans/active/osd-r6.md`; reviews `quality/2026-11-16-osd-r6-merge-review.md` and
  `quality/2026-11-16-osd-r6-recheck.md`.
- Wiring (2026-11-17, branch `feature/osd-wiring`, plan and Record `plans/active/wiring.md`,
  spec `specs/2026-11-17-osd-wiring-design.md`): `plith.exe` is single-instance, starts and
  supervises the installed helper, sends `Level` on volume, mute and brightness changes,
  `--flyout off|dry|hide` (default off), M8 done (OSD on `TextSystem`, bundled Martian Mono),
  release build has no console window. Helper reinstalled signed 2026-11-17 (Berke's yes);
  link connect, hello and bye measured on this PC. Final review clean after one fix wave.
- **Running:** no lane. State in `LANES.md`.
- Engine: frame latency 2 (ADR 0008), tune `brisk`. R3 direction Ray Paneli
  (`specs/2026-11-14-r3-visual-direction.md`, mock `design/r3/ray-paneli.html`, ADR 0007).
- Label rule (Berke 2026-11-16): Turkish UI strings upper-case with Turkish rules; names from
  Windows or Voicemeeter with invariant rules ("DISPLAY", never "DİSPLAY").
- **Next:** merge `feature/osd-wiring` (Berke's yes). Then one console
  session with Berke for every owed on-screen check: osd Task 9 (now through the real notch,
  `--flyout hide` for suppression); OSD look with the new type (number 10 DIP regular, ruler
  136 DIP); pages Task 4, 7a, 16 and 18
  on-screen steps (`--idle-check 60 --fixture banana`, `--demo pages --exit-after-demo
  --fixture banana`, 20 s media ambient while playing, hover and leave, `prewarm:` lines).
  Later and separate: the guarded route write session (one route on Banana and back).
- Console run 2026-11-16 (record in `plans/active/pages-r4.md`): idle checks 0 wakes; pages demo
  18 motions at 240 fps, 0 hitches, draw p99 at most 0.76 ms; manual run one page turn with 2
  hitches (present 4.1 ms). Still open: media `ambient` while playing, square-top look.
- **After wiring (Berke 2026-11-17):** audio page clarity job. The no-Voicemeeter view reads as
  "Logitech G733 62" with no label, unit or bar (Berke: "anlaşılmıyor hiç ne olduğunu"). Give
  each output an icon, a bar and a percent and a clear heading; keep route lines for Voicemeeter.
  Design choice is Berke's (`praxvon-direction`). Oyenza reference:
  `research/2026-11-17-oyenza-settings-and-hud.md`.
- R7 settings: two layers like Oyenza (quick settings in the island, a full window), idle look and
  OSD toggles there; the four-tab rule stays. Berke 2026-11-17 on Oyenza's Appearance page (theme
  cards with a live preview, idle look Clock/Eyes/Minimal/Hidden,
  `research/img/2026-11-17-oyenza-appearance-settings.png`): "bizimki daha iyi olmalı". Idle
  looks need a direction pass (`praxvon-direction`) before R7. Berke 2026-11-17: the WPF app
  had a welcome view (clock, detailed animated weather, the playing song in small type); "yeni
  yapacağımızda da benzer yaklaşım yapabiliriz, sadece düzenleme seçeneğimiz olur ayarlarda".
  Fold it into that direction pass; weather needs a new service (source, location). Then R5 shelf, R4b media output picker; themes
  switch in settings (palettes exist, nothing calls `set_palette`).

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
- A normal run wakes about 3 times a second on background threads (OSD supervisor 1 Hz, link
  worker 1 to 2 Hz, also after give-up); never on the UI or render thread, invisible to the idle
  check. Give-up lasts the whole session (a failure burst at logon leaves no OSD). Deferred
  minors and the final review: `quality/2026-11-17-osd-wiring-final-review.md`.
- A dev `plith.exe` trusts only the installed helper: after a protocol change, rebuild and
  reinstall the helper (`build-osd.ps1`, `install-osd.ps1` under sudo, Berke's yes).
- Residuals, low: pipe name squattable by another user (SID is not secret; random `--pipe` per
  launch is the fix); a junction swapped on the log folder can leave an empty file.
- Collapse full 812 ms against Oyenza's 658 ms. One game, one PC, one refresh rate.
- Services contract for `crates/plith`: `start()` blocks, call it off the UI thread; subscribe
  first, then `snapshot()`; `set_waker` posts one WM_APP message.
- Unverified on hardware: Voicemeeter (not installed here), WMI brightness, a second monitor.
- Branches `feature/shelf-one-surface` and `feature/notch-in-loft` hold WPF work; not merged.

## Recent decisions
- ADR 0012 Upper case: Turkish rules for our copy, invariant for foreign names
- ADR 0011 Click to expand, leave to collapse, no keyboard
- ADR 0010 UI layer: display list, cached page bitmaps, prewarm
- ADR 0009 OSD helper draws in a window created in the UIAccess band
- ADR 0008 Frame latency 2, no priority levers
