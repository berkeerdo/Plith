# Phase 4h — Game Mode (Always-On UIAccess + BandWindow)

**Date:** 2026-05-30
**Status:** Approved design, ready for writing-plans

## Problem statement

Phase 4g (force-topmost + ForegroundWatcher) closes the z-order race for **borderless fullscreen** games, which covers ~95% of modern titles. It does **not** lift Plith above true **exclusive fullscreen** (a game window that holds the DWM swap chain and bypasses normal compositing). Users running Valorant, CS2, or older D3D9/D3D11 titles in exclusive mode never see the OSD pop while a match is active.

The standard fix is the Windows UIAccess privilege combined with the undocumented `CreateWindowInBand` API — the same path used by MSI Afterburner, RTSS, FancyOSD, and similar overlays. Plith already has the `BandWindow` infrastructure ported from VoicemeeterFancyOSD (MIT, NOTICE.md credited) sitting unused in `src/Plith/Interop/BandWindow/`. This phase wires that infrastructure into `OsdWindow`, flips the manifest to `uiAccess="true"`, and adds a self-signed-cert + Program-Files install script so a local install can earn the UIAccess privilege Windows requires.

## Goals

- OSD draws above exclusive fullscreen games when Plith is installed via the included PowerShell install script.
- Dev builds (`dotnet run` from `bin\Debug\`) continue to work in normal topmost mode without any cert/install ceremony — Phase 4g behavior preserved as graceful fallback.
- Zero paid dependencies. Self-signed cert + script-based install only. WiX MSI deferred to the v0.1.0 release phase.
- Users can see at a glance (Settings → About) whether game mode is currently active or limited.

## Non-goals

- WiX MSI installer — deferred to v0.1.0 release; PowerShell install script covers Phase 4h scope.
- Auto-detect game mode toggle — always-on simplifies state management; the BandWindow infrastructure already falls back to a lower z-band when UIAccess isn't granted.
- Settings UI toggle for game mode — the install path (Program Files + signed) is the toggle; no in-app on/off switch.
- Cert renewal automation — 5-year cert validity is enough for Phase 4h; v1.0 roadmap item.
- Anti-cheat allow-list integration with specific games — README disclaimer is the documented mitigation; users opt out via tray exit.

## Approach

### Component 1 — OsdHost (BandWindow refactor)

Replace `src/Plith/Views/OsdWindow.cs` (`: Window`) with a new `src/Plith/Views/OsdHost.cs` (`: BandWindow`). `BandWindow` is a WPF `ContentControl` that owns its own native HWND created via `CreateWindowInBand`, with graceful fallback to `CreateWindowEx` when the API is unavailable.

`OsdHost` preserves the **exact** public surface the rest of Plith consumes:

```csharp
public sealed class OsdHost : BandWindow
{
    public OsdViewModel ViewModel { get; }
    public event EventHandler<MediaCommand>? MediaCommandInvoked;
    public void ShowOsd(TimeSpan visibleFor);
    public void ReassertTopmost();
}
```

Constructor responsibilities:

- `ZBandID = NativeMethods.GetTopMostZBandID()` — returns `AboveLockUX` when the process has UIAccess + is immersive, `UIAccess` when only UIAccess, `Desktop` otherwise. The fallback chain is automatic and silent.
- `TopMost = true`, `Activatable = false` (never steal focus), `IsClickThrough = false` (mouse hover keep-alive needs hit-testing).
- Content = new `OsdContent { DataContext = ViewModel }` — same UserControl Phase 1 built.
- `Opacity = 0` on construct; first `Show()` materializes the HWND (already handled by `BandWindow.Show` → `CreateWindow`).
- Subscribe to `_settings.Changed` for the live-preview hooks Phase 4c-3 added.

Behavior mapping:

| Old `OsdWindow` method | New `OsdHost` impl |
|---|---|
| `ShowOsd(visibleFor)` | Same logic: `Reposition()`, `BeginAnimation(OpacityProperty, fadeIn)`, `ReassertTopmost()`, `RestartHideTimer()`. WPF `Opacity` animates through layered-window per-pixel alpha — visually identical. |
| `FadeOutAndHide()` | Same `DoubleAnimation` to 0; on Completed, set `Opacity = 0`. Don't call `Hide()` — keeps HWND alive for instant next-show. |
| `Reposition()` | Same `Screen.PrimaryScreen.WorkingArea` math; write to `Left` / `Top` DependencyProperties (BandWindow.Ext.cs already wires these to `SetPosition()`). |
| `ReassertTopmost()` | `SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE \| SWP_NOSIZE \| SWP_NOACTIVATE)` — same as Phase 4g, just sourced from `BandWindow.Handle`. |
| `ApplyToolWindow()` | Delete — `WS_EX_TOOLWINDOW` is set in `BandWindow.CreateWindow` (`BandWindow.cs:193`). |
| `OnClosing` + `AllowShutdown` | Delete — `BandWindow.Ext.cs` already disposes `HwndSource` on `Application.Exit`. App.OnExit just stops subscribers. |
| `MouseEnter` / `MouseLeave` | Same — `BandWindow : ContentControl : FrameworkElement` raises these. |

Delete `src/Plith/Views/OsdWindow.cs`. Update type references in:
- `src/Plith/App.xaml.cs` (constructor + field type)
- `src/Plith/Services/OsdOrchestrator.cs` (constructor param + field type)
- `src/Plith/Services/ForegroundWatcher.cs` (constructor param + field type)
- `src/Plith/Services/TrayIconHost.cs` (verify — likely a tip reference for SettingsWindow plumbing)
- `src/Plith/Views/SettingsWindow.xaml.cs` (if it references OsdWindow for preview hooks)

### Component 2 — UIAccess manifest flip

`src/Plith/app.manifest`:

```xml
<requestedExecutionLevel level="asInvoker" uiAccess="true" />
```

Update the inline comment to reflect Phase 4h reality: "UIAccess requires signed binary + Program Files install. Use scripts/install-local.ps1 for production install. Dev builds gracefully fall back to non-UIAccess mode."

This flip on its own is harmless — Windows ignores `uiAccess="true"` when the binary is unsigned or installed outside a secure path. Dev workflow (`dotnet run` from `bin\`) keeps working.

### Component 3 — Self-signed cert + install scripts

Three PowerShell scripts under `scripts/`:

**`setup-cert.ps1`** (idempotent, requires admin):

1. Look up `CN=Plith Self-Signed` in `Cert:\CurrentUser\My`.
2. If absent: `New-SelfSignedCertificate` with `Type CodeSigningCert`, 5-year validity, save thumbprint to `scripts/.cert-thumbprint` (gitignored).
3. Export public cert to temp `.cer`, import to `Cert:\LocalMachine\TrustedPublisher` (this is the step that needs admin). Without TrustedPublisher entry, Windows won't honor the UIAccess request.
4. Emit thumbprint to stdout for downstream scripts.

**`install-local.ps1`** (requires admin):

1. `Stop-Process -Name Plith -EA SilentlyContinue` to release file locks.
2. Invoke `setup-cert.ps1`, capture thumbprint.
3. `dotnet publish src/Plith -c Release -o publish/ -p:PublishSingleFile=false`. Single-file disabled because UIAccess binaries must have their manifest readable by `appcompat` before exec — single-file embeds manifest in a way Windows occasionally mis-parses.
4. `signtool sign /sha1 $thumbprint /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 publish\Plith.exe`. SignTool comes from Windows SDK; install script gives an actionable error if missing.
5. `robocopy publish\ "C:\Program Files\Plith\" /MIR` — mirror to install dir, removes stale files from prior installs.
6. Update `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run` Plith entry to `C:\Program Files\Plith\Plith.exe`. (Reuses `AutoStartService` logic if convenient; otherwise inline registry edit.)
7. `Start-Process "C:\Program Files\Plith\Plith.exe"`.

