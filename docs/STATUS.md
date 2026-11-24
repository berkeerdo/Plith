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
- **Running:** no lane (`LANES.md` retired to `journal/2026-11.md` on 2026-11-19).
- Engine: frame latency 2 (ADR 0008), tune `brisk`. R3 direction Ray Paneli
  (`specs/2026-11-14-r3-visual-direction.md`, mock `design/r3/ray-paneli.html`, ADR 0007).
- Label rule (Berke 2026-11-16): Turkish UI strings upper-case with Turkish rules; names from
  Windows or Voicemeeter with invariant rules ("DISPLAY", never "DİSPLAY").
- **R7a Karot idle look:** merged as `3ee8b05` (ADR 0014, plan `plans/active/idle-look-r7.md`).
  Six resting pill looks (Karot default), the welcome view as a fifth page, `settings.ini` and
  `plays.txt` in `%LOCALAPPDATA%\Plith\`, `plith-weather` (off by default). Task 12 boxes 1 and
  2 proven on the console; box 3 (weather to `OKUMA`) open. Worktree removed 2026-11-20.
- **Theme switch: merged as `104e904`** (2026-11-24, spec `specs/2026-11-24-theme-switch-design.md`,
  plan `plans/active/theme-switch.md`). Görünüm section, Tema row (Gece, Kağıt, York, Sodyum), key
  `theme`, `--theme`; caption follows dark or light. OSD helper keeps fixed colours.
- **R5 shelf: merged as `e3aa5da`** (spec `specs/2026-11-24-r5-shelf-design.md`, ADR 0018, plan
  `plans/active/r5-shelf.md`, opus review: 5 important, all fixed).
- **R4b output picker: parked** on worktree branch `worktree-agent-afd1c836624792461` (spec
  `specs/2026-11-24-r4b-media-output-picker-design.md`, research `research/2026-11-24-r4b-app-route.md`).
  Probe over RDP: activation and Get OK, Set to `Remote Audio` gave `E_INVALIDARG`. Rerun with real
  outputs (console, or RDP with sound "On the remote PC") before any UI work; UI code there is
  written but never compiled.
- **R7 settings window: merged as `c7e693a`** (2026-11-20; spec, plan `plans/active/r7-settings.md`,
  ADR 0017, review in `quality/`). Idle check on the console: Karot 60 s, 0 render wakes.
- **Display relayout: merged as `42f29ac`** (2026-11-22; spec, plan
  `plans/active/display-relayout.md`, review in `quality/`). RDP reconnect 2026-11-23: 0 relayout
  lines (same size), 32 fps 0 hitches; the rest of Task 5 open (plan progress log).
- Session display fix merged 2026-11-23 (`ac07a42`): suppression follows the session display.
- **Owed on the console, Berke present** (any day): the relayout check (plan Task 5):
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
- 2026-11-24 later, all merged and pushed: shelf clean fix `6979c7a` (cross per wagon, `TEMİZLE`,
  `GERİ AL` only when an undo exists, sky word `BERRAK`); clock on the marks band `ea65414`; R7
  parity part 1 `ae94753` (spec `specs/2026-11-24-r7-parity-tray-design.md`, plan
  `plans/active/r7-parity-tray.md`): tray icon, exe icon and version 0.4.0, Genel row `Windows
  ile başlat` (HKCU Run `Plith`). Five checks: 1410 tests, structure 843/0, release. RDP: Berke
  saw the themes and dragged in and out ("çalışıyor"), the shelf file was emptied by his remove;
  `tray: icon added` logged. Tray menu, `Çıkış`, Explorer restart not yet seen.
- **Next: redesign of the logo/icon and the whole settings window** (Berke 2026-11-24: "logo
  iconunu ve komple ayarları güncelleriz"). Design work order: `praxvon-direction` concepts,
  Berke picks, build, `impeccable` pass. The current Genel subtitle no longer covers its rows.
- Then R7 parity part 2: installer and uninstall (helper in Program Files, trust store step,
  sudo with Berke's yes), WPF `config.ini` import. Research `research/2026-11-24-r7-parity.md`.
  Open: brightness hotkeys and endpoint pinning (WPF had both); asked Berke whether he uses them,
  his "evet öyle" was ambiguous: ask again before parity part 2.
- R4b waits for its probe with real outputs (console, or RDP sound "On the remote PC").
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
