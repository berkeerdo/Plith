# Changelog

All notable changes to Plith are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed
- **The OSD now hides itself automatically during fullscreen video playback,
  and this is on by default.** Plith detects a foreground window covering a
  monitor while it (or the system's active media session) is playing video,
  and hides the OSD for as long as that lasts — no more volume popup sitting
  on top of a movie or stream. This is **on by default** on upgrade; turn it
  off from Settings with the **"Hide during fullscreen video"** toggle
  (alongside a configurable app hide-list for players that should never be
  auto-hidden). **Games keep the OSD.** A game is only ever hidden behind if
  it is itself the app playing the media, which it never is; a true
  exclusive-fullscreen application is vetoed outright on top of that. Verified
  on real hardware with a game covering the screen and music playing in the
  background: the OSD appeared on every volume key and the auto-hide never
  engaged.
- Fullscreen video in a **packaged (Microsoft Store) player is not
  auto-hidden**. Windows reports those apps under an identifier that cannot be
  matched to their process, so Plith fails toward showing the OSD rather than
  guessing. Add the player to the hide-list if you want it covered. Desktop
  players such as VLC are detected normally.

### Accessibility
- Screen readers now announce meaningful names across the OSD and Settings
  window (media transport buttons, accent swatches, status indicators)
  instead of generic control types.
- The OSD's audio and media cards are exposed as live regions, so a value or
  track change is announced without the user needing to focus the OSD.
- Accent swatches are keyboard-reachable and show a visible focus ring and
  selection status.
- Focus indicators that had gone missing across Settings controls are
  restored.
- Added a Windows high-contrast palette for both the OSD and Settings, so the
  app switches away from its own colours when a contrast theme is active.

### Fixed
- **Holding or repeatedly pressing a volume key no longer makes the OSD
  flicker.** Volume keys repeat faster than the OSD's fade-in, and each repeat
  was restarting that fade from fully transparent, so the OSD pulsed instead
  of staying up. It now stays up for as long as the key does.
- **Enter now activates a focused accent swatch**, not just Space. The
  swatches are buttons, and a focused button in WPF ignores Enter unless it is
  asked not to.
- Screen readers no longer read out an internal type name for each OSD card,
  and the Settings scroll area is no longer an unnamed stop in the Tab order.

## [0.1.5] - 2026-09-02

### Fixed
- **Installer no longer appears to crash after the "Registering Plith"
  step.** The FinishPage subscribed its auto-launch to the WPF `Loaded`
  event, and `Loaded` fires BEFORE the first render pass. The immediate
  `Application.Shutdown()` inside `LaunchPlithAndExit` then tore the
  window down before FinishPage ever painted — from the user's side
  the installer just "disappeared" after Registering with no completion
  screen. The auto-launch is now deferred behind a 2.5 s
  `DispatcherTimer` so the Installed screen is visibly on-screen first,
  and the extra window also gives the child `Process.Start` time to
  complete before the parent tears down (which likely explains the
  reports of the auto-open-after-install occasionally not launching
  Plith at all).

### Changed
- **Installer buttons are now one consistent size across every page.**
  Each page (Welcome, Finish, Error, UninstallConfirm, UninstallFinish)
  had drifted its own MinHeight / MinWidth per button — 42 here, 36
  there, 120 on one CTA, 100 next to it, some ghost buttons unsized
  entirely so "View on GitHub" and "Close" rendered at different
  widths just because their labels differed. `AccentButtonStyle` and
  `GhostButtonStyle` now bake in MinHeight (42 / 36) and MinWidth
  (140 / 110); every per-page override was removed. A row of ghost
  buttons — Copy log + Open log, View on GitHub + Close, Uninstall +
  Cancel — is now one uniform shape regardless of label length.
- **Release artifact is a single-file .exe again** (`Plith-Setup-0.1.5.exe`,
  ~40 MB). 0.1.4 shipped as a folder+zip after Norton's SONAR / Download
  Insight engines killed the self-extract mid-init on the dev machine.
  The dev machine is now on Windows Defender, which does not sandbox this
  path; the in-app updater and the landing site's release fetcher have
  always looked for `Plith-Setup-*.exe`, so this restoration also unbreaks
  the auto-update flow that the zip pivot had silently broken. If
  Norton-heavy users report install crashes down the line, we will add a
  zip fallback as a second asset alongside the .exe.
- **Settings buttons now share one visual language.** `Set position`,
  `Check for updates`, `Release notes`, and `Download and install`
  were rendering as raw WPF default buttons because they never had a
  `Style` applied — they broke visual continuity with the rest of the
  Settings surface (accent-aware cards, ghost inputs, rounded 7px
  corners). All four now use the existing `GhostButtonStyle` /
  `PrimaryButtonStyle` from `SettingsTheme.xaml`, so hover, press,
  focus, and accent-recolor states behave the same as every other
  action in the window. `Download and install` is the only Primary
  (accent-filled) button of the group — it is the actual CTA when an
  update exists — everything else is Ghost.
- **Updates card is now responsive at narrow widths.** The old layout
  put the progress bar and both action buttons in one horizontal Grid
  row; at the Settings window's minimum column width the buttons
  crowded and the progress bar shrank to a stub. The card now stacks
  the progress bar full-width on top with the action buttons wrapping
  right-aligned underneath (`WrapPanel`), so `Download and install`
  stays fully readable even when the Settings window is resized
  toward its minimum.
- **New `ModernProgressBarStyle`.** Thin (4 px) accent-filled bar on
  a muted ghost track with 2 px rounded corners. Replaces the raw 6 px
  system ProgressBar in the Updates card. Track and fill both recolor
  live with the current accent, matching the sliders above.

### Fixed
- **Theme Studio picks now actually reach the OSD overlay.** The 0.1.4
  release wired the accent-override `ResourceDictionary` into
  `Application.Resources`, but the OSD is a `BandWindow` — a WPF
  `HwndSource` with a custom `RootVisual` — and its
  `DynamicResource` references do not receive change notifications
  when a merged dictionary is added to the app-level resources. The
  Settings window and the Settings preview both saw the new accent
  live; the actual on-screen OSD kept its default palette. `OsdHost`
  now mirrors `ThemeService.BuildAccentOverride` into its own
  `Resources.MergedDictionaries` on every `ThemeApplied` (and once at
  construction), so DynamicResource lookups inside `OsdContent` hit
  the tinted surfaces immediately.
- **Installer no longer bails with a raw stack trace when a target
  file is locked.** `Process.Kill + WaitForExit(2000) + Sleep(1500)`
  was too optimistic for UIAccess-signed Plith unwinding under an
  active Norton scan — the running tray held image sections a moment
  longer than the copy loop tolerated, and the fallback sideline
  rename never got a chance to catch up. Kill grace is up to 5 s per
  process across three verification rounds, the post-kill cushion is
  5 s, and the retry loop widens to eight attempts spanning ~12 s.
- **Identical files short-circuit past the copy loop.** Third-party
  DLLs that don't change between two Plith releases
  (`Hardcodet.NotifyIcon.Wpf`, `NAudio`, `WpfScreenHelper`, the
  runtime bits) hit this on every upgrade — and those are also the
  files most likely to still be memory-mapped or under AV scan from
  the prior install. Length + SHA-256 comparison against the source
  skips the write entirely when the bytes already match, removing the
  single biggest source of install-lock failures.
- **Preflight kill fails loud instead of quietly slipping past.** The
  old `catch { }` around `Process.Kill` swallowed permission /
  protected-handle failures and let the copy step continue into a
  guaranteed `UnauthorizedAccessException`. `EnsurePlithIsClosed` now
  runs up to three kill rounds and, if a Plith process still refuses
  to die, throws `PlithStillRunningException` before touching a single
  file. Its message names Plith, points at the tray-Exit action, and
  mentions Task Manager as the fallback for hidden tray icons.
- **`InstallLockedFileException` for the terminal case.** When even
  the sideline rename can't win the race (a persistently-held file
  where nothing else survived), the installer surfaces a message that
  names the locked file, tells the user to Exit Plith from the tray,
  and points at the exact Norton exclusion location to add
  `%ProgramFiles%\Plith` to — shown directly on the failure screen
  instead of a `MoveFile` stack.
