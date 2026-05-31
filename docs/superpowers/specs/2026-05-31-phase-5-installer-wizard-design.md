# Phase 5 — Custom WPF Setup Wizard

**Date:** 2026-05-31
**Status:** Approved design, ready for writing-plans

## Problem statement

Phase 4h shipped a working install path via `scripts/setup-cert.ps1`, `install-local.ps1`, `uninstall-local.ps1`. The flow works but feels developer-grade: open admin PowerShell, run a script, watch text scroll. There is no branded UX, no obvious entry point for non-technical users, no Add/Remove Programs polish (registry entries and Start menu shortcut were retrofitted into the PS script as a final patch).

Plith's positioning is "premium, quiet, Linear/Raycast tier." The current install UX is the only seam in the product that doesn't match that bar. This phase replaces the PowerShell scripts with a custom WPF setup wizard — a single signed `Plith-Setup-0.1.0.exe` that the user double-clicks, with branded titlebar, Mica backdrop, animated progress, and the same accent-green visual language as Settings. Existing scripts/ contents are deleted.

## Goals

- Single-file release artifact: `Plith-Setup-0.1.0.exe` (~80-100 MB), self-extracting, no separate ZIP or folder distribution.
- Premium UX matching Plith Settings aesthetic: custom titlebar, Mica, accent-green palette, smooth page transitions.
- Three-page wizard (Welcome → Progress → Finish) plus an Error page for failure paths. Advanced options collapsed by default on Welcome with sensible "all on" defaults.
- Auto-detect existing install, switch button label between Install / Reinstall / Update transparently.
- Forced install location: `C:\Program Files\Plith\`. No path picker, no custom directories, no `Game mode silently breaks` failure mode.
- Self-signed cert handled silently. Small `?` info tooltip on Welcome for transparency-curious users.
- Replace the legacy PS scripts entirely. The wizard is the sole install path; release builder script is the only thing in `scripts/`.
- Reuse Plith's theme system (Palette.Dark.xaml, custom titlebar styling, brushes) to maintain visual consistency without duplicating it.

## Non-goals

- License-acceptance page (MIT, doesn't warrant a click).
- WiX MSI / .msi format.
- Custom install location picker (UIAccess requirement makes "anywhere" footgun).
- Headless `--silent` install flag in this phase (future addition once interactive wizard is stable).
- Public CA / EV code signing — self-signed stays.
- Cert renewal automation (Phase 4h carry-over, future v1.0 work).
- Multi-language installer UI (English only, matching Plith).
- MSIX distribution (sideloading + cert chain ergonomics don't match self-signed pattern).
- Anti-cheat allowlist negotiation with game publishers.

## Approach

### Component 1 — `src/Plith.Installer/` WPF project

A new WPF project alongside `src/Plith/`. Targets `net10.0-windows10.0.22000.0`, x64, with NETAnalyzers strict (0w/0e). Public surface:

```
Plith.Installer/
├── Plith.Installer.csproj
├── app.manifest                 (requireAdministrator, uiAccess=false)
├── App.xaml + .cs               Single-instance, --uninstall flag routing
├── MainWindow.xaml + .cs        Custom titlebar + Mica + ContentControl host
├── Pages/
│   ├── WelcomePage.xaml + .cs
│   ├── ProgressPage.xaml + .cs
│   ├── FinishPage.xaml + .cs
│   └── ErrorPage.xaml + .cs
├── Services/
│   ├── InstallOrchestrator.cs
│   ├── CertService.cs
│   ├── EmbeddedExtractor.cs
│   ├── SignToolWrapper.cs
│   ├── ShortcutService.cs
│   ├── RegistryService.cs
│   ├── InstallDetector.cs
│   └── LogService.cs
├── ViewModels/
│   ├── InstallerViewModel.cs
│   └── InstallStepViewModel.cs
└── Resources/
    ├── Embedded/PlithBundle.zip      (populated by pre-build target)
    ├── Palette.Dark.xaml             (LinkedFile from src/Plith/Resources/)
    ├── Theme.xaml                    (linked / shared brush dictionary)
    ├── Animations.xaml               (SlideFadeIn/Out page transitions)
    └── icons/plith.ico               (linked from main project)
