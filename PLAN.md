# Plith — Implementation Plan

> **Name verified**: zero brand collision (checked GitHub, trademarks, audio software market).
> **Tagline:** Modern Windows audio OSD with Voicemeeter-first design and media controls baked in.

---

## 1. Vision

Replace Windows' aging volume flyout with a modern overlay that:
- **Primary**: Visualizes Voicemeeter Banana / Potato Bus & Strip changes in real time (designed for Voicemeeter users who lose the native volume OSD when hooking volume keys).
- **Fallback**: When Voicemeeter isn't running, hooks into Windows audio endpoints and behaves as a beautiful drop-in replacement for the default flyout.
- **Bonus**: Integrates Windows Media Session API to show & control currently playing media (Spotify / YouTube / Brave / any SMTC-compliant source) inside the same overlay.

**Differentiator vs ModernFlyouts / FancyOSD:**
- Single unified panel for volume + media (those two tools each do one half).
- Voicemeeter-aware out of the box, not a bolt-on.
- Modern Windows 11 design language (Mica, rounded corners, smooth motion).

---

## 2. Scope

### In scope (Phase 1–3)
- Volume OSD that triggers on any volume change (Windows master, per-device, per-app, or Voicemeeter bus/strip).
- Topmost overlay that works over fullscreen exclusive games.
- Modern Windows 11 visual language: Mica/Acrylic background, 16px rounded corners, Segoe UI Variable, smooth ease-out fade/slide animations.
- Tray icon + minimal settings (position, duration, theme, what to monitor).
- Media controls: now-playing card with album art + play/pause/next/prev buttons.
- Auto-start on boot.

### Out of scope (for now)
- Cross-platform (Windows-only).
- Audio mixing or routing (use Voicemeeter for that).
- Sound effects / DSP (use Voicemeeter / Equalizer APO).
- Mobile companion.

---

## 3. Tech Stack & Rationale

**Chosen: WPF + .NET 10 (LTS)**

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| **WPF + .NET 10** | Mature, proven topmost-over-fullscreen approach (BandWindow + renamed ApplicationFrameHost), full Win32 interop, Mica supported via API call, FancyOSD reference impl exists | XAML verbose, no native Mica markup | ✅ **CHOSEN** |
| WinUI 3 | Native Mica/Acrylic, modern, Microsoft-recommended for Windows 11 | Topmost-over-fullscreen unproven, missing low-level Win32 access | ❌ Risky for our overlay requirement |
| Avalonia 11 | Cross-platform, modern .NET | Custom Win32 interop required, less battle-tested for overlay use case | ❌ Overkill, lose proven path |
| Tauri / Electron | Web UI freedom, easy design iteration | Webview layer breaks topmost-over-fullscreen; large bundles | ❌ Eliminated |

**Why .NET 10**: LTS (supported until Nov 2028). Already installed (10.0.8). Best long-term bet.

**Why WPF specifically**: VoicemeeterFancyOSD (MIT) already solved the hard parts in WPF — BandWindow API for topmost-over-fullscreen via renamed `ApplicationFrameHost.exe` trick. We'll **borrow the Host/Bridge/Interop code** (MIT compatible) and write our **own modern WPF UserControls + ViewModels** from scratch.

---

## 4. Dependencies (NuGet + bundled)

