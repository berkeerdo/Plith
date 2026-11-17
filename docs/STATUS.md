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
- Wiring merged as `3669f39` (2026-11-17, plan and Record `plans/done/wiring.md`,
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
- **R7a Karot idle look:** merged as `3ee8b05` (ADR 0014, spec
  `specs/2026-11-17-idle-look-direction.md`, plan and progress log `plans/active/idle-look-r7.md`).
  Six resting pill looks (Karot default), the welcome view as a fifth page without a tab (click
  opens it, hover probes an hour), `settings.ini` and `plays.txt` in `%LOCALAPPDATA%\Plith\`, tr/en
  text table, `plith-weather` (Open-Meteo over WinHTTP, off by default). Whole-branch review: 4
  Important and 9 Minor fixed in one wave, re-review approved; five checks on the branch head
  (1018 tests). Renders `design/r7/engine-2/` compared with the mock. Nothing on screen yet
  (plan Task 12). Worktree `idle-look` kept until the console run.
- **Next:** the console session with Berke, every owed on-screen check batched: R7a Task 12
  (`--idle-check 120 --idle-look karot` at most 3 wakes, click opens welcome, probe follows the
  pointer, pill looks, weather on with his yes until `OKUMA`); a volume key press reaching the OSD (`osd msg: level`,
  never measured from a real key yet), osd Task 9 (now through the real notch,
  `--flyout hide` for suppression); OSD look with the new type (number 10 DIP regular, ruler
  136 DIP); audio-clarity Task 8 (outputs view, one volume click, his yes); pages Task 4, 7a, 16 and 18
  on-screen steps (`--idle-check 60 --fixture banana`, `--demo pages --exit-after-demo
  --fixture banana`, 20 s media ambient while playing, hover and leave, `prewarm:` lines).
  Later and separate: the guarded route write session (one route on Banana and back).
- Console run 2026-11-16 (record in `plans/active/pages-r4.md`): idle checks 0 wakes; pages demo
  18 motions at 240 fps, 0 hitches, draw p99 at most 0.76 ms; manual run one page turn with 2
  hitches (present 4.1 ms). Still open: media `ambient` while playing, square-top look.
- **Audio page clarity:** merged as `68bcdb0` (ADR 0013, plan
  `plans/active/audio-clarity.md`). Without Voicemeeter the page is an outputs view: default
  output with glyph, `VARSAYILAN · KULAKLIK`, a volume ruler (click sets volume) and `%62`;
  other outputs under `DİĞER ÇIKIŞLAR`, click makes default; no Voicemeeter notice. Brightness
  shows `%70`. Offscreen renders reviewed, five checks green (858 tests), code review clean
  (3 minors, recorded in the ADR). On screen and a real
  volume click wait for the console (plan Task 8).
- After the console: the "R7 languages" plan (R4 `copy.rs` into the lang table, percent format,
  more languages, font fallback check), then the R7 settings window (two layers like Oyenza;
  idle look, weather, location and OSD toggles; gives `WeatherService::reconfigure` its caller).
  Then R5 shelf, R4b media output picker; themes switch in settings (nothing calls
  `set_palette`). Background of the R7 direction: `journal/2026-11.md`.

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
- R7a unverified: the Windows location path (an unpackaged app may always be denied, then IP via
  `ipapi.co` is the real path), `lang=auto` on a machine whose UI language differs from its
  region, a real weather fetch through the app. Deferred minors: unknown settings keys logged
  raw, `plays.txt` written on the UI thread (atomic), no hatch geometry tests.

## Recent decisions
- ADR 0014 Karot idle look: click opens welcome, settings and play files, weather sources
- ADR 0013 Windows outputs get their own view, not a one-route diagram
- ADR 0012 Upper case: Turkish rules for our copy, invariant for foreign names
- ADR 0011 Click to expand, leave to collapse, no keyboard
- ADR 0010 UI layer: display list, cached page bitmaps, prewarm
