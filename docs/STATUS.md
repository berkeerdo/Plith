# Status

Updated 2026-11-18. Rewritten, not appended; old content goes to `journal/`.

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
- **Running:** no lane (`LANES.md` retired to `journal/2026-11.md` on 2026-11-19).
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
  (1018 tests). Renders `design/r7/engine-2/` compared with the mock. Console 2026-11-18:
  Task 12 boxes 1 and 2 proven (120 s Karot idle 2 wakes, click opens welcome, probe, tabs);
  box 3 (weather to `OKUMA`) open. Worktree `idle-look` can go after the fix round.
- **R7 languages:** merged as `3a281c2` (ADR 0015, plan and log
  `plans/active/r7-languages.md`, 1049 tests, five checks green on the merge). Eight languages
  (`en tr de es fr pt it pl`), every own string from the table, percent per language, measured
  fit and glyph tests, renders `design/r7/lang-*/`; Turkish idle renders byte-identical. OSD
  protocol 2 sends the muted word; the client reads the link up only after an admission probe.
  Helper for protocol 2 reinstalled signed 2026-11-18 (Berke's yes, SHA-256 `C3BF2E9C...`); a
  normal run in the console logged `osd: helper launched` and `osd: link connected` 2 s later,
  clean exit 0. Console 2026-11-18: a real volume key reached the OSD (`level volume 64..100`),
  mute too, and `lang=de` sent `STUMM` (record in `plans/active/r7-languages.md`).
- **Console session 2026-11-18 done** (records in `idle-look-r7.md`, `r7-languages.md`,
  `pages-r4.md`): every motion 240 fps but three in a real-service run during his game; pages
  demo 21 motions 0 hitches; fixture idle 60 s 1 wake; notch survives a display sleep and wake.
- **Next: the console fix round** (Berke's findings on screen; he said "önerinle devam",
  so the proposals below are decided). Spec `specs/2026-11-19-console-fix-round-design.md`,
  plan and log `plans/active/console-fix-round.md` (approved 2026-11-19, subagent route):
  1. Welcome has no way back after a tab turn: a home mark at the left of the tab row, a new
     ADR amending 0014.
  2. Karot pill right `%62` alone does not read as volume: a speaker glyph before it.
  3. Long playing title in the idle pill: scroll left once on a track change and on hover,
     then rest at the start with a fade edge (never a looping marquee: idle budget).
  4. Audio volume ruler and media timeline: a smooth drag, not only a click.
  5. The first OSD show of each run hitches (cold draw 5.9 and 9.6 ms): prewarm it.
- Still owed on screen, another day: weather on (his yes) to `OKUMA`; osd Task 9 over a game
  (`--flyout hide`); 20 s ambient on the audio and media page while playing (pages Task 18
  Step 3) and his words on the square-top look (Task 7a Step 5); audio-clarity Task 8 box
  (he used the ruler, "cetvel çalışıyor"). Later: the guarded route write session.
- Console how-to: `query session` first; release `plith.exe` is a GUI exe, so start it with
  `Start-Process -Wait`; close it with `WM_CLOSE` to class `Plith.Notch` (clean `exit 0`).
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
- After the console: the R7 settings window (two layers like Oyenza; language choice with a
  live switch;
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
- ADR 0015 Eight languages from one exhaustive table, the OSD word over the pipe
- ADR 0014 Karot idle look: click opens welcome, settings and play files, weather sources
- ADR 0013 Windows outputs get their own view, not a one-route diagram
- ADR 0012 Upper case: Turkish rules for our copy, invariant for foreign names
- ADR 0011 Click to expand, leave to collapse, no keyboard