- **Windows Restart Manager integration.** The installer now uses the
  same OS service Windows Installer, Chocolatey, and Windows Update
  use to figure out which processes hold the install-dir files. Before
  the copy step: `RmShutdown` sends WM_CLOSE (then TerminateProcess as
  fallback) to every non-critical holder, so Plith stragglers and
  Explorer preview handlers close cleanly on their own. On terminal
  failure: `RmGetList` names the surviving holders (Norton, System,
  Windows Defender — whatever RM sees) and the error message quotes
  them verbatim — the user finally sees "Norton has it open" instead
  of guessing at a mystery "some file is locked" wall.

## [0.1.4] - 2026-09-01

### Added
- **Accent theme studio in Settings.** A new "Accent color" row in the
  Appearance card carries eight curated presets — Emerald (the historical
  default), Praxvon Lime, Sky, Frost, Violet, Peach, Amber, and Rose —
  plus a Custom slot that opens an HSL slider popup with a hex input
  for any colour the user wants. Selection is live: the Settings window
  and the OSD accent surfaces both update the moment a swatch is picked
  or a slider is dragged, no restart or window reopen required. The last
  chosen custom hex is preserved when switching back and forth between
  presets so the popup opens on the previous value instead of jumping
  to a default.

### Changed
- **`ThemeService` now stacks an accent-override `ResourceDictionary` at
  the tail of `Application.Resources.MergedDictionaries` after every
  palette swap.** The dark / light palettes continue to swap the way
  they did in 0.1.3; the override replaces the four Settings accent
  brushes (`Accent`, `AccentHover`, `AccentPressed`, `AccentGlow`) plus
  the OSD's `OsdAccent`, `OsdSurfaceBrush`, `OsdBorder`, `OsdTrackBg`,
  and `OsdDivider` with brushes derived from the picked colour.