```

`tests/Plith.Installer.Tests/` accompanies for the unit-testable services (Cert, Detector, EmbeddedExtractor). UI / Wizard pages have no tests — matches Plith's existing pattern for View layer.

### Component 2 — Wizard UI flow

**Window:** 560×420 dp, Mica backdrop, custom titlebar identical in style to `SettingsWindow` (accent dot + "Plith" wordmark + "Setup" subtitle + close/minimize buttons). Centered on primary screen. Non-resizable, no maximize button — installer is a fixed-size dialog, not an app window.

**Welcome page (default state on launch):**

- Plith icon (64dp) centered top
- "Welcome to Plith" headline (TitleStyle)
- Subtitle: "Modern Windows audio OSD with Voicemeeter-first design."
- Primary button label depends on `InstallDetector` result:
  - No existing install → `"Install Plith"`
  - Same version detected → `"Reinstall Plith v0.1.0"`
  - Older version detected → `"Update Plith v0.1.0 → v0.2.0"`
- `▸ Advanced options` expander below button (chevron rotates on expand):
  - `☑ Game mode (UIAccess)  [?]` — default ON. Hint subtext: "OSD over fullscreen games"
  - `☑ Launch at Windows login` — default ON
  - `☑ Open Plith after install` — default ON
- `[?]` icon tooltip: *"Plith uses a self-signed certificate to enable UIAccess. The cert is only trusted on this machine."*

**Progress page:**

- "Installing Plith…" headline (or "Updating Plith…" for update mode)
- Vertical list of 5 steps, each row: status indicator (○ pending / ● running with pulse / ✓ done in accent green / ⚠ failed in WarningAmber) + step label
- Linear progress bar at bottom, accent green, weighted by step completion (each step = 20%)
- Close and minimize buttons disabled (window forced-modal during install — user can't accidentally cancel mid-cert-import)
- Esc key intercepted, swallowed

**Finish page:**

- Large 64dp accent green checkmark icon
- "Plith is ready" headline
- Conditional subtitle:
  - Game mode ON: "Game mode is active — OSD draws over fullscreen games."
  - Game mode OFF: "OSD draws over borderless fullscreen games."
- Primary button: `"Open Plith"` (shown if "Open Plith after install" was checked, otherwise omitted)
- Secondary row: `"View on GitHub"` + `"Close"` ghost buttons

**Error page:**

- "⚠ Install failed" headline in WarningAmber
- Failed step name + error message excerpt
- Buttons: `"Copy log"` (Clipboard.SetText with log contents) + `"Open log"` (Process.Start log file) + `"Close"`
- Log path: `%LOCALAPPDATA%\Plith\Installer\install.log`

**Page transitions:** SlideFadeIn from right, 250ms cubic ease-out. Welcome → Progress → Finish/Error is linear; no back navigation.

### Component 3 — Install pipeline (`InstallOrchestrator`)

Five sequential steps surfaced as `InstallStepViewModel` entries in Progress page:

1. **Setting up certificate** (CertService)
   - Look up `CN=Plith Self-Signed` in `CurrentUser\My`. If absent: generate via `RSA.Create(2048)` + `CertificateRequest("CN=Plith Self-Signed", rsa, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1)` + `X509KeyUsageExtension(DigitalSignature)` + `X509EnhancedKeyUsageExtension(CodeSigning OID 1.3.6.1.5.5.7.3.3)`, `req.CreateSelfSigned(notBefore: now, notAfter: now+5y)`, set `FriendlyName = "Plith Code Signing"`, persist to `CurrentUser\My`.
   - Export public cert. Import to `LocalMachine\TrustedPublisher` and `LocalMachine\Root` (both required for UIAccess validation per Phase 4h lessons learned).
   - Return thumbprint to orchestrator.

2. **Extracting Plith files** (EmbeddedExtractor)
   - Read `PlithBundle.zip` from `Assembly.GetExecutingAssembly().GetManifestResourceStream("PlithBundle.zip")`.
   - Extract via `ZipArchive.ExtractToDirectory` to `%LOCALAPPDATA%\Plith\Installer\stage\`.
   - Validates: `Plith.exe` exists in extract output.

3. **Signing executable** (SignToolWrapper)
   - Resolve `signtool.exe`: PATH first, then glob `${env:ProgramFiles(x86)}\Windows Kits\10\bin\**\x64\signtool.exe`, pick highest version.
   - If not found: throw `InvalidOperationException("signtool.exe not found. Install the Windows 10/11 SDK or VS Build Tools (workload: Desktop development with C++).")`.
   - Invoke: `signtool.exe sign /sha1 <thumbprint> /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 <stage>\Plith.exe`.
   - Capture stdout/stderr → LogService. Non-zero exit code → throw.

4. **Copying to Program Files**
   - Check + kill running `Plith` processes (Phase 4h lesson: file lock breaks mirror copy).
   - `Directory.CreateDirectory("C:\\Program Files\\Plith")` if missing.
   - Mirror copy from stage to target: traverse source recursively, `File.Copy(overwrite: true)` each, delete target-only files (mirror semantics).
   - **Copy the running installer itself** to `C:\Program Files\Plith\Setup\Plith-Uninstaller.exe` so Add/Remove Programs `UninstallString` has a stable target. (Source: `Environment.ProcessPath` of the currently running installer.)
   - Apply HKCU\Run autostart entry based on user's Advanced options choice.

5. **Registering Plith** (ShortcutService + RegistryService)
   - Create Start menu shortcut: `%ProgramData%\Microsoft\Windows\Start Menu\Programs\Plith.lnk` via `WScript.Shell` COM. TargetPath, WorkingDirectory, IconLocation, Description all set.
   - Write HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith with DisplayName, DisplayVersion (`FileVersionInfo.GetVersionInfo(installedExe).ProductVersion`), Publisher = "Plith Self-Signed", InstallLocation, DisplayIcon, EstimatedSize (computed from install dir bytes), NoModify=1, NoRepair=1, UninstallString = `"C:\Program Files\Plith\Setup\Plith-Uninstaller.exe" --uninstall`.

Each step is `async Task`. Failure in any step → orchestrator catches, marks step Failed, navigates MainWindow to ErrorPage. Idempotent: rerunning the installer after a failure repeats every step from the top — cert reuse is automatic, file copies overwrite, registry writes overwrite. No partial-state cleanup logic in this phase.

### Component 4 — Uninstall flow

`Plith-Uninstaller.exe --uninstall` (the same Setup.exe copied during install, renamed in target dir):

- App.xaml.cs detects `--uninstall` arg, routes MainWindow to a 2-page sub-flow:
  - **UninstallConfirmPage** — "Uninstall Plith?" headline, brief explanation, Uninstall + Cancel buttons.
  - **UninstallProgressPage** — 3-step list: "Stopping Plith", "Removing files", "Cleaning up registry". Same animated step list as install Progress.
  - **UninstallFinishPage** — "Plith uninstalled" + Close button.
- Removes: Program Files\Plith\ directory, Start menu shortcut, HKLM Uninstall key, HKCU\Run autostart entry.
- Leaves cert in CurrentUser\My + TrustedPublisher + Root (re-install one-step). Documented in README.

### Component 5 — Build pipeline + embedding

**Pre-build MSBuild target in Plith.Installer.csproj:**

```xml
<Target Name="PublishPlithAndEmbed" BeforeTargets="PrepareForBuild">
  <Exec Command="dotnet publish &quot;$(MSBuildThisFileDirectory)..\Plith\Plith.csproj&quot;
                 -c Release
                 -o &quot;$(MSBuildThisFileDirectory)Resources\Embedded\staging&quot;
                 -p:PublishSingleFile=false
                 -p:SelfContained=false" />
  <ZipDirectory SourceDirectory="$(MSBuildThisFileDirectory)Resources\Embedded\staging"
                DestinationFile="$(MSBuildThisFileDirectory)Resources\Embedded\PlithBundle.zip"
                Overwrite="true" />
  <RemoveDir Directories="$(MSBuildThisFileDirectory)Resources\Embedded\staging" />
