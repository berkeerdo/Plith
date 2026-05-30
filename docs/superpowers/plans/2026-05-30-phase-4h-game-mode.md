# Phase 4h Game Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the already-imported `BandWindow` infrastructure (FancyOSD MIT port at `src/Plith/Interop/BandWindow/`) into the OSD path, flip `app.manifest` to `uiAccess="true"`, and ship PowerShell scripts (`scripts/setup-cert.ps1`, `install-local.ps1`, `uninstall-local.ps1`) that produce a self-signed-cert + Program-Files install which earns Windows' UIAccess privilege so the OSD draws over exclusive fullscreen games. Dev builds (`dotnet run` from `bin\`) fall back to Phase 4g topmost behavior automatically.

**Architecture:** Replace `OsdWindow : Window` with new `OsdHost : BandWindow` (ContentControl + native HWND via `CreateWindowInBand`). `ZBandID = NativeMethods.GetTopMostZBandID()` — auto-picks `UIAccess` band when the process has UIAccess, `Desktop` band otherwise, with graceful fallback to `CreateWindowEx` if the API is unavailable. New `UiAccessProbe` static helper drives a read-only Game-mode status badge in Settings. PowerShell scripts produce a 5-year self-signed code-signing cert, import it to `LocalMachine\TrustedPublisher`, sign Plith.exe, and install to `\Program Files\Plith\`.

**Tech Stack:** WPF, .NET 10 (`net10.0-windows10.0.22000.0`), x64. PowerShell 7+ (pwsh) install scripts. SignTool from Windows SDK. Code style: English-only, Conventional Commits, no AI attribution in commits/code/docs. NETAnalyzers strict (0 warnings, 0 errors). 36 existing unit tests must stay green.

**Spec:** `docs/superpowers/specs/2026-05-30-phase-4h-game-mode-design.md`

---

## File Structure

**Create:**
- `src/Plith/Views/OsdHost.cs` — `BandWindow` subclass replacing OsdWindow. Hosts `OsdContent`. Public surface: `ViewModel`, `MediaCommandInvoked`, `ShowOsd(TimeSpan)`, `ReassertTopmost()`.
- `src/Plith/Services/UiAccessProbe.cs` — single static method `IsGameModeActive()` checking `IsBandWindowSupported() && HasUiAccessProcess()`.
- `scripts/setup-cert.ps1` — idempotent self-signed CodeSigning cert generation + TrustedPublisher import.
- `scripts/install-local.ps1` — build + sign + copy to Program Files + relaunch.
- `scripts/uninstall-local.ps1` — kill + remove Program Files dir + cleanup autostart.

**Modify:**
- `src/Plith/app.manifest` — `uiAccess="false"` → `"true"`, update inline comment.
- `src/Plith/App.xaml.cs` — replace `OsdWindow` with `OsdHost`; remove `_osd.Show()` (OsdHost pre-creates HWND in ctor); remove `_osd?.AllowShutdown()` (BandWindow.Ext handles shutdown).
- `src/Plith/Services/OsdOrchestrator.cs` — change constructor param + private field type from `OsdWindow` to `OsdHost`.
- `src/Plith/Services/ForegroundWatcher.cs` — same type swap; `ReassertTopmost()` API preserved.
- `src/Plith/Resources/Palette.Dark.xaml` — add `WarningAmber` brush (`#F5C242`, mirrors OsdGainAmber).
- `src/Plith/Resources/Palette.Light.xaml` — add `WarningAmber` brush (`#D97706`, darker for white surfaces).
- `src/Plith/Views/SettingsWindow.xaml` — append new "Game mode" section after "General" with a single read-only status row (dot + text).
- `src/Plith/Views/SettingsWindow.xaml.cs` — wire `UiAccessProbe.IsGameModeActive()` to the status badge text and dot color on window load.
- `README.md` — replace the "Visible-over-fullscreen" subsection with a new "Game mode" section covering install/uninstall scripts and anti-cheat note.
- `.gitignore` — add `scripts/.cert-thumbprint` entry.

**Delete:**
- `src/Plith/Views/OsdWindow.cs` — superseded by `OsdHost`.

---

## Task 1: Add WarningAmber brush to both palettes

Adds the amber brush used by the upcoming Game mode status badge when UIAccess is not granted. Brush color mirrors the OSD's existing amber threshold (`OsdGainAmber #F5C242` in dark theme, darkened to `#D97706` for light surface contrast).

**Files:**
- Modify: `src/Plith/Resources/Palette.Dark.xaml`
- Modify: `src/Plith/Resources/Palette.Light.xaml`

- [ ] **Step 1: Add WarningAmber to Palette.Dark.xaml**

Use Edit tool to add the brush right after `ErrorText` (line 32) in `src/Plith/Resources/Palette.Dark.xaml`:

```xml
    <SolidColorBrush x:Key="ErrorText" Color="#F87171" />
    <SolidColorBrush x:Key="WarningAmber" Color="#F5C242" />
```

- [ ] **Step 2: Add WarningAmber to Palette.Light.xaml**

Use Edit tool to add the brush right after `ErrorText` (line 33) in `src/Plith/Resources/Palette.Light.xaml`:

```xml
    <SolidColorBrush x:Key="ErrorText" Color="#DC2626" />
    <SolidColorBrush x:Key="WarningAmber" Color="#D97706" />
```

