# Changelog

All notable changes to Plith are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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

[0.1.1]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.1
[0.1.0]: https://github.com/berkeerdo/Plith/releases/tag/v0.1.0