**`uninstall-local.ps1`** (requires admin):

1. `Stop-Process -Name Plith`.
2. `Remove-Item "C:\Program Files\Plith\" -Recurse -Force`.
3. Remove `Run` registry value.
4. Leaves the cert in place. (Optional `cleanup-cert.ps1` for a clean wipe — out of scope here.)

`scripts/.cert-thumbprint` is gitignored. The cert itself never leaves the local machine — git-tracking is intentionally avoided so distribution remains "personal use only" until v0.1.0 release.

### Component 4 — Status badge in Settings

New `src/Plith/Services/UiAccessProbe.cs` — single static method:

```csharp
public static bool IsGameModeActive() =>
    NativeMethods.IsBandWindowSupported() &&
    NativeMethods.HasUiAccessProcess(System.Diagnostics.Process.GetCurrentProcess().Handle);
```

`SettingsWindow.xaml` About tab gets a new row above the existing version string:

- Active path (green dot + text): `"Game mode: Active (works over fullscreen games)"`
- Limited path (amber dot + text): `"Game mode: Limited — run scripts\install-local.ps1 for fullscreen support"`

The label uses existing dark/light palette brushes (`AccentGreen`, `WarningAmber` or equivalents from Phase 4e/4f). No new colors required.

### Component 5 — README updates