- **The entire OSD card is now tinted from the picked accent** — a deep
  hue-shifted dark on the dark theme (L≈0.07-0.11, capped saturation so
  loud primaries like Praxvon Lime read as a tint rather than paint), a
  pale tint on the light theme. Semi-transparency stays where it was
  (F0 on the surface gradient) so the overlay still floats over games
  the way it did before. The volume bar sits at the full accent tone
  so it pops off the tinted card.
- **Hover / pressed variants for Settings buttons use HSL math tuned
  per surface** — brighter on dark backgrounds, darker on light ones,
  with a luminance clamp so bright accents stay readable on white
  cards.
- **Volume bar routing:** with colour thresholds off (the default), the
  bar follows the picked accent; with thresholds on, the semantic
  green / amber / red set kicks in unchanged so the loudness cue keeps
  its meaning across every accent.
- **`config.ini` gains an `[Appearance]` section** with `AccentThemeId`
  and (optionally) `CustomAccentColor`. Unknown ids from newer builds
  survive a round-trip without being coerced to Emerald so downgrades
  don't silently rewrite the user's colour.

## [0.1.3] - 2026-08-31

### Added
- **Overlay position picker with drag-and-drop.** Settings has a single
  "Set position" button that dims every monitor, shows nine snap hotspots
  on a 3x3 grid, and lets you grab the OSD card and drag it anywhere.
  Release near a hotspot to magnet-snap; hold Alt to bypass; click a
  hotspot directly to jump. Save / Cancel from a floating toolbar that
  automatically hops to whichever screen side is opposite the OSD so it
  never hides underneath. The old Position combo box is gone.
- **Multi-monitor awareness for Custom positions.** The monitor's Windows
  device name (`\\.\DISPLAY2`, ...) is saved alongside the fractional
  coordinates, so the OSD stays on the display the user dropped it on
  across restarts. Falls back to the primary monitor silently when that
  display is unplugged.

### Changed
- **Custom position now stores the OSD's centre**, not its top-left corner.
  A media card appearing or disappearing changes the OSD's width; the old
  top-left anchor could push a right-side placement off-screen when the
  card grew. Anchoring on centre keeps the OSD visually pinned to the same
  point through content resizes.

### Fixed
- **OSD not centered / off-position on high-DPI displays.**
  `BandWindow.SetPosition` was passing DIP coordinates straight to Win32's
  `SetWindowPos`, which expects physical pixels. On 100% DPI the two are
  equal so the bug never surfaced; on 125% / 150% / 175% the OSD landed
  roughly `1/dpi` of the way across the screen (top-left-ish) instead of at
  the selected corner. `SetPosition` now multiplies by the current monitor's
  DPI scale, matching the size path.
- **Installer can now overwrite locked files in Program Files.** The
  overwrite path now retries with exponential back-off and, if the file is
  still locked (typically Norton or Windows Defender holding a scan handle),
  renames the old copy aside with `MoveFileEx MOVEFILE_DELAY_UNTIL_REBOOT`
  and drops the new bytes at the original path. The earlier retry loop
  filtered out its final attempt so the sideline fallback was never reached
  even after it existed; now it always falls through on the last try.