</Target>

<ItemGroup>
  <EmbeddedResource Include="Resources\Embedded\PlithBundle.zip" LogicalName="PlithBundle.zip" />
</ItemGroup>
```

`dotnet build src/Plith.Installer` triggers Plith publish automatically, zips it, embeds it. No CI / scripts knowledge of internal ordering needed.

**Release artifact build (`scripts/build-release.ps1`):**

```powershell
# Tests must pass
dotnet test tests/Plith.Tests/Plith.Tests.csproj
dotnet test tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj

# Build single-file installer
dotnet publish src/Plith.Installer -c Release -r win-x64 `
    -p:PublishSingleFile=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:SelfContained=true `
    -p:EnableCompressionInSingleFile=true `
    -o release/

# Rename + sign
$version = (Get-Item release/Plith.Installer.exe).VersionInfo.ProductVersion
Move-Item release/Plith.Installer.exe "release/Plith-Setup-$version.exe"

# Resolve signtool, sign, timestamp
$signtool = ... # same logic as install pipeline
$thumb = (Get-ChildItem Cert:\CurrentUser\My | Where Subject -eq 'CN=Plith Self-Signed').Thumbprint
& $signtool sign /sha1 $thumb /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "release/Plith-Setup-$version.exe"

Write-Host "Release artifact ready: release/Plith-Setup-$version.exe"
```