New "Game mode" section under installation:

```markdown
## Game mode (exclusive fullscreen support)

By default, Plith works as a normal topmost overlay — it draws over **borderless fullscreen** games (CS2, Valorant, Apex, Fortnite, etc. when set to borderless mode), which is what 95% of modern titles use.

To draw over **exclusive fullscreen** games as well, Plith needs Windows' UIAccess privilege, which requires:
1. A digitally signed binary
2. Installation to `\Program Files\`

The included script handles both:

    pwsh scripts\install-local.ps1   # requires admin once

The script generates a self-signed code-signing cert (5-year validity), imports it to Windows' Trusted Publisher store, signs Plith.exe, and installs it to `\Program Files\Plith\`. From then on, Plith launches in game mode automatically.

To uninstall:

    pwsh scripts\uninstall-local.ps1

**Anti-cheat note.** Plith is a passive overlay — it reads no game memory, injects no input, and uses only documented Windows APIs (with one exception: `CreateWindowInBand`, also used by MSI Afterburner, RTSS, FancyOSD). Tools using equivalent techniques run on millions of PCs without anti-cheat issues. However, some games' anti-cheats (Vanguard for Valorant, EAC for several titles) may treat any UIAccess overlay with suspicion. If you play competitive ranked matches in such games, exit Plith from the tray icon beforehand.
```

## Lifecycle / data flow

```
App.OnStartup
  ├─ ThemeService.Start                     (unchanged)
  ├─ _osd = new OsdHost(_settings)          [WAS new OsdWindow(_settings)]
  │    └─ OsdHost ctor:
  │       - ZBandID = GetTopMostZBandID()   ← UIAccess → AboveLockUX; else Desktop
  │       - CreateWindow()                  ← creates native HWND via CreateWindowInBand
  │       - Opacity = 0                     ← invisible until first ShowOsd
  ├─ OsdOrchestrator(_osd, _settings).Start (unchanged — public surface preserved)
  ├─ ForegroundWatcher(_osd).Start          (unchanged — ReassertTopmost still works)
  ├─ HotkeyService + tray (unchanged)

ShowOsd path (unchanged from caller's perspective):
  OsdOrchestrator.HandleValueChange
    → _osd.ShowOsd(visibleFor)
       ├─ Reposition()
       ├─ BeginAnimation(OpacityProperty, fadeIn)
       ├─ ReassertTopmost()                 ← SetWindowPos HWND_TOPMOST
       └─ RestartHideTimer()

App.OnExit
  ├─ ForegroundWatcher.Dispose              (unchanged)
  ├─ orchestrator.Dispose                   (unchanged)
  └─ Application.Exit fires
      └─ BandWindow.Ext.OnAppExit
         └─ HwndSource.Dispose()            ← clean shutdown of native HWND
```

## Error handling

