# Status

Updated 2026-11-20. Rewritten, not appended; old content goes to `journal/`.

## Now
- R0, R1, R1b, R2 and R3 are done. R6 code is merged as `4aca03e`; its console run over a game
  (plan Task 9) is the last R6 step.
- R4 code is merged as `b6bd801` (786 tests, five checks green on the trial merge). Tab bar,
  audio, media, system, shelf and pill pages as display lists drawn from cached page bitmaps
  with prewarm (ADR 0010), click to expand and leave to collapse (ADR 0011), upper-case rules
  (ADR 0012). Plan and progress log: `plans/active/pages-r4.md`; review
  `quality/2026-11-16-pages-r4-merge-review.md`. Lane `pages` stopped, worktree kept.
- R6: `plith-osd.exe` draws in a `CreateWindowInBand` window at band 2 (ADR 0009), installed and
  signed under `C:\Program Files\Plith\osd\`; `plith-ipc` pipe with identity checks;
  `plith-system` hook containment, flyout journal, key repeat collapse. Plan and Record:
  `plans/active/osd-r6.md`; reviews `quality/2026-11-16-osd-r6-merge-review.md` and
  `quality/2026-11-16-osd-r6-recheck.md`.
- Wiring merged as `3669f39` (plan and Record `plans/done/wiring.md`): `plith.exe` is
  single-instance, starts and supervises the installed helper, sends `Level` on volume, mute and
  brightness changes, `--flyout off|dry|hide` (default off), release build has no console window.
- **Running:** no lane (`LANES.md` retired to `journal/2026-11.md` on 2026-11-19).
- Engine: frame latency 2 (ADR 0008), tune `brisk`. R3 direction Ray Paneli
  (`specs/2026-11-14-r3-visual-direction.md`, mock `design/r3/ray-paneli.html`, ADR 0007).
- Label rule (Berke 2026-11-16): Turkish UI strings upper-case with Turkish rules; names from
  Windows or Voicemeeter with invariant rules ("DISPLAY", never "DİSPLAY").
- **R7a Karot idle look:** merged as `3ee8b05` (ADR 0014, plan `plans/active/idle-look-r7.md`).
  Six resting pill looks (Karot default), the welcome view as a fifth page, `settings.ini` and
  `plays.txt` in `%LOCALAPPDATA%\Plith\`, `plith-weather` (off by default). Task 12 boxes 1 and
  2 proven on the console; box 3 (weather to `OKUMA`) open. Worktree removed 2026-11-20.
- **R7 languages:** merged as `3a281c2` (ADR 0015, plan `plans/active/r7-languages.md`). Eight
  languages, OSD protocol 2 sends the muted word; proven on the console 2026-11-18.
- **Console fix round: merged as `397e7d9`** (2026-11-20, plan and log
  `plans/done/console-fix-round.md`): home mark (ADR 0016), speaker glyph level with the caps
  (fixed in the console check, `ec86288`), one-shot title scroll, ruler drag, OSD prewarm. Five
  checks on the merge, 1113 tests. Console 2026-11-19: Berke "çalışıyor hepsi"; Karot idle 60 s
  1 wake (allowed 2). Worktree removed. Earlier console sessions: `journal/2026-11.md`.
- **Next: the R7 settings window** (two layers like Oyenza; language choice with a live switch;
  idle look, weather, location and OSD toggles; gives `WeatherService::reconfigure` its caller).
  New feature: brainstorming, spec, plan first. Background of the R7 direction:
  `journal/2026-11.md`.
- Still owed on screen, another day: weather on (his yes) to `OKUMA`; osd Task 9 over a game
  (`--flyout hide`), with the first-show present hitch below; 20 s ambient on the audio and
  media page while playing (pages Task 18 Step 3) and his words on the square-top look (Task 7a
  Step 5); audio-clarity Task 8 box (he used the ruler, "cetvel çalışıyor"). Later: the guarded
  route write session.
- Then R5 shelf, R4b media output picker; themes switch in settings (nothing calls
  `set_palette`).
- Console how-to: `query session` first; release `plith.exe` is a GUI exe, so start it with
  `Start-Process -Wait`; close it with `WM_CLOSE` to class `Plith.Notch` (clean `exit 0`).
- **Audio page clarity:** merged as `68bcdb0` (ADR 0013, plan `plans/active/audio-clarity.md`).
  Without Voicemeeter the page is an outputs view; on screen and a real volume click wait for the
  console (plan Task 8).

## Open risks
- First OSD show of a run: draw p99 0.77 ms after the prewarm, but 1 hitch with present 11.6 ms
  (2026-11-19). Ruled an open item; root cause measured with osd Task 9.
- Owed on screen (console, Berke present): osd Task 9 (visibility over borderless, exclusive and
  elevated windows, motion, device-loss rebuild M7, click-through M8, flyout Hide plus a kill
  needs his separate yes); pages square-top look, draw p99 with page bitmaps.
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
- Collapse full 812 ms against Oyenza's 658 ms; one collapse hitch (8.3 ms) shows up in most
  manual runs. One game, one PC, one refresh rate.
- Services contract for `crates/plith`: `start()` blocks, call it off the UI thread; subscribe
  first, then `snapshot()`; `set_waker` posts one WM_APP message.
- Unverified on hardware: Voicemeeter (not installed here), WMI brightness, a second monitor.
- Branches `feature/shelf-one-surface` and `feature/notch-in-loft` hold WPF work; not merged.
- R7a unverified: the Windows location path (an unpackaged app may always be denied, then IP via
  `ipapi.co` is the real path), `lang=auto` on a machine whose UI language differs from its
  region (here auto picks English: the OSD sent `MUTED`), a real weather fetch through the app.
  Deferred minors: unknown settings keys logged raw, `plays.txt` written on the UI thread
  (atomic), no hatch geometry tests.

## Recent decisions
- ADR 0016 The welcome view is reachable from the tab row
- ADR 0015 Eight languages from one exhaustive table, the OSD word over the pipe
- ADR 0014 Karot idle look: click opens welcome, settings and play files, weather sources
- ADR 0013 Windows outputs get their own view, not a one-route diagram
- ADR 0012 Upper case: Turkish rules for our copy, invariant for foreign names