- [ ] **Step 3: Build to verify palette XAML still parses**

```powershell
dotnet build src/Plith/Plith.csproj -c Debug
```

Expected: build succeeds, 0 warnings, 0 errors.

- [ ] **Step 4: Commit**

```powershell
git add src/Plith/Resources/Palette.Dark.xaml src/Plith/Resources/Palette.Light.xaml
git commit -m @'
feat(palette): add WarningAmber brush for Game mode badge

Adds amber brush to both dark and light palettes. Dark mirrors the OSD's
existing #F5C242 threshold color; light darkens to #D97706 for white-surface
contrast. Consumed by the Phase 4h Game mode status badge in Settings.
'@
```

---

## Task 2: Create UiAccessProbe service

Single static helper that combines `NativeMethods.IsBandWindowSupported()` and `NativeMethods.HasUiAccessProcess()` into one boolean the Settings UI binds to.

**Files:**
- Create: `src/Plith/Services/UiAccessProbe.cs`

- [ ] **Step 1: Create UiAccessProbe.cs**

Write `src/Plith/Services/UiAccessProbe.cs`:

```csharp
using Plith.Interop;

namespace Plith.Services;

/// <summary>
/// Reports whether the current process can use the UIAccess + CreateWindowInBand path
/// to draw above exclusive fullscreen games. True only when the underlying Win32 API
/// is available AND the process token carries the UIAccess privilege (granted by
/// Windows when a signed binary in a trusted location requests it via app.manifest).
/// </summary>
public static class UiAccessProbe
{
    public static bool IsGameModeActive()
    {
        if (!NativeMethods.IsBandWindowSupported()) return false;
        using var proc = System.Diagnostics.Process.GetCurrentProcess();
        return NativeMethods.HasUiAccessProcess(proc.Handle);
    }
}
```

- [ ] **Step 2: Build**

```powershell
dotnet build src/Plith/Plith.csproj -c Debug
```

Expected: build succeeds, 0 warnings, 0 errors.

- [ ] **Step 3: Commit**

```powershell
git add src/Plith/Services/UiAccessProbe.cs
git commit -m @'
feat(services): add UiAccessProbe for Game mode detection

Returns true when CreateWindowInBand is exported (Win10+) and the process
token carries UIAccess. The Settings Game mode badge flips between active
and limited based on this single call.
'@
```

---

## Task 3: Refactor OsdWindow → OsdHost (BandWindow subclass)

The big refactor. Creates `OsdHost : BandWindow` with the same public surface (`ShowOsd`, `ViewModel`, `MediaCommandInvoked`, `ReassertTopmost`) so the rest of the codebase only needs a one-line type swap. The new class hosts `OsdContent` as the BandWindow's `Content`, animates `Opacity` via WPF (layered window per-pixel transparency composes correctly), and uses `Left`/`Top` DependencyProperties already wired in `BandWindow.Ext.cs`. Drops the `OnClosing` + `AllowShutdown` hack — `BandWindow.Ext.OnAppExit` disposes `HwndSource` on `Application.Exit` directly.

**Files:**
- Create: `src/Plith/Views/OsdHost.cs`
- Modify: `src/Plith/App.xaml.cs`
- Modify: `src/Plith/Services/OsdOrchestrator.cs`
- Modify: `src/Plith/Services/ForegroundWatcher.cs`
- Delete: `src/Plith/Views/OsdWindow.cs`

- [ ] **Step 1: Create OsdHost.cs**

Write `src/Plith/Views/OsdHost.cs`:

```csharp
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media.Animation;
using System.Windows.Threading;
using Plith.Interop;
using Plith.Services;
using Plith.ViewModels;
using WpfScreenHelper;

namespace Plith.Views;

/// <summary>
/// BandWindow-backed OSD host. Creates a native HWND via CreateWindowInBand in the
/// highest z-band the current process is allowed to enter (UIAccess when granted,
/// Desktop otherwise). Replaces the Phase 1 OsdWindow : Window approach, which
/// could not draw above exclusive fullscreen games.
/// </summary>
public sealed class OsdHost : BandWindow
{
    private const double EdgeMarginDip = 96;
    private const int FadeInMs = 140;
    private const int FadeOutMs = 220;

    private readonly OsdContent _content;
    private readonly SettingsService _settings;
    private DispatcherTimer? _hideTimer;
    private int _showGeneration;
    private TimeSpan _currentVisibleFor;
    private bool _isFadingOut;

    public OsdViewModel ViewModel { get; } = new();

    public event EventHandler<MediaCommand>? MediaCommandInvoked;

    public OsdHost(SettingsService settings)
    {
        _settings = settings;

        ZBandID = NativeMethods.GetTopMostZBandID();
        TopMost = true;
        Activatable = false;      // never steal focus
        IsClickThrough = false;   // mouse hover keep-alive needs hit-testing
        Opacity = 0;
        Focusable = false;

        _content = new OsdContent { DataContext = ViewModel };
        _content.MediaCommandInvoked += (s, cmd) => MediaCommandInvoked?.Invoke(this, cmd);
        Content = _content;

        MouseEnter += OnMouseEnter;
        MouseLeave += OnMouseLeave;

        // Propagate live-preview changes from Settings into the view-model so the OSD
        // updates without requiring a pop to materialise them. Same pattern as OsdWindow.
        _settings.Changed += m => Dispatcher.BeginInvoke(() =>
        {
            ViewModel.UseColorThresholds = m.UseColorThresholds;
            ViewModel.CompactMode = m.CompactMode;
            Reposition();
        });

        ViewModel.UseColorThresholds = _settings.Current.UseColorThresholds;
        ViewModel.CompactMode = _settings.Current.CompactMode;

        // Pre-create the native HWND so the first ShowOsd is instant.
        // BandWindow.CreateWindow is idempotent if HasSourceCreated is already true.
        CreateWindow();
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_NOSIZE = 0x0001;
    private const uint SWP_NOMOVE = 0x0002;
    private const uint SWP_NOACTIVATE = 0x0010;

    /// <summary>Re-assert HWND_TOPMOST so a game / video player that raised itself topmost
    /// after our last ShowOsd doesn't sit above us. Safe to call repeatedly: SetWindowPos
    /// with NOACTIVATE leaves the foreground window's focus untouched.</summary>
    public void ReassertTopmost()
    {
        if (Handle == 0) return;
        _ = SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }

    private void OnMouseEnter(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_settings.Current.HoverKeepAlive) return;
        _hideTimer?.Stop();
        BeginAnimation(OpacityProperty, null);
        Opacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
    }

    private void OnMouseLeave(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_settings.Current.HoverKeepAlive) return;
        if (_currentVisibleFor <= TimeSpan.Zero) return;
        if (_isFadingOut) return;
        RestartHideTimer(_currentVisibleFor);
    }

    private void RestartHideTimer(TimeSpan visibleFor)
    {
        _hideTimer?.Stop();
        _hideTimer = new DispatcherTimer { Interval = visibleFor };
        _hideTimer.Tick += (_, _) =>
        {
            _hideTimer!.Stop();
            FadeOutAndHide();
        };
        _hideTimer.Start();
    }

    public void ShowOsd(TimeSpan visibleFor)
    {
        _showGeneration++;
        _isFadingOut = false;
        _currentVisibleFor = visibleFor;
        double targetOpacity = Math.Clamp(_settings.Current.OsdOpacityPercent, 50, 100) / 100.0;
        bool wasHidden = Opacity < targetOpacity - 0.01;

        if (wasHidden)
        {
            Reposition();
            Show();   // BandWindow.Show — Visibility=Visible + SetWindowPos HWND_TOPMOST
            BeginAnimation(OpacityProperty, null);
            var fadeIn = new DoubleAnimation(targetOpacity, TimeSpan.FromMilliseconds(FadeInMs))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            };
            BeginAnimation(OpacityProperty, fadeIn);
        }
        else
        {
            BeginAnimation(OpacityProperty, null);
            Opacity = targetOpacity;
        }

        ReassertTopmost();

        if (_settings.Current.HoverKeepAlive && IsMouseOver) return;
        RestartHideTimer(visibleFor);
    }

    private void FadeOutAndHide()
    {
        var gen = _showGeneration;
        _isFadingOut = true;
        var fadeOut = new DoubleAnimation(0.0, TimeSpan.FromMilliseconds(FadeOutMs))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn },
        };
        fadeOut.Completed += (_, _) =>
        {
            if (_showGeneration == gen)
            {
                Opacity = 0;
                _isFadingOut = false;
            }
        };
        BeginAnimation(OpacityProperty, fadeOut);
    }

    private void Reposition()
    {
        var screen = Screen.PrimaryScreen;
        if (screen is null) return;
        var area = screen.WorkingArea;

        _content.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        _content.UpdateLayout();
        var w = _content.DesiredSize.Width;
        var h = _content.DesiredSize.Height;
        if (w == 0 || h == 0) return;

        (Left, Top) = _settings.Current.Position switch
        {
            OsdPosition.BottomCenter => (area.Left + (area.Width - w) / 2, area.Bottom - h - EdgeMarginDip),
            OsdPosition.BottomRight  => (area.Right - w - EdgeMarginDip,   area.Bottom - h - EdgeMarginDip),
            OsdPosition.TopCenter    => (area.Left + (area.Width - w) / 2, area.Top + EdgeMarginDip),
            OsdPosition.TopRight     => (area.Right - w - EdgeMarginDip,   area.Top + EdgeMarginDip),
            _                        => (area.Left + (area.Width - w) / 2, area.Bottom - h - EdgeMarginDip),
        };
    }
}
```

- [ ] **Step 2: Update App.xaml.cs to use OsdHost**

Use Edit tool. Three changes in `src/Plith/App.xaml.cs`:

First, change field type. Replace:

```csharp
    private OsdWindow? _osd;
```

with:

```csharp
    private OsdHost? _osd;
```

Second, change constructor + drop `Show()` call. Replace:

```csharp
        _osd = new OsdWindow(_settings);
        _osd.Show();   // create the native handle now so first ShowOsd is instant; Opacity=0 keeps it invisible
        _orchestrator = new OsdOrchestrator(_osd, _settings);
```

with:

```csharp
        _osd = new OsdHost(_settings);   // ctor calls CreateWindow() so first ShowOsd is instant
        _orchestrator = new OsdOrchestrator(_osd, _settings);
```