- **`IsBandWindowSupported()` returns false** (Win7 or non-Windows). Already handled in `BandWindow.CreateWindow` — falls back to `CreateWindowEx`. Same topmost behavior as old OsdWindow.
- **UIAccess not granted** (dev build, or production install never run). `GetTopMostZBandID()` returns `Desktop`. OSD is still topmost via WS_EX_TOPMOST, behaves like Phase 4g. Status badge shows "Limited".
- **`SetWindowPos` returns false in `ReassertTopmost`**. Benign and silent — next show retries.
- **`signtool` not on PATH** during `install-local.ps1`. Script errors out with: `"signtool.exe not found. Install Windows SDK or VS Build Tools, or run from a Developer PowerShell prompt."` Exit 1.
- **Admin elevation denied** during cert import or Program Files copy. PowerShell `Import-Certificate` / `robocopy` will fail; script catches, prints `"This step requires administrator privileges. Right-click PowerShell → Run as administrator."` Exit 1.
- **Cert expired (5 years from setup)**. Out of scope for Phase 4h — install script would silently re-sign with the expired cert, Windows would refuse to honor UIAccess, status badge would flip to "Limited" without obvious cause. v1.0 roadmap: detect expiry in `setup-cert.ps1` and regenerate.

## Testing

**Manual smoke (dev build, `dotnet run` from `bin\Debug\`):**
- OSD pops on volume change → normal topmost behavior. Borderless games covered (Phase 4g behavior preserved).
- Settings → About → status badge shows amber "Limited".
- Existing Phase 4g checks (foreground swap re-asserts topmost, boot-race recovery) unchanged.

**Manual smoke (production install via `install-local.ps1`):**
- After install + relaunch, status badge shows green "Active".
- Spin volume during exclusive fullscreen game (e.g., CS2 launched with `-fullscreen`) — OSD pops on top of game.
- Alt-tab out and back — OSD still re-asserts above the game.
- Tray exit, tray re-launch — autostart entry points at `\Program Files\Plith\`, status badge stays green.
- Uninstall via `uninstall-local.ps1` — Plith gone, autostart cleaned, cert remains in store (Trusted Publisher entry survives by design for next reinstall).

**Unit tests:** 36/36 must stay green. The refactor is View-layer only; OsdViewModel, settings round-trip, hotkey format/migration tests are unaffected.

**No new unit tests** — `OsdHost` is Win32/COM glue; testing it meaningfully requires a real HWND + display. Existing pattern (no tests for `OsdWindow`, `ForegroundWatcher`) is consistent.

## Risk / open questions

- **Anti-cheat false positive.** Low historical rate (FancyOSD, RTSS, MSI Afterburner all use the same Win32 surface without bans), but non-zero. README disclaimer + "exit before ranked" guidance is the documented mitigation.
- **Single-file publish incompatibility.** UIAccess binaries occasionally trip up `appcompat`'s manifest parser when the manifest is embedded via `PublishSingleFile=true`. Spec uses `PublishSingleFile=false` (multi-file Release publish). If single-file becomes desirable later, validate Windows still honors UIAccess.
- **Dev/prod behavior divergence.** Developer running `dotnet run` sees "Limited" status, no fullscreen coverage. Could surprise contributors. Status badge makes the state visible; README's "Game mode" section explains the install requirement.
- **Cert thumbprint persistence.** `scripts/.cert-thumbprint` is per-machine and gitignored. If the cert is recreated (e.g., after wipe + reinstall), the thumbprint file rebuilds itself transparently. No state to migrate.
- **PowerShell execution policy.** Users with restricted execution policy can't run the install script. Documented workaround in README: `pwsh -ExecutionPolicy Bypass -File scripts\install-local.ps1`.

## Out of scope

- WiX MSI installer (v0.1.0 release).
- Cert renewal automation (v1.0).
- GUI installer / no-PowerShell install path.
- Code signing with a trusted CA cert (SignPath.io for OSS distribution — future option).
- Anti-cheat allow-list submissions to specific game publishers.
- Auto-detect mode that toggles z-band per active foreground process.
