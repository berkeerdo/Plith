# Changelog

All notable changes to Plith are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