Third, drop `AllowShutdown()` call in `OnExit`. Replace:

```csharp
        _osd?.AllowShutdown();    // unblock OnClosing so real shutdown can destroy the window
        _trayHost?.Dispose();
```

with:

```csharp
        // BandWindow.Ext.OnAppExit disposes HwndSource on Application.Exit; no manual unblock needed.
        _trayHost?.Dispose();
```

- [ ] **Step 3: Update OsdOrchestrator type reference**

Use Edit tool in `src/Plith/Services/OsdOrchestrator.cs`. Replace:

```csharp
    private readonly OsdWindow _osd;
```

with:

```csharp
    private readonly OsdHost _osd;
```

And replace the constructor signature:

```csharp
    public OsdOrchestrator(OsdWindow osd, SettingsService settings)
```

with:

```csharp
    public OsdOrchestrator(OsdHost osd, SettingsService settings)
```

- [ ] **Step 4: Update ForegroundWatcher type reference**

Use Edit tool in `src/Plith/Services/ForegroundWatcher.cs`. Replace:

```csharp
    private readonly OsdWindow _osd;
```

with:

```csharp
    private readonly OsdHost _osd;
```

And the constructor signature:

```csharp
    public ForegroundWatcher(OsdWindow osd)
```

with:

```csharp
    public ForegroundWatcher(OsdHost osd)
```

- [ ] **Step 5: Delete OsdWindow.cs**

```powershell
Remove-Item src\Plith\Views\OsdWindow.cs
```

- [ ] **Step 6: Build and verify zero warnings**

```powershell
dotnet build src/Plith/Plith.csproj -c Debug
```

Expected: build succeeds, 0 warnings, 0 errors. If any analyzer warning fires, fix it before continuing.

- [ ] **Step 7: Run tests (36 must stay green)**

```powershell
dotnet test tests/Plith.Tests/Plith.Tests.csproj
```

Expected: `Passed: 36, Failed: 0, Skipped: 0`. The View refactor shouldn't touch any tested surface — if a test fails, investigate before moving on.

- [ ] **Step 8: Manual smoke (dev mode, no UIAccess yet)**

```powershell
dotnet run --project src/Plith
```

Pop the volume wheel. Expected: OSD appears at bottom-center, fades in/out, hover keeps it alive, position changes when Settings position toggles. Phase 4g topmost behavior preserved (this is the graceful fallback path — UIAccess not granted yet because manifest still says `false` and binary is unsigned).

Kill Plith from tray. Expected: clean shutdown, no exception in Output window.

- [ ] **Step 9: Commit**

```powershell
git add -A
git commit -m @'
refactor(osd): replace OsdWindow with OsdHost (BandWindow subclass)

OsdHost subclasses BandWindow from src/Plith/Interop/BandWindow/ (already
ported from FancyOSD in Phase 1, unused until now). Z-band auto-picks via
NativeMethods.GetTopMostZBandID() — UIAccess when granted, Desktop with
graceful fallback. Public surface (ShowOsd, ViewModel, MediaCommandInvoked,
ReassertTopmost) preserved; App, OsdOrchestrator, ForegroundWatcher just
get a one-line type swap.

Drops the OnClosing + AllowShutdown hack — BandWindow.Ext.OnAppExit handles
HwndSource disposal on Application.Exit directly.

Dev builds (dotnet run from bin\) continue to work in topmost-only mode
since the manifest hasn't flipped yet and the binary is unsigned.
'@
```

---

## Task 4: Add Game mode status badge to Settings

A new "Game mode" section at the bottom of the Settings scroll list, after the existing "General" section. Single read-only row: status dot (green = active, amber = limited) + status text + helper sub-text pointing the user at `scripts\install-local.ps1` when limited.

**Files:**
- Modify: `src/Plith/Views/SettingsWindow.xaml`
- Modify: `src/Plith/Views/SettingsWindow.xaml.cs`

- [ ] **Step 1: Add Game mode section XAML**

Use Edit tool in `src/Plith/Views/SettingsWindow.xaml`. Find the closing of the "General" section (the `</Border>` right before `</StackPanel>` that ends the main scroll list — around line 409-411). Replace:

```xml
                                <CheckBox x:Name="AutoStartToggle"
                                          Grid.Column="1"
                                          Style="{StaticResource ToggleSwitchStyle}" />
                            </Grid>
                        </Border>
                    </Border>
                </Border>

                </StackPanel>
            </ScrollViewer>
```

with:

```xml
                                <CheckBox x:Name="AutoStartToggle"
                                          Grid.Column="1"
                                          Style="{StaticResource ToggleSwitchStyle}" />
                            </Grid>
                        </Border>
                    </Border>
                </Border>

                <!-- ============== Game mode ============== -->
                <TextBlock Style="{StaticResource SectionHeaderStyle}" Text="Game mode" Margin="4,18,0,10" />
                <Border Style="{StaticResource CardStyle}">
                    <Border Style="{StaticResource RowLastStyle}">
                        <Grid>
                            <Grid.ColumnDefinitions>
                                <ColumnDefinition Width="Auto" />
                                <ColumnDefinition Width="*" />
                            </Grid.ColumnDefinitions>
                            <Border x:Name="GameModeDot"
                                    Grid.Column="0"
                                    Width="8" Height="8"
                                    CornerRadius="4"
                                    VerticalAlignment="Top"
                                    Margin="0,6,12,0"
                                    Background="{DynamicResource WarningAmber}" />
                            <StackPanel Grid.Column="1">
                                <TextBlock x:Name="GameModeStatusLabel"
                                           Style="{StaticResource RowLabelStyle}"
                                           Text="Game mode: Limited" />
                                <TextBlock x:Name="GameModeHint"
                                           Style="{StaticResource RowHintStyle}"
                                           Text="Run scripts\install-local.ps1 (admin) to install Plith with UIAccess and draw over exclusive fullscreen games." />
                            </StackPanel>
                        </Grid>
                    </Border>
                </Border>

                </StackPanel>
            </ScrollViewer>
```