- **VoicemeeterRemote API** — already available at `C:\Program Files (x86)\VB\Voicemeeter\VoicemeeterRemote64.dll`. P/Invoke wrapper (write our own thin one, or borrow A-tG's `VmrapiDynWrap`).
- **Windows Media Session** — `Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager` (built-in via CsWinRT).
- **Windows Audio APIs** — `MMDeviceAPI` + `IAudioEndpointVolume` + `IAudioSessionNotification` via NAudio.CoreAudioApi or direct COM interop.
- **Mica/Acrylic** — `DwmSetWindowAttribute` with `DWMWA_SYSTEMBACKDROP_TYPE` (Win11 21H2+).
- **WpfScreenHelper** (NuGet) — multi-monitor positioning.
- **INI File Parser** (NuGet) — settings persistence.
- **Hardcodet.NotifyIcon.Wpf** (NuGet) — tray icon.

---

## 5. Phase Breakdown

### Phase 1 — Voicemeeter-first MVP (target: 3–4 hours)

**Deliverable:** Functional OSD that appears when Voicemeeter Bus A1 fader / mute changes. Modern Mica design, works over fullscreen games.

Steps:
1. `dotnet new wpf -n Plith -f net10.0-windows`
2. Add project structure (see §6).
3. Port BandWindow/Host/Bridge from FancyOSD (MIT) — credit in NOTICE.md.
4. Write `VoicemeeterClient` wrapper (P/Invoke for Login/Logout/GetParameterFloat/IsParametersDirty + polling loop).
5. Modern OSD `UserControl`: horizontal fader gauge + dB value + bus/strip name label + Mica background + rounded corners.
6. Trigger logic: poll Voicemeeter every 30ms, on parameter change → show OSD → fade out after 2s (configurable).
7. Tray icon with "Exit" only (settings deferred to Phase 3).
8. Test: launch + spin G733 wheel (hooked to Bus A1) → OSD should appear with live value.

**Done when:**
- Build & run on this machine.
- OSD shows in <100ms after Voicemeeter parameter change.
- Visible over fullscreen game (test with any Steam game in fullscreen exclusive).
- Mica background actually renders on Windows 11.

### Phase 2 — Media Controller (target: 2–3 hours)

**Deliverable:** Same OSD now also triggers on media change (next track, play/pause) AND can be summoned via hotkey for interactive media control.

Steps:
1. Add `MediaSessionClient` using `GlobalSystemMediaTransportControlsSessionManager.RequestAsync()`.
2. Subscribe to `SessionsChanged`, `MediaPropertiesChanged`, `PlaybackInfoChanged`.
3. Extend OSD `UserControl` with media card: album art (from `Thumbnail`) + title + artist + Play/Pause/Next/Prev buttons.
4. Layout strategy: if a volume change triggers OSD, show volume gauge prominently + media card compact. If media changes, show media prominently + volume small.
5. Optional: global hotkey (Win+Shift+V) to summon OSD on demand for media skip.

**Done when:**
- Spotify track change → OSD shows new track + album art.
- Buttons in OSD actually pause/play/skip.

### Phase 3 — Windows Audio Fallback + Settings + Polish (target: 4–6 hours)

**Deliverable:** OSD also works for non-Voicemeeter users. Settings UI. Auto-start. Theme options.

Steps:
1. `WindowsAudioClient` using `MMDeviceEnumerator` + `IAudioEndpointVolume` callbacks.
2. Source priority: if Voicemeeter is running → use it. Else fall back to Windows default endpoint.
3. Settings window (separate WPF window): position, duration, theme (light/dark/auto), which Voicemeeter bus to monitor, color-coded fader, opacity, auto-start toggle.
4. Color-coded gauge: green (<0 dB), amber (0–6), red (>6).
5. Per-app audio mini list (optional, EarTrumpet-lite): top 3 apps with volume sliders inside OSD.
6. Auto-start via Startup folder shortcut.
7. INI config at `%LOCALAPPDATA%\Plith\config.ini`.

**Done when:**
- Uninstall Voicemeeter temporarily → OSD still works for normal Windows volume.
- Settings persist across restarts.
- Auto-start works.

### Phase 4 — Release Polish (later)

- App icon + branding.
- Installer (MSIX or simple zip).
- README + screenshots + GIF demo.
- GitHub repo + Releases.
- Code signing (eventually, to avoid Norton/SmartScreen friction).

---

## 6. File Structure

```
C:\Projects\plith\
├── PLAN.md                         (this file)
├── CLAUDE.md                       (project context for future sessions)
├── NOTICE.md                       (credits — MIT inherited from FancyOSD's Host/Bridge)
├── README.md                       (write at Phase 4)
├── .gitignore                      (.NET + Visual Studio)
├── Plith.sln
├── src/
│   ├── Plith/                      (main WPF app)
│   │   ├── Plith.csproj
│   │   ├── App.xaml / App.xaml.cs
│   │   ├── Program.cs              (entry, before App boots)
│   │   ├── Views/
│   │   │   ├── OsdWindow.xaml
│   │   │   ├── OsdContent.xaml      (UserControl: gauge + media card)
│   │   │   ├── VolumeGauge.xaml
│   │   │   ├── MediaCard.xaml
│   │   │   └── SettingsWindow.xaml  (Phase 3)
│   │   ├── ViewModels/
│   │   │   ├── OsdViewModel.cs
│   │   │   ├── VolumeViewModel.cs
│   │   │   └── MediaViewModel.cs
│   │   ├── Services/
│   │   │   ├── VoicemeeterClient.cs
│   │   │   ├── MediaSessionClient.cs
│   │   │   ├── WindowsAudioClient.cs       (Phase 3)
│   │   │   ├── OsdOrchestrator.cs           (decides what to show)
│   │   │   └── SettingsService.cs
│   │   ├── Interop/
│   │   │   ├── BandWindow/                  (ported from FancyOSD MIT)
│   │   │   ├── Mica.cs
│   │   │   └── VoicemeeterRemoteInterop.cs
│   │   └── Resources/
│   │       ├── Styles.xaml
│   │       ├── Colors.xaml
│   │       └── icons/
│   ├── Plith.Host/                 (renamed ApplicationFrameHost.exe — ported, MIT)
│   └── Plith.Bridge/               (C++ bridge for true topmost — ported, MIT)
└── docs/
    └── architecture.md             (optional, post-MVP)
```

---

## 7. Setup Commands (run in fresh session)

```powershell
cd C:\Projects\plith

# Verify .NET 10 SDK (we have runtime; need SDK for building)
dotnet --list-sdks
# If no 10.x SDK, install:
# winget install Microsoft.DotNet.SDK.10

# Bootstrap solution
dotnet new sln -n Plith
dotnet new wpf -n Plith -o src/Plith -f net10.0-windows
dotnet sln add src/Plith/Plith.csproj

# Initialize git
git init
git add PLAN.md CLAUDE.md NOTICE.md .gitignore
git commit -m "chore: initial planning docs"
```

---

## 8. Reference Implementations to Mine

| Project | What to borrow | License |
|---|---|---|
| [VoicemeeterFancyOSD](https://github.com/A-tG/VoicemeeterFancyOSD) | Host/Bridge/Interop (BandWindow topmost-over-fullscreen) | MIT — credit in NOTICE.md |
| [ModernFlyouts](https://github.com/ModernFlyouts-Community/ModernFlyouts) | Native Windows volume hook approach (Phase 3) | MIT |
| [voicemeeter-remote-api-extended](https://github.com/A-tG/voicemeeter-remote-api-extended) | Clean C# wrapper around VoicemeeterRemote64.dll | MIT |
| [WpfScreenHelper](https://github.com/micdenny/WpfScreenHelper) | Multi-monitor positioning | MIT |

**Don't copy UI code** from any of them — design from scratch for the modern Win11 look we want.

---

## 9. User's Environment (capture from current session)

- **OS**: Windows 11 Pro 10.0.26200
- **Headset**: Logitech G733 Gaming Headset (wireless, USB dongle)
- **Voicemeeter**: Banana installed and running (`voicemeeterpro.exe`)
  - Strip 3 (VAIO) → A1 (G733) + A2 (CABLE Input)
  - Strip 4 (AUX) → A1 only
  - Hook Volume Keys → ✓ Bus A1
- **.NET runtime**: 8.0.27, 9.0.16, 10.0.x installed (Desktop + AspNetCore + WindowsDesktop)
- **Norton 360**: Active and aggressive (TLS interception via `NODE_EXTRA_CA_CERTS=norton-tls-shield-root.pem`). Blocks unsigned binaries from GitHub Releases. **Note**: Our compiled output will need user to add `C:\Projects\plith\` and `C:\Tools\Plith\` to Norton exception list.
- **Existing OSD**: VoicemeeterFancyOSD v1.2.2.1 installed at `C:\Tools\VoicemeeterFancyOSD\` — currently functional but visually outdated.

---

## 10. Open Questions for Day 1 Implementation Session

1. **Branding**: Stick with "Plith" or pick a new name before too much code references it?
2. **Position default**: Bottom-center (like Windows native flyout) or top-right (like FancyOSD)?
3. **Multi-bus design**: When user adjusts both Bus A1 and Bus A2, show separate OSDs sequentially or a unified panel with two gauges?
4. **Hotkey for media controls**: Auto-suggest `Win+Shift+V` or leave unmapped?
5. **Telemetry**: Strict no — confirm. (Personal tool, no analytics.)

---

## 11. Coding Conventions (per user's CLAUDE.md global rules)

- All code comments, docstrings, identifiers, commit messages: **English only**.
- Conventional Commits format.
- No "Co-Authored-By: Claude" or AI attribution anywhere.

---

## 12. Definition of Done — Phase 1 MVP

- [ ] `dotnet build` succeeds without warnings.
- [ ] `dotnet run` launches Plith; tray icon appears.
- [ ] Spinning G733 volume wheel triggers OSD with live Bus A1 dB value.
- [ ] OSD has Mica background and renders correctly on Windows 11.
- [ ] OSD is visible over a fullscreen Steam game (test with any title).
- [ ] OSD fades in/out smoothly (no jarring pop).
- [ ] No console output on launch (release build).
- [ ] Process memory <50 MB idle.
- [ ] Git repo has clean commit history.

---

## Next Action

In a fresh Claude Code session opened in `C:\Projects\plith\`:

> "Read PLAN.md and CLAUDE.md. We're starting Phase 1 MVP implementation. Begin with project bootstrap (§7) then proceed to step 3 in §5."