- **Settings preview redraws after saving a Custom position** so it shows
  the new placement instead of the old preset.

## [0.1.2] - 2026-08-31

### Fixed
- **Tray Exit deadlock that appeared to freeze the whole system.**
  `WindowsAudioClient.Stop` held its lock across
  `UnregisterEndpointNotificationCallback`, which synchronously waits for any
  in-flight COM MTA callback to return. Those callbacks reacquire the same
  lock at entry, so the UI dispatcher deadlocked. The frozen dispatcher then
  starved the `WH_KEYBOARD_LL` hook, making every keystroke wait on
  `LowLevelHooksTimeout`. Draining the enumerator outside the lock resolves
  it. `App.OnExit` also gained per-step Dispose logging so any future
  shutdown hang points at the exact culprit.

### Changed
- **Installer no longer needs the Windows 10/11 SDK on the target machine.**
  `Plith.exe` is now signed with the developer's code-signing cert at build
  time and the cert's public key is embedded in the installer. Install-time
  signing (and `SignToolWrapper`) is gone; `CertService` just registers the
  embedded cert in `LocalMachine\Root` + `TrustedPublisher` so the pre-signed
  binary validates for UIAccess.

### Added
- **In-app update checker.** Settings gains an "Updates" card that queries
  GitHub Releases, downloads the matching `Plith-Setup-*.exe` asset with
  progress, launches it via UAC, and exits Plith so the update swap
  doesn't collide with a running binary. A "Release notes" button opens
  the release page in the default browser.

## [0.1.1] - 2026-08-05

### Added
- **Mixer-agnostic Windows endpoint pinning.** Pin any Windows render endpoint
  (SteelSeries Sonar Chat/Game/Media, Rode Unify submix, Elgato Wave Link
  input, VB-Cable) as the OSD source instead of following the OS default.
  Silently falls back to the default when the pinned endpoint is unplugged,
  and resumes on it the moment it returns.
- **`WH_KEYBOARD_LL` global keyboard hook** intercepts the volume media keys
  before Windows' native flyout gets a chance to paint. Plith's OSD appears
  first; the native one no longer flickers on top on Win11 builds where the
  new flyout window class had escaped the suppressor.

### Fixed
- **Win11 native flyout suppression** now covers z-bands 0x10 / 0x11 / 0x12;
  previously the flyout would still paint on some 24H2 builds.
- **Shutdown crash** caused by the WinRT SMTC finalizer running after WPF
  teardown. `OnExit` now calls `Environment.Exit(...)` after the dispose chain.
- **Log spam** from `NativeFlyoutSuppressor`. Cheap filters gate before any
  log write, so idle window events don't fill the log.
- Settings window no longer appears in the Alt+Tab list
  (`ShowInTaskbar="False"`).
- Disabled ComboBox now visibly looks disabled (opacity + cursor).

### Changed
- **Voicemeeter integration is optional at runtime.** Plith detects whether
  `VoicemeeterRemote64.dll` is present and collapses source modes and Settings
  rows when it isn't, so users without Voicemeeter installed don't see dead
  controls.

## [0.1.0] - 2026-07-30

Initial public release.

- Voicemeeter-first audio OSD via `VoicemeeterRemote64.dll` P/Invoke; polls
  Bus A1 by default, any bus 0–31 by configuration.
- Windows Core Audio fallback via NAudio + `IMMNotificationClient` — default
  device swap reattaches transparently.
- SMTC media integration: album art, title, artist, transport controls for
  Spotify, YouTube in Chromium browsers, Twitch web, and Windows media apps.
- **Game mode** via a UIAccess-signed BandWindow + `CreateWindowInBand`;
  draws above exclusive fullscreen games.
- Free-form summon hotkey capture (Ctrl+Alt+V, Ctrl+Shift+V, Alt+Shift+V,
  Ctrl+Alt+M).
- Auto-start on Windows login via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Live theme following (Light / Dark / Auto), OSD stays dark for readability
  over games.
- Modern Settings window with live OSD preview, opacity slider, color
  thresholds, compact mode, hover keep-alive.
- Installer wizard: self-signed certificate setup, code signing, Program
  Files install, UIAccess enablement for Game mode.

[0.1.5]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.5
[0.1.4]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.4
[0.1.3]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.3
[0.1.2]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.2
[0.1.1]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.1
[0.1.0]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.0