- [ ] **Step 2: Wire status in SettingsWindow.xaml.cs**

Use Edit tool in `src/Plith/Views/SettingsWindow.xaml.cs`. At the end of the constructor (right before the closing `}` at line 70, after `UpdateHotkeyConflictWarning();`) add:

```csharp
        UpdateHotkeyConflictWarning();
        ApplyGameModeStatus();
```

Then add the helper method right after `OnHotkeyBindingChanged` (around line 95):

```csharp
    private void OnHotkeyBindingChanged()
    {
        // BindingChanged can be raised from a non-UI thread in principle (Apply runs on
        // whichever dispatcher loaded the message window). Marshal to ours before touching XAML.
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(UpdateHotkeyConflictWarning));
            return;
        }
        UpdateHotkeyConflictWarning();
    }

    private void ApplyGameModeStatus()
    {
        bool active = UiAccessProbe.IsGameModeActive();
        GameModeDot.Background = (System.Windows.Media.Brush)FindResource(active ? "Accent" : "WarningAmber");
        GameModeStatusLabel.Text = active
            ? "Game mode: Active"
            : "Game mode: Limited";
        GameModeHint.Text = active
            ? "Plith is signed and running from a trusted location — OSD draws over exclusive fullscreen games."
            : @"Run scripts\install-local.ps1 (admin) to install Plith with UIAccess and draw over exclusive fullscreen games.";
    }
```

- [ ] **Step 3: Build**

```powershell
dotnet build src/Plith/Plith.csproj -c Debug
```

Expected: build succeeds, 0 warnings, 0 errors.

- [ ] **Step 4: Manual smoke**

```powershell
dotnet run --project src/Plith
```

Open Settings from the tray. Scroll to bottom — verify "Game mode" section appears below "General", dot is amber, label says "Limited", hint mentions `install-local.ps1`. Close Plith.

- [ ] **Step 5: Commit**

```powershell
git add -A
git commit -m @'
feat(settings): add Game mode status badge

New read-only section at the bottom of Settings shows whether the current
Plith process has UIAccess (green dot, "Active") or is running without it
(amber dot, "Limited" + hint pointing at scripts\install-local.ps1).
Driven by UiAccessProbe.IsGameModeActive().
'@
```

---

## Task 5: Create PowerShell install scripts

Three scripts under a new `scripts/` directory plus a gitignore entry for the cert-thumbprint state file. All three require admin elevation; install-local.ps1 chains setup-cert.ps1 transparently.

**Files:**
- Create: `scripts/setup-cert.ps1`
- Create: `scripts/install-local.ps1`
- Create: `scripts/uninstall-local.ps1`
- Modify: `.gitignore`

- [ ] **Step 1: Create scripts/setup-cert.ps1**

Write `scripts/setup-cert.ps1`:

```powershell
# scripts/setup-cert.ps1 — idempotent self-signed code-signing cert setup for Plith.
# Requires admin (TrustedPublisher store is HKLM-scoped).
# Emits the cert thumbprint on the last stdout line so install-local.ps1 can capture it.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

# Re-launch elevated if not already.
$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "setup-cert.ps1 requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$subject = 'CN=Plith Self-Signed'
$thumbFile = Join-Path $PSScriptRoot '.cert-thumbprint'

# 1. Find or create the cert in CurrentUser\My.
$cert = Get-ChildItem -Path Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
    Select-Object -First 1

if (-not $cert) {
    Write-Host "Generating new self-signed CodeSigning cert (5-year validity)..."
    $cert = New-SelfSignedCertificate `
        -Subject $subject `
        -Type CodeSigningCert `
        -KeyUsage DigitalSignature `
        -FriendlyName 'Plith Code Signing' `
        -CertStoreLocation Cert:\CurrentUser\My `
        -NotAfter (Get-Date).AddYears(5)
} else {
    Write-Host "Reusing existing Plith cert (thumbprint $($cert.Thumbprint))."
}

# 2. Persist thumbprint for install-local.ps1.
Set-Content -Path $thumbFile -Value $cert.Thumbprint -NoNewline

# 3. Ensure public cert is in LocalMachine\TrustedPublisher so Windows honors UIAccess.
$installed = Get-ChildItem -Path Cert:\LocalMachine\TrustedPublisher |
    Where-Object { $_.Thumbprint -eq $cert.Thumbprint }

if (-not $installed) {
    Write-Host "Importing public cert into LocalMachine\TrustedPublisher..."
    $tempCer = [IO.Path]::Combine($env:TEMP, "Plith.cer")
    try {
        Export-Certificate -Cert $cert -FilePath $tempCer | Out-Null
        Import-Certificate -FilePath $tempCer -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
    } finally {
        Remove-Item -Path $tempCer -ErrorAction SilentlyContinue
    }
} else {
    Write-Host "Public cert already trusted in LocalMachine\TrustedPublisher."
}

# Final stdout: thumbprint, so caller can `$thumb = & setup-cert.ps1 | Select-Object -Last 1`.
$cert.Thumbprint
```