The installer EXE itself is signed with the same self-signed cert. SmartScreen will still show "unknown publisher" the first download (self-signed CA is not a public root), but reduces the warning surface.

### Component 6 — Theme + asset sharing

Plith.Installer reuses the Settings palette without duplication:

```xml
<!-- Plith.Installer.csproj -->
<ItemGroup>
  <Page Include="..\Plith\Resources\Palette.Dark.xaml">
    <Link>Resources\Palette.Dark.xaml</Link>
  </Page>
  <Page Include="..\Plith\Resources\Theme.xaml">
    <Link>Resources\Theme.xaml</Link>
  </Page>
  <Resource Include="..\Plith\Resources\icons\plith.ico">
    <Link>Resources\icons\plith.ico</Link>
  </Resource>
</ItemGroup>
```

Linked file pattern — single source of truth in `src/Plith/Resources/`, installer compiles them into its own assembly. Any palette change in Plith automatically propagates to installer next build. Installer adds its own page-specific styles (Welcome icon size, Progress step list template, Finish hero checkmark) in a new `Resources/InstallerStyles.xaml`.

### Component 7 — Legacy script removal + README

Delete:
- `scripts/setup-cert.ps1`
- `scripts/install-local.ps1`
- `scripts/uninstall-local.ps1`

Keep / add:
- `scripts/build-release.ps1` — release builder, only thing left in scripts/.

README updates:
- "Game mode" section rewritten:
  - Old: "Run `pwsh scripts\install-local.ps1` as admin"
  - New: "Download `Plith-Setup-<version>.exe` from the latest release and double-click. The wizard handles cert setup, signing, and Program Files install automatically."
- Add "Uninstall" subsection: "Settings → Apps → Installed apps → Plith → Uninstall" or "double-click `Plith-Uninstaller.exe` in the Setup subfolder."
- Build-from-source section adds `scripts/build-release.ps1` reference for creating a custom installer artifact.

## Lifecycle / data flow

```
User double-clicks Plith-Setup-0.1.0.exe
  ├─ Windows UAC prompt (manifest requireAdministrator)
  ├─ App.OnStartup
  │    ├─ Single-instance mutex check
  │    ├─ Parse args: --uninstall ? route to UninstallFlow : route to InstallFlow
  │    └─ Create MainWindow + ContentControl host
  ├─ InstallDetector.GetExistingVersion() → null | "0.1.0"
  ├─ MainWindow.NavigateTo(WelcomePage)
  │    └─ WelcomeViewModel populates button label based on detector result
  └─ User clicks Install/Update
       ├─ MainWindow.NavigateTo(ProgressPage) with SlideFadeIn animation
       ├─ InstallOrchestrator.RunAsync(options)
       │    ├─ Step 1: CertService.EnsureCert() → thumbprint
       │    ├─ Step 2: EmbeddedExtractor.ExtractTo(stageDir)
       │    ├─ Step 3: SignToolWrapper.Sign(stageDir/Plith.exe, thumbprint)
       │    ├─ Step 4: File ops → copy stage → C:\Program Files\Plith
       │    │              + copy self to Setup\Plith-Uninstaller.exe
       │    │              + apply HKCU\Run if autostart option on
       │    └─ Step 5: ShortcutService + RegistryService
       ├─ Success path → MainWindow.NavigateTo(FinishPage)
       │    └─ If "Open Plith" checked: Process.Start via explorer.exe
       └─ Failure path → MainWindow.NavigateTo(ErrorPage)
            └─ LogService has full diagnostic info
```

## Error handling

- **Cert generation fails** (cryptographic exception, store access denied): throw → log → ErrorPage. User retries; if same error, README points at "ensure admin context."
- **Stage extraction fails** (out of disk, corrupt embed): throw → ErrorPage. Common cause: insufficient disk space (~150 MB needed for temp + install). Error message surfaces this.
- **signtool not found**: actionable error with SDK install instruction.
- **signtool exit non-zero**: stderr captured to log, generic error to UI ("Signing failed; see log").
- **Program Files copy fails** (locked file): kill-Plith logic in step 4 mitigates; if still locked (rare race with another tool), throw → ErrorPage.
- **Registry write fails** (HKLM admin lost mid-install, very rare): step 5 throws → ErrorPage. Files already in place; rerun completes idempotently.
- **Mid-install close attempt** (Alt+F4, close button): swallowed during Progress page. Window force-modal during install.
- **Logging**: every step start, end, exception → `%LOCALAPPDATA%\Plith\Installer\install.log` (append-only, timestamped). UI's "Copy log" and "Open log" buttons surface it on ErrorPage.

