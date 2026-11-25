# Status

Updated 2026-11-25. Rewritten, not appended; old content goes to `journal/`.

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
  Probe over RDP: Get OK, Set to `Remote Audio` gave `E_INVALIDARG`; rerun with real outputs
  before any UI work (UI code there never compiled).
- Settings window `c7e693a` (plan `plans/active/r7-settings.md`) and display relayout `42f29ac`
  (plan `plans/active/display-relayout.md`, Task 5 open): records in `journal/2026-11.md`.
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
- R7 parity part 1 `ae94753`: tray menu, `Çıkış`, an Explorer restart not yet seen (journal).
- **App icon done (ADR 0019):** lime speaker-fabric tile, black source at the top edge, three
  tone-on-tone ripples; master `design/identity/icon/plith-master-1024.png`, `.ico` from
  `scripts/build-icon.ps1`. Release rebuilt, icon read back from the exe, `tray: icon added`
  (RDP, 19:09); tray look not yet confirmed ("kumaş kısmı anlaşılmıyor").
- **Karot pill weather fix:** right text no longer dropped by `CAP_RISE` (measured on a render);
  shows the weather now (`9° BULUTLU`), the turn only on the welcome view (Berke picked it).
- **Identity round 3: merged as `f95a805`** and pushed (spec
  `specs/2026-11-24-identity-r3-design.md`, plan `plans/active/identity-r3.md`, ADR 0020): About
  icon header, flow station, class icon; installer A Halka material approved ("evet şimdi oldu")
  in `design/identity/r3/installer/`. Owed: a real click on the gear to see the title bar and
  taskbar icon (plan Task 5 Step 2).
- **Next: R7 parity part 2** (`research/2026-11-24-r7-parity.md`): the Rust installer drawn as A
  Halka (`design/identity/r3/installer/README.md` has the layout), uninstall (helper in Program
  Files, trust store, sudo with his yes), `config.ini` import, and the update check (spec
  `specs/2026-11-25-update-check-design.md`: daily check, one tray notice per version, `KUR`
  verifies the Plith signature, then the installer's update screen). Asset name stays
  `Plith-Setup-<version>.exe`. Ask whether he uses brightness hotkeys and endpoint pinning.
- **New idea (Berke 2026-11-24), not started:** in the Hidden idle look, hover slides down a
  small strip with info; every idle look gets a choice of what its details show. Brainstorm first.
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
- ADR 0020 The settings window marks the selected section in the theme's flow colour
- ADR 0019 The app icon is a lime speaker-fabric tile with a black source
- ADR 0018 A file drag over the island opens the shelf
- ADR 0017 Settings live in two layers: a quick page in the island and a full window
- ADR 0016 The welcome view is reachable from the tab row