- [ ] **Step 2: Create scripts/install-local.ps1**

Write `scripts/install-local.ps1`:

```powershell
# scripts/install-local.ps1 — build, sign, and install Plith to %ProgramFiles%\Plith\
# so it earns the UIAccess privilege from app.manifest. Requires admin.

[CmdletBinding()]
param(
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "install-local.ps1 requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$projPath = Join-Path $repoRoot 'src\Plith\Plith.csproj'
$publishDir = Join-Path $repoRoot 'publish'
$installDir = Join-Path $env:ProgramFiles 'Plith'

# 1. Resolve signtool. Windows SDK or VS Build Tools must be installed.
$signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signtool) {
    $candidates = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
        -Filter 'signtool.exe' -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match 'x64' } |
        Sort-Object FullName -Descending
    if ($candidates) { $signtool = $candidates[0] }
}
if (-not $signtool) {
    throw "signtool.exe not found. Install the Windows 10/11 SDK or VS Build Tools (workload: 'Desktop development with C++') and re-run."
}

# 2. Stop Plith if running so we can overwrite files.
Get-Process -Name Plith -ErrorAction SilentlyContinue | Stop-Process -Force

# 3. Set up cert (idempotent); capture thumbprint.
$thumb = & (Join-Path $PSScriptRoot 'setup-cert.ps1') | Select-Object -Last 1
if (-not $thumb) { throw "setup-cert.ps1 did not return a thumbprint." }

# 4. Build a multi-file Release publish. PublishSingleFile=false because UIAccess
#    binaries occasionally trip up appcompat's manifest parser when the manifest
#    is embedded in a single-file bundle.
Write-Host "Publishing $Configuration build..."
& dotnet publish $projPath -c $Configuration -o $publishDir `
    -p:PublishSingleFile=false -p:SelfContained=false | Out-Host
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed." }

# 5. Sign the main executable. WPF .NET app only needs the exe signed; the DLLs
#    don't need signatures for UIAccess to be honored (only the manifest-bearing
#    assembly does).
$exePath = Join-Path $publishDir 'Plith.exe'
Write-Host "Signing $exePath..."
& $signtool.Source sign /sha1 $thumb /fd SHA256 `
    /tr 'http://timestamp.digicert.com' /td SHA256 $exePath | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed." }

# 6. Mirror to Program Files. /MIR removes stale files from a prior install.
Write-Host "Installing to $installDir..."
if (-not (Test-Path $installDir)) { New-Item -ItemType Directory -Path $installDir | Out-Null }
& robocopy $publishDir $installDir /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
# robocopy uses non-zero success codes; 0-7 are non-error.
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with code $LASTEXITCODE." }

# 7. Launch. Plith's AutoStartService.Apply rewrites the HKCU\Run entry to the
#    new path on every startup, so no manual registry edit is needed here.
$installedExe = Join-Path $installDir 'Plith.exe'
Write-Host "Launching $installedExe..."
Start-Process -FilePath $installedExe

Write-Host ""
Write-Host "Done. Open Settings and check the Game mode badge — it should now read 'Active'."
```

- [ ] **Step 3: Create scripts/uninstall-local.ps1**

Write `scripts/uninstall-local.ps1`:

```powershell
# scripts/uninstall-local.ps1 — remove the Program Files install. Leaves the
# self-signed cert in place so re-install is one-step. Requires admin.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "uninstall-local.ps1 requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$installDir = Join-Path $env:ProgramFiles 'Plith'

# 1. Stop Plith if running.
Get-Process -Name Plith -ErrorAction SilentlyContinue | Stop-Process -Force

# 2. Remove install directory.
if (Test-Path $installDir) {
    Write-Host "Removing $installDir..."
    Remove-Item -Path $installDir -Recurse -Force
} else {
    Write-Host "$installDir not found — nothing to remove."
}

# 3. Clean up the HKCU\Run autostart entry that pointed at the now-deleted exe.
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$runValue = Get-ItemProperty -Path $runKey -Name 'Plith' -ErrorAction SilentlyContinue
if ($runValue) {
    Remove-ItemProperty -Path $runKey -Name 'Plith'
    Write-Host "Removed HKCU\Run autostart entry."
}

Write-Host "Done. Cert remains in CurrentUser\My + LocalMachine\TrustedPublisher for next install."
```

- [ ] **Step 4: Update .gitignore**

Use Edit tool. Append to `.gitignore`:

```
# Throwaway icon previews + logo concept exports (regenerable from tools/*.py)
tools/preview-*.png
tools/logo-*.png
tools/gallery/

# Code-signing cert thumbprint state (regenerated by scripts/setup-cert.ps1)
scripts/.cert-thumbprint
```

- [ ] **Step 5: Verify scripts parse**