## Testing

**Unit (`tests/Plith.Installer.Tests/`, target 5-10 tests):**
- `CertService.EnsureCert()` idempotent: first call creates, second call reuses same thumbprint. Uses a real CurrentUser\My store + cleanup in test teardown.
- `InstallDetector.GetExistingVersion()` returns null when Plith.exe absent; returns parsed version string when present (use temp dir fixture).
- `EmbeddedExtractor` extracts a test ZIP to a temp dir, verifies expected files.

**Plith.Tests:** 36/36 must stay green. The installer doesn't touch any Plith source.

**Manual smoke matrix:**
- Fresh Windows VM: download Setup.exe → double-click → wizard appears → Install → success → Plith.exe in Program Files + Start menu + Add/Remove Programs → launch Plith from Start menu → badge "Active".
- Existing install: rerun Setup.exe → Welcome shows "Reinstall Plith v0.1.0" → Install → success → no duplicates.
- Update path: bump csproj version to 0.2.0 → rebuild Setup → run on machine with 0.1.0 → Welcome shows "Update Plith v0.1.0 → v0.2.0" → Update → success.
- Uninstall: Add/Remove Programs → Plith → Uninstall → wizard appears → Uninstall → Program Files\Plith gone, shortcut gone, registry gone, cert remains.
- Failure injection: rename signtool.exe → run installer → ErrorPage shows "signtool not found" with SDK hint.
- Anti-cheat (acceptance): Valorant practice → volume wheel → OSD pops (same as Phase 4h end state).

## Risk / open questions

- **Single-file publish + manifest interaction**: `PublishSingleFile=true` for Plith.Installer (the installer itself, not Plith) embeds the installer's `app.manifest` correctly via apphost. Verified against .NET 8/10 behavior — not the same trap as Plith's uiAccess concern.
- **SmartScreen reputation**: first download of a self-signed installer triggers "Microsoft Defender SmartScreen prevented…" dialog. User clicks "More info → Run anyway." Documented in README. Mitigation requires public CA cert, deferred.
- **Embedded ZIP size**: Plith publish is ~50-70 MB. Compressed inside the installer EXE adds maybe 30 MB of single-file overhead → ~80-100 MB total. Acceptable for a desktop installer.
- **Plith.Installer.exe locks during run**: Setup copies itself to `Setup\Plith-Uninstaller.exe` at install time. The running Setup.exe is still locked on disk (downloaded location), but `File.Copy(source: Environment.ProcessPath, destination: ...)` reads from the locked source and writes a new copy — supported by Windows.
- **Update mode mid-run Plith**: if user is running Plith when they run Setup to update, Step 4 kills the process. They lose any unsaved Settings state, but Plith auto-saves on every change so no real data loss.
- **Uninstaller deleting itself**: classic Windows pain point. Solution: Setup.exe in `Program Files\Plith\Setup\` calls `Directory.Delete("C:\\Program Files\\Plith\\", recursive: true)` at the end, but this includes the running uninstaller binary. Mitigation: spawn a `cmd /c timeout /t 2 && rd /s /q "C:\\Program Files\\Plith"` child process then `Environment.Exit(0)` — child outlives parent, deletes the now-defunct uninstaller. Documented pattern.
- **Anti-cheat**: same Phase 4h surface, no change. Installer doesn't touch this — just sets up the Plith binary that was already designed for Game mode.

## Out of scope

- WiX MSI installer (rejected explicitly during brainstorm — premium UX preferred).
- `--silent` headless mode (future addition once interactive flow is stable).
- Auto-update detection inside Plith ("a new version is available" toast in Settings) — separate phase.
- Public CA / EV signing — future commercial deployment decision.
- MSIX / Microsoft Store distribution.
- Multi-language UI (English only this phase).
- Cert renewal automation (5-year cert is sufficient for Phase 5 lifespan).
- Repair / Modify functionality from Add/Remove Programs (Uninstall is the only modify path).
- Per-user install (HKCU only). Stays admin-only for UIAccess.
