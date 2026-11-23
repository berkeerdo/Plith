# Status

Updated 2026-11-22. Rewritten, not appended; old content goes to `journal/`.

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
- **R7 settings window: merged as `c7e693a`** (2026-11-20). Spec
  `specs/2026-11-20-r7-settings-window-design.md`, plan and progress log
  `plans/active/r7-settings.md` (Tasks 1 to 10 ticked), ADR 0017, review
  `quality/2026-11-20-settings-window-final-review.md`. Window with Genel, Boşta, OSD, Hakkında
  on its own render thread; quick page behind a gear; live apply, atomic save, new keys `osd`
  and `flyout`. Five checks on the merge: fmt, clippy, 1237 tests, structure 0, release.
  Idle check 2026-11-20 console: Karot 60 s, 0 render wakes (allowed 2), 93.8 ms CPU.
  Worktree and branch removed.
- **Display relayout: merged as `42f29ac`** (2026-11-22). Spec
  `specs/2026-11-22-display-relayout-design.md`, plan and progress log
  `plans/active/display-relayout.md` (Tasks 1 to 4 ticked), review
  `quality/2026-11-22-display-relayout-final-review.md` (1 important, fixed in `51b0b30`).
  The shell re-reads the primary monitor, moves the window and rebuilds the hit region; the
  render thread rebuilds the stack (recover path, WARP on failure) and reopens the clock on a
  new monitor, rate or remote state. Five checks on the branch head (merge tree identical):
  fmt, clippy, 1262 tests, structure 0, release. RDP reconnect seen 2026-11-23: 0 relayout
  lines (same size), motion 32 fps 0 hitches; the rest of Task 5 open (plan progress log).
- **Fix `fix/session-display-state` (`ac07a42`, unmerged):** suppression follows the session
  display, not the console one (froze the notch over RDP). Merge after the office VM suite.
- **Next: one console session, Berke present.** First the relayout check (plan Task 5):
  resolution and scale on the console, the RDP to console switch, and a window resize over
  RDP. Check the display-off hide still works on the console (monitor off). Then finish the settings console session (started 2026-11-21, a real click opened the
  window in 39 ms; progress note in the plan; plan Task 12, Task 11 Step 1):
  open from the gear with a real click, every row live, German and back, city and a real fetch,
  OSD off and on (one `osd start` per switch, no `exited` line), click-to-window delay, caret.
  Known open: on the quick page the Playing preview title can vanish while its one-shot scroll
  runs (one line in `quick_facts`); preview text is about 5 px at 1x.
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
- Relayout unmeasured on a real display change. Unknown: a DXGI vblank wait on an output RDP
  removed, the hardware rebuild on the RDP to console switch, the flash of a rebuild. Review
  minors: an hz change keeps the old ambient divisor and stats rate until the run ends; an
  early `DisplayChanged` before `attach_relayout` is dropped; coalescing merges only queued
  messages (same-size RDP reconnect: 0 lines); the settings window keeps the start monitor
  handle; two relayout tests assert an origin that is equal by design.
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