```powershell
pwsh -NoProfile -Command "Get-Command -Syntax (Resolve-Path scripts\setup-cert.ps1)"
pwsh -NoProfile -Command "Get-Command -Syntax (Resolve-Path scripts\install-local.ps1)"
pwsh -NoProfile -Command "Get-Command -Syntax (Resolve-Path scripts\uninstall-local.ps1)"
```

Expected: each command prints the script's parameter syntax without throwing a parse error. If a script has a syntax error PowerShell will surface it here.

- [ ] **Step 6: Commit**

```powershell
git add scripts/setup-cert.ps1 scripts/install-local.ps1 scripts/uninstall-local.ps1 .gitignore
git commit -m @'
feat(scripts): self-signed cert + Program Files install scripts

Three PowerShell scripts under scripts/:
- setup-cert.ps1: idempotent self-signed CodeSigning cert (5y) + import to
  LocalMachine\TrustedPublisher so Windows honors UIAccess
- install-local.ps1: dotnet publish + signtool sign + robocopy mirror to
  %ProgramFiles%\Plith\ + launch
- uninstall-local.ps1: stop process + remove install dir + clean HKCU\Run

.gitignore tracks scripts/.cert-thumbprint as per-machine state.
'@
```

---

## Task 6: Flip manifest to uiAccess="true"

The one-line change that activates Game mode for signed + Program-Files installs. Dev `dotnet run` builds keep working because Windows ignores `uiAccess="true"` on unsigned binaries — they fall back to the same Phase 4g topmost path.

**Files:**
- Modify: `src/Plith/app.manifest`

- [ ] **Step 1: Flip uiAccess attribute and update comment**

Use Edit tool in `src/Plith/app.manifest`. Replace:

```xml
        <!-- UIAccess true would lift us above fullscreen exclusive games but requires signing + Program Files install. Phase 4. -->
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
```

with:

```xml
        <!-- uiAccess=true is honored by Windows only when the binary is signed AND installed
             to a trusted location (\Program Files\). Use scripts\install-local.ps1 for the
             production install. Dev builds (dotnet run from bin\) fall back to non-UIAccess
             mode transparently — BandWindow picks the Desktop z-band instead of UIAccess. -->
        <requestedExecutionLevel level="asInvoker" uiAccess="true" />
```

- [ ] **Step 2: Build**

```powershell
dotnet build src/Plith/Plith.csproj -c Debug
```

Expected: build succeeds, 0 warnings, 0 errors.

- [ ] **Step 3: Manual smoke (dev mode, post-manifest-flip)**

```powershell
dotnet run --project src/Plith
```

Pop the volume wheel. Expected: OSD still appears normally (UIAccess silently ignored on the unsigned dev build). Open Settings — Game mode badge still says "Limited" amber. Close Plith.

- [ ] **Step 4: Commit**

```powershell
git add src/Plith/app.manifest
git commit -m @'
feat(manifest): request uiAccess for Game mode

Flips uiAccess attribute to true so Windows grants UIAccess to signed
Plith.exe instances installed under \Program Files\. Dev builds are
unsigned and outside that path, so the request is silently ignored —
BandWindow gracefully falls back to the Desktop z-band, preserving
Phase 4g topmost behavior.
'@
```

---

## Task 7: README Game mode section + final verification

Replace the existing "Visible-over-fullscreen" subsection in the README with a new "Game mode" section covering install/uninstall script usage and the anti-cheat note. Then run the full verification matrix.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace the Visible-over-fullscreen subsection**

Use Edit tool in `README.md`. Replace lines 56-62 (the current subsection):

```markdown
### Visible-over-fullscreen

Plith uses a regular topmost WPF window (`ShowInTaskbar=false`, `ShowActivated=false`) so the
OSD floats above the desktop and over **fullscreen-borderless** games, which is what nearly
all modern titles ship with. **Fullscreen-exclusive** mode would need the BandWindow +
UIAccess path; that infrastructure is in the codebase but disabled, waiting on a signed
binary in a future release.
```

with:

```markdown
### Game mode (works over fullscreen games)

By default — running from `bin\` or any unsigned build — Plith uses a regular topmost
window so the OSD floats over **fullscreen-borderless** games, which is what nearly all
modern titles ship with. To draw over **exclusive fullscreen** games as well, Plith needs
the Windows UIAccess privilege, which requires a digitally signed binary installed to
`\Program Files\`.

The included PowerShell script handles both — generates a self-signed cert, signs Plith,
and installs it to `\Program Files\Plith\`:

```powershell
# Right-click PowerShell → Run as administrator
pwsh scripts\install-local.ps1
```

Open Settings after launch — the Game mode badge at the bottom flips from amber
"Limited" to green "Active". The OSD now uses `CreateWindowInBand` in Windows'
UIAccess z-band and draws above exclusive fullscreen games.

To uninstall:

```powershell
pwsh scripts\uninstall-local.ps1
```

**Anti-cheat note.** Plith is a passive overlay — it reads no game memory, injects
no input, and uses only documented Windows APIs (with one exception: `CreateWindowInBand`,
also used by MSI Afterburner, RTSS, and FancyOSD). Tools that use equivalent techniques
run on millions of PCs without anti-cheat issues. However, some games' anti-cheats
(Vanguard for Valorant, EAC for several titles) may treat any UIAccess overlay with
suspicion. If you play competitive ranked matches in such games, exit Plith from the
tray icon beforehand.
```

