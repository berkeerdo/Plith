<div align="center">

<img src="src/Plith/Resources/icons/plith.ico" width="96" height="96" alt="Plith" />

# Plith

**A modern Windows audio OSD with Voicemeeter-first design and media controls baked in.**

[![License: MIT](https://img.shields.io/badge/license-MIT-4AD695?style=flat-square)](LICENSE)
[![.NET 10](https://img.shields.io/badge/.NET-10.0-512BD4?style=flat-square)](https://dotnet.microsoft.com/)
[![Windows 11](https://img.shields.io/badge/Windows-10%2F11-0078D6?style=flat-square)](https://www.microsoft.com/windows)

</div>

---

Windows' aging volume flyout, replaced by a quiet, rounded card that surfaces what's actually changing —
Voicemeeter bus levels, the Windows default endpoint, and whatever's playing in Spotify / YouTube / Brave —
without taking your attention away from the thing you were already doing.

```
┌─────────────────────────────────────────┐
│  ▓▓ Now playing      Title    [⏮ ⏯ ⏭]  │
│  ─────────────────────────────────────  │
│  BUS A1               +3.0 dB           │
│  ████████████████████░░░░░░░             │
└─────────────────────────────────────────┘
                bottom-center
```

## Features

- **Voicemeeter-first.** Polls `VoicemeeterRemote64.dll` directly; reads Bus A1 by default,
  any bus from 0 to 31 by configuration. Detects when the Voicemeeter engine starts or stops
  and switches sources without restart.
- **Windows Core Audio fallback.** When Voicemeeter isn't running, Plith listens to the
  Windows default render endpoint via NAudio. Default-device swap (plug in headphones,
  switch output) reattaches transparently via `IMMNotificationClient`.
- **Mixer-agnostic endpoint pinning.** Any Windows render endpoint can be pinned as the
  OSD source instead of following the OS default — surface a single SteelSeries Sonar
  channel (Chat / Game / Media), a Rode Unify submix, an Elgato Wave Link input, or a
  VB-Cable line. When pinned, default-device swaps are ignored; when the pinned endpoint
  goes away (Sonar restart, device unplug) Plith silently falls back to the default and
  resumes on the pinned device the moment it returns.
- **Media (SMTC) integration.** Album art, title, artist, and play / pause / next / previous
  buttons for whatever app is publishing into `GlobalSystemMediaTransportControlsSession` —
  Spotify, YouTube in Brave / Edge / Chrome, the Windows app, Twitch web player, etc.
  The buttons route back to the original session.
- **Settings window.** Linear / Raycast-tier UI with a custom titlebar, Win11 rounded
  corners, and a thin overlay scrollbar. Follows Windows' light/dark theme live (or
  can be pinned to Dark / Light). Tray-anchored (`ShowInTaskbar=false`), never leaves
  an entry in the alt-tab list. Configurable knobs: show duration (500 ms – 10 s),
  position (BottomCenter / BottomRight / TopCenter / TopRight), hover keep-alive,
  opacity, color-thresholds toggle, compact mode, audio-source mode (Auto /
  ForceVoicemeeter / ForceWindows), monitored Voicemeeter bus, Windows endpoint pin,
  media auto-show toggle, launch on Windows login, free-form summon hotkey.
- **Summon hotkey.** A configurable system-wide hotkey (Ctrl+Alt+V, Ctrl+Shift+V,
  Alt+Shift+V, or Ctrl+Alt+M) pops the OSD with whatever values it currently holds —
  useful for one-handed media skips without touching the volume wheel first.
- **Auto-start on Windows login.** Toggles a per-user `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
  registry entry so Plith launches with Windows.
- **Branded tray icon.** Modern accent-green sound-emission mark; double-click opens
  Settings; right-click for Settings + Exit.

### Install

Download the latest `Plith-Setup-<version>.exe` from the [Releases](https://github.com/berkeerdo/Plith/releases) page
and double-click. The wizard handles cert setup, signing, and Program Files install
automatically.

Windows will show "Microsoft Defender SmartScreen prevented an unrecognized app from
starting" the first time — Plith is signed with a self-signed certificate, not a public
CA. Click **More info → Run anyway** to continue.

After install, Plith appears in Start menu search and Add/Remove Programs.

### Game mode (works over fullscreen games)

After the install above completes, Plith earns the Windows UIAccess privilege via the
self-signed certificate and installs to `\Program Files\Plith\`. Open Settings — the
Game mode badge at the bottom reads green **"Active"**. The OSD now uses
`CreateWindowInBand` in Windows' UIAccess z-band and draws above exclusive fullscreen
games, not just borderless ones.

**Anti-cheat note.** Plith is a passive overlay — it reads no game memory, injects
no input, and uses only documented Windows APIs (with one exception: `CreateWindowInBand`,
also used by MSI Afterburner, RTSS, and FancyOSD). Tools that use equivalent techniques
run on millions of PCs without anti-cheat issues. However, some games' anti-cheats
(Vanguard for Valorant, EAC for several titles) may treat any UIAccess overlay with
suspicion. If you play competitive ranked matches in such games, exit Plith from the
tray icon beforehand.

### Uninstall

**Settings → Apps → Installed apps → Plith → Uninstall**, or double-click
`Plith-Uninstaller.exe` in `C:\Program Files\Plith\Setup\`. The wizard removes the
install dir, Start menu shortcut, and Add/Remove Programs entry. The self-signed
code-signing certificate is left in `CurrentUser\My + LocalMachine\TrustedPublisher +
LocalMachine\Root` so a future re-install is one-step. To remove the cert manually,
open `certmgr.msc` and look for `CN=Plith Self-Signed`.

### Build a release artifact

```powershell
# From an admin PowerShell (cert lookup may require it)
pwsh scripts\build-release.ps1
```

Produces `release/Plith-Setup-<version>.exe`, signed with the self-signed cert.

## Configuration

Settings persist to `%LOCALAPPDATA%\Plith\config.ini`. The file is INI-encoded and written
with `CultureInfo.InvariantCulture`, so values are identical across machine locales:

```ini
[General]
AutoStart = False

[Osd]
ShowDurationMs = 2000
Position = BottomCenter
HoverKeepAlive = True
SummonHotkey = None

[Audio]
AudioSource = Auto
MonitoredBusIndex = 0

[Media]
AutoShowOnMedia = False
```

Open the Settings window from the tray icon (right-click → Settings… or double-click the icon).

## Build from source

Plith targets **.NET 10 on Windows 10 build 22000 (Windows 11 21H2) or newer**.

```powershell
# Requires: .NET 10 SDK
# winget install Microsoft.DotNet.SDK.10

git clone https://github.com/berkeerdo/Plith.git
cd Plith
dotnet build src/Plith/Plith.csproj -c Release
dotnet run --project src/Plith
```

For a redistributable single-file build:

```powershell
# Framework-dependent (~26 MB, end user needs .NET 10 runtime)
dotnet publish src/Plith/Plith.csproj -c Release -r win-x64 `
  -p:PublishSingleFile=true -p:SelfContained=false `
  -o dist/release-fdep

# Self-contained (~83 MB, no .NET runtime needed)
dotnet publish src/Plith/Plith.csproj -c Release -r win-x64 `
  -p:PublishSingleFile=true -p:SelfContained=true `
  -p:EnableCompressionInSingleFile=true `
  -o dist/release-sc
```

WPF + .NET 10 does not support `PublishTrimmed=true` (SDK error NETSDK1168), so size
optimization stops at the single-file bundle.

### Run the tests

```powershell
dotnet test tests/Plith.Tests/Plith.Tests.csproj
```

## Architecture

```
src/Plith/
├── Services/
│   ├── VoicemeeterClient.cs        # P/Invoke + 30 ms polling loop
│   ├── WindowsAudioClient.cs       # NAudio + IMMNotificationClient
│   ├── MediaSessionClient.cs       # CsWinRT SMTC wrapper
│   ├── OsdOrchestrator.cs          # Source state machine + funnels into one OSD pipeline
│   ├── SettingsService.cs          # INI-backed config
│   ├── AutoStartService.cs         # HKCU Run registry toggle
│   ├── HotkeyService.cs            # RegisterHotKey + hidden HWND_MESSAGE window
│   ├── TrayIconHost.cs             # Hardcodet.NotifyIcon + context menu
│   └── NativeFlyoutSuppressor.cs   # SetWinEventHook (currently disabled)
├── ViewModels/
│   ├── OsdViewModel.cs             # Source-agnostic: Label, GainNormalized, GainText
│   ├── MediaViewModel.cs           # Title, Artist, AlbumArt, IsPlaying
│   └── BoolToVisibilityConverters.cs
├── Views/
│   ├── OsdWindow.cs                # Topmost WPF Window, fade in/out, position dispatch
│   ├── OsdContent.xaml             # Card layout (media row + volume row)
│   ├── MediaCard.xaml              # Album art + title/artist + transport buttons
│   └── SettingsWindow.xaml         # Custom-titlebar Settings UI
├── Interop/
│   ├── Mica.cs                     # DwmSetWindowAttribute Mica/Acrylic backdrop helper
│   └── BandWindow/                 # Topmost-over-fullscreen infrastructure (currently inactive)
└── Resources/
    ├── Theme.xaml                  # OSD palette
    ├── SettingsTheme.xaml          # Settings palette + control templates
    └── icons/plith.ico             # Multi-resolution app icon
```

## Tech stack

- **WPF + .NET 10 (LTS)** for the UI and OSD window.
- **NAudio** for the Windows Core Audio (`MMDeviceEnumerator`, `IAudioEndpointVolume`,
  `IMMNotificationClient`) wrapper.
- **CsWinRT** for `Windows.Media.Control.GlobalSystemMediaTransportControlsSession`
  (target framework `net10.0-windows10.0.22000.0` brings the projection in automatically).
- **`Hardcodet.NotifyIcon.Wpf`** for the tray icon.
- **`WpfScreenHelper`** for multi-monitor screen rectangles.
- **`ini-parser-netstandard`** for the config file.

The BandWindow and `WndProcHookManager` interop layer is adapted from the MIT-licensed
[VoicemeeterFancyOSD](https://github.com/A-tG/VoicemeeterFancyOSD) (see `NOTICE.md`),
with our own modern WPF user controls and view-models written from scratch.

## Roadmap

- ~~**Phase 4c-3.** OSD opacity slider, color thresholds, compact mode.~~ **Done.**
- ~~**Phase 4d.** Game mode — BandWindow + UIAccess-signed binary.~~ **Done.**
- ~~**Phase 4e.** Free-form hotkey capture UI, light-theme variant.~~ **Done.**
- ~~**Phase 4f.** Mixer-agnostic endpoint pinning (Sonar / Unify / Wave Link / VB-Cable
  channels), Voicemeeter auto-detection, Win11-safe native flyout suppression
  (new z-bands + low-level keyboard hook so the flyout is intercepted before it
  paints).~~ **Done.**
- **Phase 4c-4.** Code signing via SignPath.io OSS path (or a paid certificate) and an
  MSIX installer so the download survives Norton / SmartScreen without manual exception.
- **Phase 4g.** Optional Sonar HTTP API integration — surface Sonar-specific labels
  ("Master +3 dB", "Chat mute") in the OSD when Sonar is running, alongside the
  existing endpoint-level view.

## Credits

- The Host / Bridge / `BandWindow` topmost-over-fullscreen technique is adapted from
  [VoicemeeterFancyOSD](https://github.com/A-tG/VoicemeeterFancyOSD) — MIT, A-tG and
  contributors. See [`NOTICE.md`](NOTICE.md) for the full attribution.
- The native-flyout-hide approach (suppressor, currently inactive) is inspired by
  [ModernFlyouts](https://github.com/ModernFlyouts-Community/ModernFlyouts).
- `VoicemeeterRemote64.dll` ships with the user's Voicemeeter installation by
  [VB-Audio Software](https://vb-audio.com/Voicemeeter/) (Vincent Burel) and is not
  redistributed by Plith.

## License

[MIT](LICENSE) — see the file for the full text.
