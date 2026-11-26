# Status

Updated 2026-11-26 (late evening). Rewritten, not appended; old content goes to `journal/`.

## Now
- R0 to R3, R5, R7 languages, theme switch, settings window, identity r3 and the app icon are
  merged; history in `journal/2026-11.md`. R4, R6 and R7a are merged with console runs left.
- **R7 part 2 merged as `3a9cb89` and pushed** (2026-11-26). Correction: its CI run on main was
  red (setup register tests on the runner's 8.3 temp path; the dependency rules step never ran);
  both fixed in the theme packs merge `3b868f6`, PR #1 green on run 45826321975. Spec
  `specs/2026-11-25-r7-installer-design.md`, plan `plans/active/r7-installer.md`, ADR 0021.
  `plith-setup.exe`: A Halka window at medium, elevated worker over a pipe, signed payload
  (`plith.exe`, `osd\plith-osd.exe`, `setup\plith-setup.exe`), Apps entry, Start Menu link,
  certificate in LocalMachine Root and TrustedPublisher, uninstall; one-time WPF `config.ini`
  import; daily update check against GitHub `releases/latest` with a tray notice and an About
  row. Build: `scripts/build-setup.ps1` (never overwrites `release\Plith-Setup-<v>.exe`; older
  builds kept as `-pre1` to `-pre4`). Checks on `0e12f40`: 1604 tests passed, 1 ignored (live
  network), clippy, fmt, structure.
- **Installed on this PC from the setup** (2026-11-26 00:21, build `40A12AC9...`): Plith runs
  from `C:\Program Files\Plith`, the old WPF install is gone. Eight real runs and three bugs
  fixed: `quality/2026-11-25-setup-first-run.md`.
- A dev `plith.exe` from `target\` now trusts the installed helper from the setup build; after a
  protocol change rerun the setup (or `install-osd.ps1` under sudo, Berke's yes).
- Label rule (Berke 2026-11-16): Turkish UI strings upper-case with Turkish rules; names from
  Windows or Voicemeeter with invariant rules ("DISPLAY", never "DİSPLAY").
- Engine: frame latency 2 (ADR 0008), tune `brisk`. R3 direction Ray Paneli (ADR 0007).
- **Next: theme worlds and settings r2** (Berke 2026-11-26: "temalarımız kötü", the theme part
  must be made good; settings r2 mock "çok daha iyi"; comps r2 "güzel baya"). Every theme is a
  material world like Halka: Halka, Firn (glacier firn), Kablo (1866 Atlantic cable), Şerit
  (punched telegraph tape), plus plain Gece; York, Sodyum, Kağıt retire. Text must read on
  every material: per-theme ink colours, a quiet text zone, contrast tested (ink 7, muted 4.5,
  faint and flow 3, measured per pixel, not on the mean). Themes are data packs so a later
  theme designer and marketplace fit on top. Settings r2 brings back Görünüm as a visual
  theme picker (cards with a mini island; overrides the old no-cards anti-goal). Comps r2 and
  palettes: artifact `PPgArGSRbE82QB6q3ZdytY`; page, rasters and contrast
  script in `docs/design/themes/r2/` (the script checks the floor mean only; the per-pixel
  check is still owed). Settings r2 mock: artifact `UQpu8w5MWVvn9diUWpZUjs`, not in the repo.
  **Spec approved** (Berke 2026-11-26): `specs/2026-11-26-theme-worlds-design.md`, ADR 0022
  (supersedes 0007). **Plans 1 and 2 merged as `3b868f6`** (2026-11-26; PR #1 green). Plan 1
  (`plans/done/theme-packs.md`): packs, colour solve, accent and floor keys, retired themes
  migrate to Gece colours. Plan 2 (`plans/active/halka-island.md`, Tasks 1 to 6 done): Halka
  is a built-in pack (lime fabric open, glossy black pill collapsed, `collapsed.flow.color`
  lime), `ThemePaint` (open, collapsed, art) to the render thread, painter draws the material
  over an opaque black base, the engine drives `MaterialMotion`, `motion` lines end with
  `; theme <id>`. Per-pixel contrast in the text zone (worst at 2x): ink 10.64, muted 6.15,
  faint 4.37, flow 12.55. 1747 workspace tests. Render set seen by Berke (artifact
  `A7kvbsAcEUZAMvUWPNbozY`); his pick 2026-11-26: ripples follow the morph progress (start at
  0.15 / 0.30 / 0.45, each over 0.15), not time (option A). Then (Berke: text did not show):
  page text waits until the ripples fade (60 ms), idle look leaves with the pill; the page comes
  162.5 ms after Gece's (fake clock). Merged as `b72bdb7` (PR #2 green). Rulings of the run:
  ledger summary in `journal/2026-11.md`.
- **Next: plan 2 Task 7, console measurement with Berke** (release build, `--theme halka` and
  `gece`, expand and collapse audio, media, welcome ten times each, live switch while open;
  check the UI-thread art decode on a switch to Halka, the ring-clip frames' draw ms, how the
  162 ms page delay feels, a collapse gap with neither page nor idle look). Then
  theme worlds plan 3 (Firn, Kablo, Şerit, settings r2, Sıfırla for migrated colours). No
  release before plan 3.
- Release held until parity and the theme worlds (Berke asked 2026-11-26). After plan 3: R7
  parity close (vs WPF 0.3.1, WPF out of `src/`), then public 0.4.0 (0.4.1 tests the update).
- **New idea (Berke 2026-11-24), not started:** in the Hidden idle look, hover slides down a
  small strip with info; every idle look gets a choice of what its details show. Brainstorm first.
- **R4b output picker: parked** on branch `worktree-agent-afd1c836624792461`: probe Set to
  `Remote Audio` gave `E_INVALIDARG` over RDP; rerun with real outputs before any UI work.
- **Owed on the console, Berke present** (any day): the relayout check (plan
  `display-relayout.md` Task 5: resolution and scale, RDP to console switch, resize over RDP,
  display-off hide); the settings console session (plan `r7-settings.md` Task 12, Task 11
  Step 1: gear click, every row live, German and back, city and a real fetch, OSD off and on,
  click-to-window delay, caret); identity r3 Task 5 Step 2 (title bar and taskbar icon). Known
  open: the quick page Playing preview title can vanish during its one-shot scroll; preview
  text is about 5 px at 1x.
- Still owed on screen, another day: weather on to `OKUMA`; osd Task 9 over a game (`--flyout
  hide`); 20 s ambient on the audio and media page (pages Task 18 Step 3) and the square-top look
  (Task 7a Step 5); audio-clarity Task 8 box; an Explorer restart with the tray icon; the tray
  icon look. Later: the guarded route write session.
- Console how-to: `query session` first; release `plith.exe` is a GUI exe, so start it with
  `Start-Process -Wait`; close it with `WM_CLOSE` to class `Plith.Notch` (clean `exit 0`).

## Open risks
- Update from WPF 0.3.1: its updater downloads `Plith-Setup-*` and starts it elevated
  (`runas`), so every old user enters the setup's unmeasured elevated-start relaunch (C2, N1).
  Test it once (install 0.3.1, press update) before the first release.
- Setup links: tests expand 8.3 temp paths; production `register` / `uninstall` link-target
  compares are unchecked for a user with a short profile path.
- Halka: art decodes on the UI thread at start, on a theme switch and on a scale-class change
  (unmeasured); `gece-base.png` render fixture passes on CI's WARP today but is exact to 2 levels.
- Setup, unmeasured: a standard (non-admin) user (the UIAccess helper's level; the client trusts
  only a server strictly above it, so it may fail closed); the elevated-start relaunch (C2) and
  the UAC owner window; the update path past the check (download, signature, update screen,
  restart) until a newer release exists.
- Setup trust: the self-signed certificate goes into LocalMachine Root, so its private key can
  sign anything this PC (and every user's) will trust. It lives only in this PC's CurrentUser
  store; losing or leaking it is the risk. A real code-signing certificate replaces it before a
  wide release.
- Setup, parked before public release: N1 a "Run as administrator" compatibility flag on the
  setup makes the medium relaunch loop (fix: a marker argument); N2 SmartScreen blocking the
  relaunch fails silently (log only); N3 harmless. Deferred review minors:
  `quality/2026-11-26-r7-installer-deferred-minors.md`.
- Relayout unmeasured on a real display change (vblank wait on an output RDP removed, rebuild on
  the RDP to console switch, the flash of a rebuild). Review minors in
  `quality/2026-11-22-display-relayout-final-review.md`.
- First OSD show of a run: 1 hitch with present 11.6 ms after the prewarm (2026-11-19); root
  cause measured with osd Task 9.
- Not measured: what reaches the display at latency 2; the game cost of anything.
- Brightness flyout suppression (G5) is unverified and may hide Quick Settings while its slider
  moves: keep it off until measured on a laptop.
- A normal run wakes about 3 times a second on background threads (OSD supervisor, link worker);
  give-up lasts the whole session. Minors: `quality/2026-11-17-osd-wiring-final-review.md`.
- Residuals, low: pipe name squattable by another user (random `--pipe` per launch is the fix);
  a junction swapped on the log folder can leave an empty file.
- Collapse full 812 ms against Oyenza's 658 ms; one collapse hitch (8.3 ms) in most manual runs.
- Services contract for `crates/plith`: `start()` blocks, call it off the UI thread; subscribe
  first, then `snapshot()`; `set_waker` posts one WM_APP message.
- Unverified on hardware: Voicemeeter (not installed here), WMI brightness, a second monitor.
- R7a unverified: the Windows location path (IP via `ipapi.co` may be the real path), `lang=auto`
  when UI language and region differ, a real weather fetch through the app.
- Branches `feature/shelf-one-surface` and `feature/notch-in-loft` hold WPF work; not merged.

## Recent decisions
- ADR 0022 Themes are material worlds in data packs (supersedes 0007)
- ADR 0021 Plith installs with its own setup and an elevated worker
- ADR 0020 The settings window marks the selected section in the theme's flow colour
- ADR 0019 The app icon is a lime speaker-fabric tile with a black source
- ADR 0018 A file drag over the island opens the shelf