- [ ] **Step 2: Run the full test suite**

```powershell
dotnet test tests/Plith.Tests/Plith.Tests.csproj
```

Expected: `Passed: 36, Failed: 0, Skipped: 0`.

- [ ] **Step 3: Build Release with strict analyzers**

```powershell
dotnet build src/Plith/Plith.csproj -c Release -warnaserror
```

Expected: build succeeds, 0 warnings, 0 errors. `-warnaserror` enforces the strict NETAnalyzers contract.

- [ ] **Step 4: Manual install smoke**

Run the install script from an admin PowerShell prompt:

```powershell
# Open Windows Terminal as administrator first
pwsh scripts\install-local.ps1
```

Expected output sequence:
1. `Generating new self-signed CodeSigning cert (5-year validity)...` OR `Reusing existing Plith cert (thumbprint ...).`
2. `Importing public cert into LocalMachine\TrustedPublisher...` OR `Public cert already trusted in LocalMachine\TrustedPublisher.`
3. `Publishing Release build...` + dotnet publish output
4. `Signing C:\...\publish\Plith.exe...` + signtool output ending with `Successfully signed: ...\Plith.exe`
5. `Installing to C:\Program Files\Plith...`
6. `Launching C:\Program Files\Plith\Plith.exe...`
7. `Done. Open Settings and check the Game mode badge — it should now read 'Active'.`

Then verify in the running Plith:
- Open Settings from tray → scroll to bottom → **Game mode badge dot is green, label reads "Active"**.
- Tray exit, then re-launch from Start menu (Plith Self-Signed) → badge still green.
- Pop volume wheel — OSD appears normally.
- (Optional, requires an exclusive-fullscreen-capable game) Launch a game in exclusive fullscreen, pop volume — OSD appears over the game.

- [ ] **Step 5: Manual uninstall smoke**

```powershell
# Admin PowerShell
pwsh scripts\uninstall-local.ps1
```

Expected:
1. `Removing C:\Program Files\Plith...`
2. `Removed HKCU\Run autostart entry.` (if autostart was on)
3. `Done. Cert remains in CurrentUser\My + LocalMachine\TrustedPublisher for next install.`

Verify:
- `C:\Program Files\Plith\` no longer exists.
- `HKCU:\Software\Microsoft\Windows\CurrentVersion\Run` has no `Plith` value.
- Cert still present: `Get-ChildItem Cert:\CurrentUser\My | Where Subject -eq 'CN=Plith Self-Signed'` returns the cert.

- [ ] **Step 6: Final dev-mode regression check**

```powershell
dotnet run --project src/Plith
```

Open Settings → Game mode badge is amber "Limited" again (since the installed binary is gone and we're running unsigned from bin\). All other functionality (volume OSD, media card, hotkey, position, theme) works exactly as before Phase 4h. Exit Plith from tray.

- [ ] **Step 7: Commit README**

```powershell
git add README.md
git commit -m @'
docs(readme): document Game mode install path + anti-cheat note

Replaces the placeholder Visible-over-fullscreen subsection with a real
Game mode section covering scripts/install-local.ps1, scripts/uninstall-local.ps1,
and the anti-cheat compatibility note (mitigation: exit Plith via tray before
competitive ranked matches).
'@
```

---

## Verification Summary

After all 7 tasks, the following must be true:

- `dotnet build src/Plith/Plith.csproj -c Release -warnaserror` → 0 warnings, 0 errors.
- `dotnet test tests/Plith.Tests/Plith.Tests.csproj` → 36 passed, 0 failed.
- Dev mode (`dotnet run`): OSD works as in Phase 4g; Settings → Game mode badge is amber "Limited".
- Production install (`pwsh scripts\install-local.ps1`): Plith launches from `\Program Files\Plith\`; Settings → Game mode badge is green "Active"; OSD draws over exclusive fullscreen games.
- Uninstall (`pwsh scripts\uninstall-local.ps1`): install directory removed; autostart entry cleaned; cert retained for next install.

---

## Notes for the Implementing Agent

- **Conventional Commits + Turkish-free + AI-attribution-free.** Commit messages strictly in English, Conventional Commits format. NEVER include `Co-Authored-By: Claude`, `Generated by Claude`, or any Claude / AI mention. Repository rule, non-negotiable.
- **PowerShell here-strings for commit messages.** The user's environment is Windows PowerShell. Use `git commit -m @'...'@` (single-quoted, literal) — the closing `'@` MUST be at column 0, no leading whitespace.
- **No `--no-verify`, no `--amend`.** New commits for every step. Pre-commit hooks (if any) must pass.
- **Confirm before running the install/uninstall scripts.** The user is the operator. After Task 5 commits the scripts, the manual install smoke in Task 7 Step 4 requires admin PowerShell that the user opens themselves — flag the elevation requirement explicitly and wait for them to confirm before invoking.
- **Don't reorder tasks.** Each task's commit isolates a single conceptual change. Reordering breaks bisectability.
- **BandWindow code is pre-existing.** Do NOT modify `src/Plith/Interop/BandWindow/*.cs` — it's MIT-credited port from FancyOSD (see NOTICE.md). Just consume it from `OsdHost`.
