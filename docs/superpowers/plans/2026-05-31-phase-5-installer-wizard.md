# Phase 5 Installer Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a premium WPF setup wizard at `src/Plith.Installer/` that produces a single signed `Plith-Setup-X.Y.Z.exe` with embedded Plith binaries, three-page UX (Welcome → Progress → Finish), auto-detect existing install for Install/Reinstall/Update mode switching, dual-store cert handling, Start menu shortcut, Add/Remove Programs registration, and a matching uninstall sub-flow. Replaces the legacy PowerShell install scripts entirely.

**Architecture:** New WPF project alongside `src/Plith/`. MSBuild pre-build target publishes Plith Release, zips it, embeds as `PlithBundle.zip` resource. Installer runtime extracts → signs with auto-generated self-signed cert → mirrors to `\Program Files\Plith\` → registers Start menu + uninstall. Five install steps surfaced as animated `InstallStepViewModel` rows on `ProgressPage`. Plith.Installer.exe copies itself to `Setup\Plith-Uninstaller.exe` during install so Add/Remove Programs UninstallString has a stable target. Self-delete via spawned `cmd /c timeout && rd` child process.

**Tech Stack:** WPF, .NET 10 (`net10.0-windows10.0.22000.0`), x64. xUnit for `Plith.Installer.Tests`. Self-signed cert via `RSA.Create()` + `CertificateRequest` (built-in `System.Security.Cryptography.X509Certificates`). Embedded ZIP via `System.IO.Compression.ZipArchive`. `WScript.Shell` COM for Start menu .lnk. Direct registry access via `Microsoft.Win32.Registry`. Signtool from Windows SDK. PowerShell 7+ for `scripts/build-release.ps1`.

**Spec:** `docs/superpowers/specs/2026-05-31-phase-5-installer-wizard-design.md` (commit `019aca9`)

---

## File Structure

**Create — Plith.Installer project:**
- `src/Plith.Installer/Plith.Installer.csproj` — WPF SDK project, x64, NETAnalyzers strict, embedding target
- `src/Plith.Installer/app.manifest` — requireAdministrator, uiAccess=false, supportedOS Win10/11
- `src/Plith.Installer/App.xaml` + `App.xaml.cs` — single-instance, `--uninstall` flag routing
- `src/Plith.Installer/MainWindow.xaml` + `.cs` — custom titlebar + Mica + ContentControl page host
- `src/Plith.Installer/Pages/WelcomePage.xaml` + `.cs` — branding, Install/Update button, Advanced expander
- `src/Plith.Installer/Pages/ProgressPage.xaml` + `.cs` — animated step list, linear progress bar
- `src/Plith.Installer/Pages/FinishPage.xaml` + `.cs` — success state, Open Plith / GitHub / Close
- `src/Plith.Installer/Pages/ErrorPage.xaml` + `.cs` — error state, copy log / open log / close
- `src/Plith.Installer/Pages/UninstallConfirmPage.xaml` + `.cs` — confirm uninstall
- `src/Plith.Installer/Pages/UninstallProgressPage.xaml` + `.cs` — 3-step uninstall progress
- `src/Plith.Installer/Pages/UninstallFinishPage.xaml` + `.cs` — uninstall success
- `src/Plith.Installer/ViewModels/InstallerViewModel.cs` — state machine, Advanced options bound state
- `src/Plith.Installer/ViewModels/InstallStepViewModel.cs` — per-step status (Pending/Running/Done/Failed)
- `src/Plith.Installer/Services/LogService.cs` — append-only file logger
- `src/Plith.Installer/Services/CertService.cs` — cert gen + dual-store import
- `src/Plith.Installer/Services/EmbeddedExtractor.cs` — `PlithBundle.zip` resource → temp dir
- `src/Plith.Installer/Services/SignToolWrapper.cs` — signtool locate + invoke
- `src/Plith.Installer/Services/ShortcutService.cs` — Start menu .lnk via `WScript.Shell`
- `src/Plith.Installer/Services/RegistryService.cs` — Add/Remove Programs entry
- `src/Plith.Installer/Services/InstallDetector.cs` — existing install version detection
- `src/Plith.Installer/Services/InstallOrchestrator.cs` — pipeline composition (5 install + 3 uninstall steps)
- `src/Plith.Installer/Resources/InstallerStyles.xaml` — page-specific styles (step list template, hero icons)
- `src/Plith.Installer/Resources/Animations.xaml` — SlideFadeIn/Out storyboards

**Create — tests:**
- `tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj` — xUnit, net10.0-windows
- `tests/Plith.Installer.Tests/CertServiceTests.cs` — idempotency tests
- `tests/Plith.Installer.Tests/InstallDetectorTests.cs` — version detection
- `tests/Plith.Installer.Tests/EmbeddedExtractorTests.cs` — ZIP extraction
- `tests/Plith.Installer.Tests/TestHelpers/TempDirectory.cs` — IDisposable temp dir fixture
- `tests/Plith.Installer.Tests/TestHelpers/TempCertStore.cs` — per-test cert cleanup

**Create — scripts:**
- `scripts/build-release.ps1` — Test → publish single-file → rename → sign release artifact

**Modify:**
- `README.md` — replace Game mode install section with Plith-Setup.exe path; add Uninstall section
- `Plith.sln` (if exists) — add Plith.Installer and Plith.Installer.Tests references

**Delete:**
- `scripts/setup-cert.ps1`
- `scripts/install-local.ps1`
- `scripts/uninstall-local.ps1`

---

## Task 1: Scaffold Plith.Installer project + linked resources

Creates the WPF project skeleton with the csproj, manifest, and the directory structure. Links Palette.Dark.xaml + Theme.xaml + plith.ico from `src/Plith/Resources/` so the installer reuses Plith's brand exactly without duplication.

**Files:**
- Create: `src/Plith.Installer/Plith.Installer.csproj`
- Create: `src/Plith.Installer/app.manifest`
- Create: `src/Plith.Installer/Resources/InstallerStyles.xaml` (placeholder)
- Create: `src/Plith.Installer/Resources/Animations.xaml` (placeholder)

- [ ] **Step 1: Create folder structure**

```powershell
New-Item -ItemType Directory -Force -Path src\Plith.Installer\Pages
New-Item -ItemType Directory -Force -Path src\Plith.Installer\ViewModels
New-Item -ItemType Directory -Force -Path src\Plith.Installer\Services
New-Item -ItemType Directory -Force -Path src\Plith.Installer\Resources\Embedded
New-Item -ItemType Directory -Force -Path src\Plith.Installer\Services
```

- [ ] **Step 2: Write Plith.Installer.csproj**

Write `src/Plith.Installer/Plith.Installer.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <OutputType>WinExe</OutputType>
    <TargetFramework>net10.0-windows10.0.22000.0</TargetFramework>
    <SupportedOSPlatformVersion>10.0.22000.0</SupportedOSPlatformVersion>
    <RootNamespace>Plith.Installer</RootNamespace>
    <AssemblyName>Plith.Installer</AssemblyName>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <LangVersion>latest</LangVersion>
    <UseWPF>true</UseWPF>
    <UseWindowsForms>false</UseWindowsForms>
    <Platforms>x64</Platforms>
    <PlatformTarget>x64</PlatformTarget>
    <ApplicationManifest>app.manifest</ApplicationManifest>
    <ApplicationIcon>Resources\icons\plith.ico</ApplicationIcon>
    <Authors>Plith</Authors>
    <Product>Plith Setup</Product>
    <Description>Plith setup wizard.</Description>
    <Version>0.1.0</Version>
    <EnableNETAnalyzers>true</EnableNETAnalyzers>
    <AnalysisLevel>latest</AnalysisLevel>
    <AnalysisMode>Recommended</AnalysisMode>
    <!--
      Suppressions with justification:
      CA1707 — Win32 constants intentionally use ALL_CAPS_WITH_UNDERSCORES.
    -->
    <NoWarn>$(NoWarn);CA1707</NoWarn>
  </PropertyGroup>

  <!-- Linked resources from src/Plith — single source of truth -->
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

  <!-- Embedded resource placeholder — populated by PublishPlithAndEmbed target (Task 2) -->
  <ItemGroup>
    <None Include="Resources\Embedded\PlithBundle.zip" Condition="Exists('Resources\Embedded\PlithBundle.zip')">
      <CopyToOutputDirectory>Never</CopyToOutputDirectory>
    </None>
  </ItemGroup>

</Project>
```

- [ ] **Step 3: Write app.manifest**

Write `src/Plith.Installer/app.manifest`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <assemblyIdentity version="0.1.0.0" name="Plith.Installer" />

  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v2">
    <security>
      <requestedPrivileges xmlns="urn:schemas-microsoft-com:asm.v3">
        <!-- Installer needs admin: TrustedPublisher/Root store imports are HKLM, and
             Program Files copy requires elevated token. uiAccess=false because the
             installer itself isn't an overlay — it just sets one up. -->
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>

  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 / 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
    </application>
  </compatibility>

  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
```

- [ ] **Step 4: Write empty Animations.xaml placeholder**

Write `src/Plith.Installer/Resources/Animations.xaml`:

```xml
<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">

    <!-- SlideFadeIn from right (250ms cubic ease-out) — used for forward page navigation. -->
    <Storyboard x:Key="SlideFadeIn">
        <DoubleAnimation Storyboard.TargetProperty="(UIElement.RenderTransform).(TranslateTransform.X)"
                         From="32" To="0" Duration="0:0:0.25">
            <DoubleAnimation.EasingFunction>
                <CubicEase EasingMode="EaseOut" />
            </DoubleAnimation.EasingFunction>
        </DoubleAnimation>
        <DoubleAnimation Storyboard.TargetProperty="(UIElement.Opacity)"
                         From="0" To="1" Duration="0:0:0.25">
            <DoubleAnimation.EasingFunction>
                <CubicEase EasingMode="EaseOut" />
            </DoubleAnimation.EasingFunction>
        </DoubleAnimation>
    </Storyboard>

</ResourceDictionary>
```

- [ ] **Step 5: Write empty InstallerStyles.xaml placeholder**

Write `src/Plith.Installer/Resources/InstallerStyles.xaml`:

```xml
<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
                    xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">

    <!-- Page-specific installer styles populated by later tasks (step list template, hero icons,
         large accent button). Empty for now so MainWindow can reference it without errors. -->

</ResourceDictionary>
```

- [ ] **Step 6: Verify the project file is valid (restore + build will fail without app.xaml; this just confirms XML is valid)**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj /t:Restore
```

Expected: `Restore complete in ...` with no errors. (Build will fail later because we have no App.xaml yet — that's Task 3. We only check restore here.)

- [ ] **Step 7: Commit**

```bash
git add src/Plith.Installer/Plith.Installer.csproj src/Plith.Installer/app.manifest src/Plith.Installer/Resources/Animations.xaml src/Plith.Installer/Resources/InstallerStyles.xaml
git commit -m "$(cat <<'EOF'
feat(installer): scaffold Plith.Installer WPF project

New WPF project at src/Plith.Installer/ targeting net10.0-windows10.0.22000.0
with strict NETAnalyzers. Linked palette/theme/icon files from src/Plith so
the installer reuses Plith's brand without duplication. Manifest requests
admin (TrustedPublisher import + Program Files copy require it) and leaves
uiAccess=false — the installer isn't an overlay, just sets one up.
EOF
)"
```

---

## Task 2: MSBuild pre-build target — publish Plith and embed as ZIP

Adds the MSBuild target that publishes Plith Release and zips the output into `Resources/Embedded/PlithBundle.zip`. The zip becomes an embedded resource accessible at runtime via `Assembly.GetManifestResourceStream("PlithBundle.zip")`.

**Files:**
- Modify: `src/Plith.Installer/Plith.Installer.csproj` (add target + EmbeddedResource)
- Modify: `.gitignore` (ignore PlithBundle.zip — regenerated each build)

- [ ] **Step 1: Add MSBuild target to Plith.Installer.csproj**

Use Edit tool on `src/Plith.Installer/Plith.Installer.csproj`. Replace the existing `None Include="Resources\Embedded\PlithBundle.zip"` block:

```xml
  <!-- Embedded resource placeholder — populated by PublishPlithAndEmbed target (Task 2) -->
  <ItemGroup>
    <None Include="Resources\Embedded\PlithBundle.zip" Condition="Exists('Resources\Embedded\PlithBundle.zip')">
      <CopyToOutputDirectory>Never</CopyToOutputDirectory>
    </None>
  </ItemGroup>
```

with:

```xml
  <!-- Pre-build target: publish Plith Release, zip the output, embed as resource.
       Triggers on every build so the embedded bundle is always current. Out-of-date
       checking is delegated to the Exec invocation (dotnet publish is itself incremental). -->
  <Target Name="PublishPlithAndEmbed" BeforeTargets="PrepareForBuild">
    <PropertyGroup>
      <PlithStagingDir>$(MSBuildThisFileDirectory)Resources\Embedded\staging</PlithStagingDir>
      <PlithBundleZip>$(MSBuildThisFileDirectory)Resources\Embedded\PlithBundle.zip</PlithBundleZip>
    </PropertyGroup>

    <Exec Command="dotnet publish &quot;$(MSBuildThisFileDirectory)..\Plith\Plith.csproj&quot; -c Release -o &quot;$(PlithStagingDir)&quot; -p:PublishSingleFile=false -p:SelfContained=false"
          ConsoleToMSBuild="true" />

    <Delete Files="$(PlithBundleZip)" Condition="Exists('$(PlithBundleZip)')" />

    <ZipDirectory SourceDirectory="$(PlithStagingDir)"
                  DestinationFile="$(PlithBundleZip)" />

    <RemoveDir Directories="$(PlithStagingDir)" />
  </Target>

  <ItemGroup>
    <EmbeddedResource Include="Resources\Embedded\PlithBundle.zip" Condition="Exists('Resources\Embedded\PlithBundle.zip')">
      <LogicalName>PlithBundle.zip</LogicalName>
    </EmbeddedResource>
  </ItemGroup>
```

- [ ] **Step 2: Add PlithBundle.zip to .gitignore**

Use Edit tool on `.gitignore`. Append to the file (after the existing `scripts/.cert-thumbprint` line):

```
# Code-signing cert thumbprint state (regenerated by scripts/setup-cert.ps1)
scripts/.cert-thumbprint

# Installer's embedded bundle — regenerated from Plith publish on every build.
src/Plith.Installer/Resources/Embedded/PlithBundle.zip
src/Plith.Installer/Resources/Embedded/staging/
```

- [ ] **Step 3: Verify the target runs (build will still fail due to missing App.xaml; we look for the zip)**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-String -Pattern 'PublishPlithAndEmbed|PlithBundle.zip|error'
```

Expected: line shows the `PublishPlithAndEmbed` target executing, errors mention missing `App.xaml` / `MainWindow.xaml` (those come in next tasks).

Then verify the ZIP exists:

```powershell
Test-Path src\Plith.Installer\Resources\Embedded\PlithBundle.zip
```

Expected: `True`.

- [ ] **Step 4: Commit**

```bash
git add src/Plith.Installer/Plith.Installer.csproj .gitignore
git commit -m "$(cat <<'EOF'
feat(installer): MSBuild target to publish Plith and embed as PlithBundle.zip

PublishPlithAndEmbed target runs BeforeTargets=PrepareForBuild on every
Plith.Installer build, publishing Plith Release (multi-file) into a staging
dir, zipping it to Resources/Embedded/PlithBundle.zip, then deleting staging.
The zip is declared as an EmbeddedResource so it ships inside the installer
assembly and is accessible at runtime via Assembly.GetManifestResourceStream.

PlithBundle.zip and the staging dir are gitignored — both regenerable.
EOF
)"
```

---

## Task 3: App.xaml + MainWindow shell with custom titlebar and Mica

Minimum viable WPF shell so the project builds and a window appears. Custom titlebar matches Plith Settings — accent dot + "Plith" + "·" + "Setup" wordmark + minimize/close. Mica backdrop on Win11. Fixed 560×420 dp, centered. ContentControl in the middle for page hosting (filled in by later tasks).

**Files:**
- Create: `src/Plith.Installer/App.xaml` + `App.xaml.cs`
- Create: `src/Plith.Installer/MainWindow.xaml` + `MainWindow.xaml.cs`

- [ ] **Step 1: Write App.xaml**

Write `src/Plith.Installer/App.xaml`:

```xml
<Application x:Class="Plith.Installer.App"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             ShutdownMode="OnMainWindowClose">
    <Application.Resources>
        <ResourceDictionary>
            <ResourceDictionary.MergedDictionaries>
                <ResourceDictionary Source="Resources/Palette.Dark.xaml" />
                <ResourceDictionary Source="Resources/Theme.xaml" />
                <ResourceDictionary Source="Resources/Animations.xaml" />
                <ResourceDictionary Source="Resources/InstallerStyles.xaml" />
            </ResourceDictionary.MergedDictionaries>
        </ResourceDictionary>
    </Application.Resources>
</Application>
```

- [ ] **Step 2: Write App.xaml.cs**

Write `src/Plith.Installer/App.xaml.cs`:

```csharp
using System.Threading;
using System.Windows;

namespace Plith.Installer;

public partial class App : Application
{
    // Single-instance mutex — prevents two installer windows competing for the cert store
    // and the install dir. Mutex named with a unique GUID so it doesn't collide with any
    // other software using a "Plith" mutex name.
    private const string SingleInstanceMutexName = "Global\\Plith.Installer.SingleInstance.7F9C8E1A";
    private Mutex? _singleInstanceMutex;

    public bool IsUninstallMode { get; private set; }

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _singleInstanceMutex = new Mutex(initiallyOwned: true, SingleInstanceMutexName, out bool createdNew);
        if (!createdNew)
        {
            MessageBox.Show("Plith Setup is already running.",
                "Plith Setup", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown();
            return;
        }

        IsUninstallMode = e.Args.Length > 0 && e.Args[0] == "--uninstall";

        var window = new MainWindow();
        window.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _singleInstanceMutex?.ReleaseMutex();
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
```

- [ ] **Step 3: Write MainWindow.xaml**

Write `src/Plith.Installer/MainWindow.xaml`:

```xml
<Window x:Class="Plith.Installer.MainWindow"
        xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        xmlns:shell="clr-namespace:System.Windows.Shell;assembly=PresentationFramework"
        Title="Plith Setup"
        Width="560" Height="420"
        MinWidth="560" MinHeight="420"
        MaxWidth="560" MaxHeight="420"
        WindowStartupLocation="CenterScreen"
        WindowStyle="None"
        ResizeMode="NoResize"
        Background="{DynamicResource WindowBg}"
        Foreground="{DynamicResource TextPrimary}"
        FontFamily="{StaticResource UiFont}"
        TextOptions.TextFormattingMode="Ideal"
        TextOptions.TextRenderingMode="ClearType"
        UseLayoutRounding="True">
    <shell:WindowChrome.WindowChrome>
        <shell:WindowChrome CaptionHeight="44"
                            GlassFrameThickness="0"
                            UseAeroCaptionButtons="False"
                            CornerRadius="0"
                            ResizeBorderThickness="0" />
    </shell:WindowChrome.WindowChrome>

    <Grid>
        <Grid.RowDefinitions>
            <RowDefinition Height="44" />
            <RowDefinition Height="*" />
        </Grid.RowDefinitions>

        <!-- Custom titlebar -->
        <Border Grid.Row="0" Background="{DynamicResource HeaderBg}">
            <Grid>
                <Grid.ColumnDefinitions>
                    <ColumnDefinition Width="*" />
                    <ColumnDefinition Width="Auto" />
                </Grid.ColumnDefinitions>

                <StackPanel Grid.Column="0"
                            Orientation="Horizontal"
                            VerticalAlignment="Center"
                            Margin="18,0,0,0">
                    <Border Width="6" Height="6"
                            CornerRadius="3"
                            Background="{DynamicResource Accent}"
                            VerticalAlignment="Center"
                            Margin="0,0,10,0" />
                    <TextBlock Text="Plith"
                               FontFamily="{StaticResource UiFont}"
                               FontWeight="SemiBold"
                               FontSize="13"
                               Foreground="{DynamicResource TextPrimary}"
                               VerticalAlignment="Center" />
                    <TextBlock Text="·"
                               Foreground="{DynamicResource TextTertiary}"
                               Margin="8,0"
                               VerticalAlignment="Center" />
                    <TextBlock Text="Setup"
                               FontFamily="{StaticResource UiFont}"
                               FontSize="13"
                               Foreground="{DynamicResource TextSecondary}"
                               VerticalAlignment="Center" />
                </StackPanel>

                <StackPanel Grid.Column="1"
                            Orientation="Horizontal"
                            HorizontalAlignment="Right"
                            shell:WindowChrome.IsHitTestVisibleInChrome="True">
                    <Button x:Name="MinimizeButton"
                            Style="{StaticResource CaptionButtonStyle}"
                            Content="&#xE921;"
                            ToolTip="Minimize" />
                    <Button x:Name="CloseButton"
                            Style="{StaticResource CloseCaptionButtonStyle}"
                            Content="&#xE8BB;"
                            ToolTip="Close" />
                </StackPanel>
            </Grid>
        </Border>

        <!-- Page host. Pages swap in via PageHost.Content = <page instance> -->
        <ContentControl x:Name="PageHost" Grid.Row="1" />
    </Grid>
</Window>
```

- [ ] **Step 4: Write MainWindow.xaml.cs (shell only — page navigation impl in later tasks)**

Write `src/Plith.Installer/MainWindow.xaml.cs`:

```csharp
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace Plith.Installer;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        MinimizeButton.Click += (_, _) => WindowState = WindowState.Minimized;
        CloseButton.Click += (_, _) => Close();

        SourceInitialized += (_, _) =>
        {
            ApplyRoundedCorners();
            ApplyImmersiveDarkMode();
            ApplyMica();
        };

        // Page navigation (NavigateTo) will be wired up by the orchestrator in a later task.
        // For now MainWindow just hosts an empty ContentControl named PageHost.
    }

    /// <summary>Replace the current page in the host. Used by App.xaml.cs and orchestrator.</summary>
    public void NavigateTo(System.Windows.Controls.UserControl page)
    {
        PageHost.Content = page;
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint hwnd, int attr, ref int value, int size);

    private const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;
    private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    private const int DWMWA_SYSTEMBACKDROP_TYPE = 38;
    private const int DWMWCP_ROUND = 2;
    private const int DWMSBT_MAINWINDOW = 2; // Mica

    private void ApplyRoundedCorners()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int pref = DWMWCP_ROUND;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, ref pref, sizeof(int));
    }

    private void ApplyImmersiveDarkMode()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int dark = 1;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref dark, sizeof(int));
    }

    private void ApplyMica()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int backdrop = DWMSBT_MAINWINDOW;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, ref backdrop, sizeof(int));
    }
}
```

- [ ] **Step 5: Build — should succeed now**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 10
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 6: Smoke test — launch the empty installer window**

```powershell
dotnet run --project src\Plith.Installer
```

Expected: window appears, 560×420 with custom titlebar (accent dot + "Plith · Setup"), Mica/dark backdrop, minimize + close buttons functional. ContentControl area is empty (no page yet). Close window. The launch must be elevated; if not (manifest already says requireAdministrator), Windows triggers UAC.

- [ ] **Step 7: Commit**

```bash
git add src/Plith.Installer/App.xaml src/Plith.Installer/App.xaml.cs src/Plith.Installer/MainWindow.xaml src/Plith.Installer/MainWindow.xaml.cs
git commit -m "$(cat <<'EOF'
feat(installer): App.xaml + MainWindow shell with Mica titlebar

Custom titlebar matches Plith Settings — accent dot + "Plith · Setup"
wordmark + minimize/close. Mica backdrop, immersive dark mode, rounded
corners via DwmSetWindowAttribute. Fixed 560x420 dp window, centered,
non-resizable. Single-instance mutex prevents two installers competing
for the cert store. --uninstall flag flips App.IsUninstallMode for later
routing. ContentControl named PageHost waits for pages from later tasks.
EOF
)"
```

---

## Task 4: LogService

Append-only file logger. Used by all services and the orchestrator. Writes to `%LOCALAPPDATA%\Plith\Installer\install.log` (creates dirs as needed). Thread-safe — wraps writes in a lock since steps can run on the dispatcher and exceptions can hit a background thread.

**Files:**
- Create: `src/Plith.Installer/Services/LogService.cs`

- [ ] **Step 1: Write LogService.cs**

Write `src/Plith.Installer/Services/LogService.cs`:

```csharp
using System.Globalization;
using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Append-only diagnostic log for the installer. Lives at
/// %LOCALAPPDATA%\Plith\Installer\install.log so the ErrorPage's "Open log" and
/// "Copy log" buttons can surface it on failure. Per-write lock; install steps
/// run on the dispatcher but exceptions can hit a background thread.
/// </summary>
public sealed class LogService
{
    private readonly string _logPath;
    private readonly object _lock = new();

    public string LogPath => _logPath;

    public LogService()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Plith", "Installer");
        Directory.CreateDirectory(dir);
        _logPath = Path.Combine(dir, "install.log");
    }

    public void Info(string message) => Write("INFO", message);

    public void Error(string step, Exception ex)
    {
        Write("ERROR", $"step={step} type={ex.GetType().Name} message={ex.Message}");
        Write("ERROR", $"stack={ex.StackTrace}");
    }

    public string ReadAll()
    {
        lock (_lock)
        {
            try { return File.ReadAllText(_logPath); }
            catch { return string.Empty; }
        }
    }

    private void Write(string level, string message)
    {
        var line = string.Format(CultureInfo.InvariantCulture,
            "[{0:yyyy-MM-ddTHH:mm:ss.fffZ}] [{1}] {2}\r\n",
            DateTime.UtcNow, level, message);
        lock (_lock)
        {
            try { File.AppendAllText(_logPath, line); }
            catch { /* logging must never crash the installer */ }
        }
    }
}
```

- [ ] **Step 2: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 3: Commit**

```bash
git add src/Plith.Installer/Services/LogService.cs
git commit -m "$(cat <<'EOF'
feat(installer): LogService — append-only diagnostic log

Writes timestamped entries to %LOCALAPPDATA%\Plith\Installer\install.log.
Info() and Error(step, exception) entry points. ErrorPage's Copy/Open log
buttons read from LogPath. All writes wrapped in a lock so background-thread
exceptions don't tear a half-written line.
EOF
)"
```

---

## Task 5: Tests project scaffold + TestHelpers

Spins up `tests/Plith.Installer.Tests/` with xUnit + the two reusable test helpers (`TempDirectory` IDisposable, `TempCertStore` per-test cleanup). No production tests yet — they come in Tasks 6, 7, 8.

**Files:**
- Create: `tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj`
- Create: `tests/Plith.Installer.Tests/TestHelpers/TempDirectory.cs`
- Create: `tests/Plith.Installer.Tests/TestHelpers/TempCertStore.cs`
- Create: `tests/Plith.Installer.Tests/PlaceholderTest.cs` (deleted in Task 6 when first real test arrives)

- [ ] **Step 1: Create test project folder**

```powershell
New-Item -ItemType Directory -Force -Path tests\Plith.Installer.Tests\TestHelpers
```

- [ ] **Step 2: Write tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj**

Write `tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj`:

```xml
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net10.0-windows10.0.22000.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>enable</ImplicitUsings>
    <LangVersion>latest</LangVersion>
    <IsPackable>false</IsPackable>
    <UseWPF>false</UseWPF>
    <Platforms>x64</Platforms>
    <PlatformTarget>x64</PlatformTarget>
    <EnableNETAnalyzers>true</EnableNETAnalyzers>
    <AnalysisLevel>latest</AnalysisLevel>
    <AnalysisMode>Recommended</AnalysisMode>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.11.1" />
    <PackageReference Include="xunit" Version="2.9.2" />
    <PackageReference Include="xunit.runner.visualstudio" Version="2.8.2" />
  </ItemGroup>

  <ItemGroup>
    <ProjectReference Include="..\..\src\Plith.Installer\Plith.Installer.csproj" />
  </ItemGroup>

</Project>
```

- [ ] **Step 3: Write TempDirectory test helper**

Write `tests/Plith.Installer.Tests/TestHelpers/TempDirectory.cs`:

```csharp
using System.IO;

namespace Plith.Installer.Tests.TestHelpers;

/// <summary>Disposable temp directory for tests. Deletes itself on Dispose even if
/// the test left files inside. Use with `using var dir = new TempDirectory();`.</summary>
public sealed class TempDirectory : IDisposable
{
    public string Path { get; }

    public TempDirectory()
    {
        Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(),
            "Plith.Installer.Tests." + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path);
    }

    public void Dispose()
    {
        try { Directory.Delete(Path, recursive: true); } catch { /* best-effort */ }
    }
}
```

- [ ] **Step 4: Write TempCertStore test helper**

Write `tests/Plith.Installer.Tests/TestHelpers/TempCertStore.cs`:

```csharp
using System.Security.Cryptography.X509Certificates;

namespace Plith.Installer.Tests.TestHelpers;

/// <summary>Removes any leftover Plith test certs from the user's CurrentUser\My store
/// on Dispose. Tests can run safely without polluting the real cert store.</summary>
public sealed class TempCertStore : IDisposable
{
    public const string TestSubject = "CN=Plith Test " + nameof(TempCertStore);

    public void Dispose() => RemoveAll();

    public static void RemoveAll()
    {
        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadWrite);
        var toRemove = store.Certificates.Find(X509FindType.FindBySubjectName,
            "Plith Test", validOnly: false);
        if (toRemove.Count > 0) store.RemoveRange(toRemove);
        store.Close();
    }
}
```

- [ ] **Step 5: Write a placeholder test so the project builds + runs**

Write `tests/Plith.Installer.Tests/PlaceholderTest.cs`:

```csharp
namespace Plith.Installer.Tests;

public class PlaceholderTest
{
    // Placeholder so xUnit discovery runs. Deleted in Task 6 when real tests arrive.
    [Xunit.Fact]
    public void Project_builds_and_xunit_discovers_tests() => Xunit.Assert.True(true);
}
```

- [ ] **Step 6: Verify test project builds and the placeholder test runs**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 1, Failed: 0, Skipped: 0, Total: 1`.

- [ ] **Step 7: Commit**

```bash
git add tests/Plith.Installer.Tests/
git commit -m "$(cat <<'EOF'
feat(installer-tests): xUnit test project scaffold + TestHelpers

New tests/Plith.Installer.Tests/ project: net10.0-windows, xUnit 2.9, project
reference to Plith.Installer. TempDirectory (disposable scratch dir) and
TempCertStore (cleans up Plith test certs on dispose) helpers. PlaceholderTest
proves discovery works; deleted when the first real service test lands.
EOF
)"
```

---

## Task 6: CertService + tests (TDD)

Generates a self-signed code-signing cert via `RSA.Create()` + `CertificateRequest` (no PowerShell shell-out needed — .NET has the full API). Imports public cert into `LocalMachine\TrustedPublisher` AND `LocalMachine\Root` (Phase 4h proved both are needed). Idempotent — if a Plith cert already exists in `CurrentUser\My`, reuse its thumbprint.

**Files:**
- Create: `src/Plith.Installer/Services/CertService.cs`
- Create: `tests/Plith.Installer.Tests/CertServiceTests.cs`
- Delete: `tests/Plith.Installer.Tests/PlaceholderTest.cs`

- [ ] **Step 1: Write the failing test**

Write `tests/Plith.Installer.Tests/CertServiceTests.cs`:

```csharp
using System.Security.Cryptography.X509Certificates;
using Plith.Installer.Services;
using Plith.Installer.Tests.TestHelpers;
using Xunit;

namespace Plith.Installer.Tests;

public class CertServiceTests : IDisposable
{
    private readonly TempCertStore _certCleanup = new();

    public void Dispose() => _certCleanup.Dispose();

    [Fact]
    public void EnsureCert_creates_cert_when_none_exists_and_returns_thumbprint()
    {
        TempCertStore.RemoveAll();   // ensure clean slate
        var svc = new CertService(subjectName: TempCertStore.TestSubject);

        var thumbprint = svc.EnsureCert();

        Assert.NotNull(thumbprint);
        Assert.Equal(40, thumbprint.Length);   // SHA-1 thumbprint hex = 40 chars

        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadOnly);
        var found = store.Certificates.Find(X509FindType.FindByThumbprint, thumbprint, validOnly: false);
        Assert.Single(found);
    }

    [Fact]
    public void EnsureCert_returns_existing_thumbprint_when_cert_already_present()
    {
        TempCertStore.RemoveAll();
        var svc = new CertService(subjectName: TempCertStore.TestSubject);
        var first = svc.EnsureCert();

        var second = svc.EnsureCert();

        Assert.Equal(first, second);
    }
}
```

- [ ] **Step 2: Delete the placeholder test (no longer needed)**

```powershell
Remove-Item tests\Plith.Installer.Tests\PlaceholderTest.cs
```

- [ ] **Step 3: Run test to verify it fails (no CertService yet)**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 10
```

Expected: build fails with `CS0246: The type or namespace name 'CertService' could not be found`.

- [ ] **Step 4: Write CertService.cs**

Write `src/Plith.Installer/Services/CertService.cs`:

```csharp
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

namespace Plith.Installer.Services;

/// <summary>
/// Self-signed code-signing cert lifecycle. EnsureCert returns the thumbprint of a
/// usable cert — reusing an existing CN=Plith Self-Signed entry in CurrentUser\My
/// when present, or generating a new 5-year cert otherwise. Imports the public cert
/// into BOTH LocalMachine\TrustedPublisher AND LocalMachine\Root so the UIAccess
/// chain validates (Phase 4h lessons learned: TrustedPublisher alone is not enough).
/// </summary>
public sealed class CertService
{
    public const string DefaultSubject = "CN=Plith Self-Signed";

    private static readonly Oid CodeSigningOid = new("1.3.6.1.5.5.7.3.3");
    private readonly string _subject;

    public CertService(string? subjectName = null)
    {
        _subject = subjectName ?? DefaultSubject;
    }

    /// <summary>Find or create a usable cert; ensure it's also in LocalMachine\TrustedPublisher
    /// + LocalMachine\Root; return its SHA-1 thumbprint.</summary>
    public string EnsureCert()
    {
        var cert = FindExisting() ?? CreateAndPersist();
        EnsureInLocalMachineStore(cert, StoreName.TrustedPublisher);
        EnsureInLocalMachineStore(cert, StoreName.Root);
        return cert.Thumbprint!;
    }

    private X509Certificate2? FindExisting()
    {
        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadOnly);
        foreach (var existing in store.Certificates)
        {
            if (existing.Subject == _subject && existing.NotAfter > DateTime.UtcNow)
                return existing;
        }
        return null;
    }

    private X509Certificate2 CreateAndPersist()
    {
        using var rsa = RSA.Create(2048);
        var req = new CertificateRequest(_subject, rsa, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1);
        req.CertificateExtensions.Add(new X509KeyUsageExtension(
            X509KeyUsageFlags.DigitalSignature, critical: true));
        req.CertificateExtensions.Add(new X509EnhancedKeyUsageExtension(
            new OidCollection { CodeSigningOid }, critical: true));

        var notBefore = DateTimeOffset.UtcNow.AddMinutes(-5);   // tolerate slight clock skew
        var notAfter = notBefore.AddYears(5);
        var cert = req.CreateSelfSigned(notBefore, notAfter);
        cert.FriendlyName = "Plith Code Signing";

        // Re-load with persisted private key. CreateSelfSigned returns an ephemeral key by
        // default; signing tools and SignTool need the key to be in the user's key store.
        var pfx = cert.Export(X509ContentType.Pfx);
        var persisted = X509CertificateLoader.LoadPkcs12(pfx, password: null,
            X509KeyStorageFlags.PersistKeySet | X509KeyStorageFlags.UserKeySet | X509KeyStorageFlags.Exportable);

        using var store = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        store.Open(OpenFlags.ReadWrite);
        store.Add(persisted);
        return persisted;
    }

    private static void EnsureInLocalMachineStore(X509Certificate2 cert, StoreName storeName)
    {
        using var store = new X509Store(storeName, StoreLocation.LocalMachine);
        store.Open(OpenFlags.ReadWrite);
        var existing = store.Certificates.Find(X509FindType.FindByThumbprint, cert.Thumbprint!, validOnly: false);
        if (existing.Count > 0) return;

        // Public-only copy — never put the private key in LocalMachine stores.
        var publicOnly = new X509Certificate2(cert.Export(X509ContentType.Cert));
        store.Add(publicOnly);
    }
}
```

- [ ] **Step 5: Run tests — should pass**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 2, Failed: 0`.

- [ ] **Step 6: Verify Plith.Tests still green**

```powershell
dotnet test tests\Plith.Tests\Plith.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 36, Failed: 0`.

- [ ] **Step 7: Commit**

```bash
git add src/Plith.Installer/Services/CertService.cs tests/Plith.Installer.Tests/CertServiceTests.cs tests/Plith.Installer.Tests/PlaceholderTest.cs
git commit -m "$(cat <<'EOF'
feat(installer): CertService — self-signed cert with dual-store import

Pure-.NET cert lifecycle via RSA.Create() + CertificateRequest — no shell-out
to PowerShell. EnsureCert finds an existing CN=Plith Self-Signed in
CurrentUser\My (still valid) or generates a new 5-year cert with the
CodeSigning EKU (OID 1.3.6.1.5.5.7.3.3). Reloads with PersistKeySet so
SignTool can read the private key. Imports the public cert into BOTH
LocalMachine\TrustedPublisher and LocalMachine\Root — Phase 4h taught
that both stores are required for UIAccess chain validation.

Tests prove idempotency (second call reuses thumbprint) and that the cert
lands in CurrentUser\My. TempCertStore fixture cleans up between runs.
EOF
)"
```

---

## Task 7: InstallDetector + tests

Reads `FileVersionInfo` of `C:\Program Files\Plith\Plith.exe` to detect an existing install. Returns the parsed `Version` or `null`. Used by WelcomePage to decide the primary button label (Install / Reinstall / Update).

**Files:**
- Create: `src/Plith.Installer/Services/InstallDetector.cs`
- Create: `tests/Plith.Installer.Tests/InstallDetectorTests.cs`

- [ ] **Step 1: Write the failing tests**

Write `tests/Plith.Installer.Tests/InstallDetectorTests.cs`:

```csharp
using System.IO;
using Plith.Installer.Services;
using Plith.Installer.Tests.TestHelpers;
using Xunit;

namespace Plith.Installer.Tests;

public class InstallDetectorTests
{
    [Fact]
    public void GetInstalledVersion_returns_null_when_install_dir_missing()
    {
        using var dir = new TempDirectory();
        var fakeExe = Path.Combine(dir.Path, "MissingPlith.exe");
        var detector = new InstallDetector(fakeExe);

        var version = detector.GetInstalledVersion();

        Assert.Null(version);
    }

    [Fact]
    public void GetInstalledVersion_returns_version_when_exe_present_with_version_info()
    {
        // We can use the currently-running test host as a stand-in — it has FileVersionInfo
        // and exists on disk. Asserting the parsed version is just "not null + non-empty"
        // because we don't control the test host's version.
        var testHostExe = System.Diagnostics.Process.GetCurrentProcess().MainModule!.FileName;
        var detector = new InstallDetector(testHostExe);

        var version = detector.GetInstalledVersion();

        Assert.NotNull(version);
        Assert.False(string.IsNullOrEmpty(version));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (no InstallDetector yet)**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-String -Pattern 'InstallDetector|error CS'
```

Expected: build failure mentioning `InstallDetector`.

- [ ] **Step 3: Write InstallDetector.cs**

Write `src/Plith.Installer/Services/InstallDetector.cs`:

```csharp
using System.Diagnostics;
using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Detects whether Plith is already installed at the standard location and reads its
/// version. WelcomePage uses this to switch the primary button label between
/// "Install Plith" / "Reinstall Plith vX.Y.Z" / "Update Plith vX.Y.Z → vN.M.P".
/// </summary>
public sealed class InstallDetector
{
    public static readonly string DefaultInstalledExePath =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            "Plith", "Plith.exe");

    private readonly string _installedExePath;

    public InstallDetector(string? installedExePath = null)
    {
        _installedExePath = installedExePath ?? DefaultInstalledExePath;
    }

    public string InstalledExePath => _installedExePath;

    /// <summary>Returns the ProductVersion of the installed Plith.exe, or null if not installed.</summary>
    public string? GetInstalledVersion()
    {
        if (!File.Exists(_installedExePath)) return null;
        try
        {
            var info = FileVersionInfo.GetVersionInfo(_installedExePath);
            return info.ProductVersion ?? info.FileVersion;
        }
        catch
        {
            return null;
        }
    }
}
```

- [ ] **Step 4: Run tests — should pass**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 4, Failed: 0`.

- [ ] **Step 5: Commit**

```bash
git add src/Plith.Installer/Services/InstallDetector.cs tests/Plith.Installer.Tests/InstallDetectorTests.cs
git commit -m "$(cat <<'EOF'
feat(installer): InstallDetector — read existing Plith.exe ProductVersion

Reads FileVersionInfo of the standard install path
(C:\Program Files\Plith\Plith.exe), returns ProductVersion or null. Used by
WelcomePage to switch the primary button label between Install / Reinstall /
Update based on whether (and which version of) Plith is already installed.
Constructor takes an optional path for testability.
EOF
)"
```

---

## Task 8: EmbeddedExtractor + tests

Reads the embedded `PlithBundle.zip` resource and extracts to a target dir. Used by Step 2 of the install pipeline. Validates that the extracted output contains `Plith.exe` — if not, the bundle was built wrong.

**Files:**
- Create: `src/Plith.Installer/Services/EmbeddedExtractor.cs`
- Create: `tests/Plith.Installer.Tests/EmbeddedExtractorTests.cs`

- [ ] **Step 1: Write the failing tests**

Write `tests/Plith.Installer.Tests/EmbeddedExtractorTests.cs`:

```csharp
using System.IO;
using System.IO.Compression;
using Plith.Installer.Services;
using Plith.Installer.Tests.TestHelpers;
using Xunit;

namespace Plith.Installer.Tests;

public class EmbeddedExtractorTests
{
    [Fact]
    public void Extract_writes_zip_contents_to_target_dir()
    {
        using var bundleDir = new TempDirectory();
        using var targetDir = new TempDirectory();

        // Build a fake bundle.zip with a Plith.exe stub.
        var zipPath = Path.Combine(bundleDir.Path, "fake-bundle.zip");
        using (var zipStream = File.Create(zipPath))
        using (var zip = new ZipArchive(zipStream, ZipArchiveMode.Create))
        {
            var entry = zip.CreateEntry("Plith.exe");
            using var writer = new StreamWriter(entry.Open());
            writer.Write("stub");
        }

        var extractor = EmbeddedExtractor.FromStream(File.OpenRead(zipPath));
        extractor.ExtractTo(targetDir.Path);

        Assert.True(File.Exists(Path.Combine(targetDir.Path, "Plith.exe")));
    }

    [Fact]
    public void Extract_throws_when_bundle_lacks_Plith_exe()
    {
        using var bundleDir = new TempDirectory();
        using var targetDir = new TempDirectory();

        var zipPath = Path.Combine(bundleDir.Path, "bad-bundle.zip");
        using (var zipStream = File.Create(zipPath))
        using (var zip = new ZipArchive(zipStream, ZipArchiveMode.Create))
        {
            zip.CreateEntry("README.txt");
        }

        var extractor = EmbeddedExtractor.FromStream(File.OpenRead(zipPath));

        Assert.Throws<InvalidDataException>(() => extractor.ExtractTo(targetDir.Path));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (no EmbeddedExtractor yet)**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-String -Pattern 'EmbeddedExtractor|error CS'
```

Expected: build failure mentioning `EmbeddedExtractor`.

- [ ] **Step 3: Write EmbeddedExtractor.cs**

Write `src/Plith.Installer/Services/EmbeddedExtractor.cs`:

```csharp
using System.IO;
using System.IO.Compression;
using System.Reflection;

namespace Plith.Installer.Services;

/// <summary>
/// Extracts the embedded PlithBundle.zip resource (the Plith Release publish output) into
/// a target directory. Validates that the extracted output contains Plith.exe so a
/// corrupt or wrong-content bundle fails fast instead of producing a broken install.
/// </summary>
public sealed class EmbeddedExtractor
{
    public const string BundleResourceName = "PlithBundle.zip";

    private readonly Stream _bundleStream;

    private EmbeddedExtractor(Stream bundleStream)
    {
        _bundleStream = bundleStream;
    }

    /// <summary>Factory for production use — pulls the zip from the installer assembly's
    /// embedded resources.</summary>
    public static EmbeddedExtractor FromEmbeddedResource()
    {
        var assembly = Assembly.GetExecutingAssembly();
        var stream = assembly.GetManifestResourceStream(BundleResourceName)
            ?? throw new InvalidOperationException(
                $"Embedded resource '{BundleResourceName}' not found. " +
                "The pre-build target failed to populate Resources/Embedded/.");
        return new EmbeddedExtractor(stream);
    }

    /// <summary>Factory for tests — pass any stream containing a valid bundle.</summary>
    public static EmbeddedExtractor FromStream(Stream stream) => new(stream);

    public void ExtractTo(string targetDir)
    {
        Directory.CreateDirectory(targetDir);
        using (var archive = new ZipArchive(_bundleStream, ZipArchiveMode.Read))
        {
            archive.ExtractToDirectory(targetDir, overwriteFiles: true);
        }

        var plithExe = Path.Combine(targetDir, "Plith.exe");
        if (!File.Exists(plithExe))
            throw new InvalidDataException(
                $"Bundle is missing Plith.exe (expected at '{plithExe}'). " +
                "The pre-build embedding pipeline did not include the main executable.");
    }
}
```

- [ ] **Step 4: Run tests — should pass**

```powershell
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 6, Failed: 0`.

- [ ] **Step 5: Commit**

```bash
git add src/Plith.Installer/Services/EmbeddedExtractor.cs tests/Plith.Installer.Tests/EmbeddedExtractorTests.cs
git commit -m "$(cat <<'EOF'
feat(installer): EmbeddedExtractor — extract PlithBundle.zip resource

Reads the embedded PlithBundle.zip (populated by the pre-build target) and
extracts to a target dir via ZipArchive.ExtractToDirectory. Throws
InvalidDataException if the extracted output is missing Plith.exe — fails fast
on a wrong-content bundle instead of producing a broken install.

Two factories: FromEmbeddedResource() for production, FromStream() for tests.
EOF
)"
```

---

## Task 9: SignToolWrapper + ShortcutService + RegistryService

Win32/COM glue services. No unit tests — they're thin wrappers around `Process.Start`, `WScript.Shell` COM, and `Microsoft.Win32.Registry`. Manual smoke covers them via the full install flow.

**Files:**
- Create: `src/Plith.Installer/Services/SignToolWrapper.cs`
- Create: `src/Plith.Installer/Services/ShortcutService.cs`
- Create: `src/Plith.Installer/Services/RegistryService.cs`

- [ ] **Step 1: Write SignToolWrapper.cs**

Write `src/Plith.Installer/Services/SignToolWrapper.cs`:

```csharp
using System.Diagnostics;
using System.IO;
using System.Linq;

namespace Plith.Installer.Services;

/// <summary>
/// Locates signtool.exe (PATH first, Windows SDK fallback) and invokes it to sign a binary
/// with a thumbprint-referenced cert. Throws actionable errors when the tool can't be found
/// or when the signing call fails.
/// </summary>
public sealed class SignToolWrapper
{
    private readonly LogService _log;

    public SignToolWrapper(LogService log)
    {
        _log = log;
    }

    /// <summary>Sign the given exe with the cert identified by SHA-1 thumbprint.
    /// Uses SHA-256 file digest + timestamps via digicert.com.</summary>
    public void Sign(string exePath, string certThumbprint)
    {
        var signtool = ResolveSignToolPath()
            ?? throw new InvalidOperationException(
                "signtool.exe not found. Install the Windows 10/11 SDK or VS Build Tools " +
                "(workload: 'Desktop development with C++') and re-run.");

        _log.Info($"signtool: using '{signtool}'");
        _log.Info($"signtool: signing '{exePath}' with thumbprint {certThumbprint}");

        var psi = new ProcessStartInfo(signtool)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
        };
        psi.ArgumentList.Add("sign");
        psi.ArgumentList.Add("/sha1");
        psi.ArgumentList.Add(certThumbprint);
        psi.ArgumentList.Add("/fd");
        psi.ArgumentList.Add("SHA256");
        psi.ArgumentList.Add("/tr");
        psi.ArgumentList.Add("http://timestamp.digicert.com");
        psi.ArgumentList.Add("/td");
        psi.ArgumentList.Add("SHA256");
        psi.ArgumentList.Add(exePath);

        using var proc = Process.Start(psi)
            ?? throw new InvalidOperationException("Failed to launch signtool.");
        string stdout = proc.StandardOutput.ReadToEnd();
        string stderr = proc.StandardError.ReadToEnd();
        proc.WaitForExit();

        if (!string.IsNullOrWhiteSpace(stdout)) _log.Info("signtool stdout: " + stdout.Trim());
        if (!string.IsNullOrWhiteSpace(stderr)) _log.Info("signtool stderr: " + stderr.Trim());

        if (proc.ExitCode != 0)
            throw new InvalidOperationException(
                $"signtool exited with code {proc.ExitCode}. See install.log.");
    }

    private static string? ResolveSignToolPath()
    {
        // 1. PATH lookup
        var pathDirs = Environment.GetEnvironmentVariable("PATH")?.Split(Path.PathSeparator)
            ?? Array.Empty<string>();
        foreach (var dir in pathDirs)
        {
            var candidate = Path.Combine(dir, "signtool.exe");
            if (File.Exists(candidate)) return candidate;
        }

        // 2. Windows SDK fallback
        var sdkRoot = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86),
            "Windows Kits", "10", "bin");
        if (!Directory.Exists(sdkRoot)) return null;

        return Directory.EnumerateFiles(sdkRoot, "signtool.exe", SearchOption.AllDirectories)
            .Where(p => p.Contains(@"\x64\", StringComparison.OrdinalIgnoreCase))
            .OrderByDescending(p => p)
            .FirstOrDefault();
    }
}
```

- [ ] **Step 2: Write ShortcutService.cs**

Write `src/Plith.Installer/Services/ShortcutService.cs`:

```csharp
using System.IO;

namespace Plith.Installer.Services;

/// <summary>
/// Creates and removes the Start menu .lnk via WScript.Shell COM. The shortcut lives in
/// %ProgramData%\Microsoft\Windows\Start Menu\Programs so all users see it (matches
/// the all-users install posture). Removing the .lnk on uninstall is a single File.Delete.
/// </summary>
public sealed class ShortcutService
{
    public static readonly string StartMenuShortcutPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
        "Programs", "Plith.lnk");

    public void CreateStartMenuShortcut(string targetExePath, string description)
    {
        var wshType = Type.GetTypeFromProgID("WScript.Shell")
            ?? throw new InvalidOperationException("WScript.Shell COM type not available.");
        dynamic shell = Activator.CreateInstance(wshType)!;
        dynamic shortcut = shell.CreateShortcut(StartMenuShortcutPath);
        shortcut.TargetPath = targetExePath;
        shortcut.WorkingDirectory = Path.GetDirectoryName(targetExePath) ?? string.Empty;
        shortcut.IconLocation = targetExePath;
        shortcut.Description = description;
        shortcut.Save();
    }

    public void RemoveStartMenuShortcut()
    {
        if (File.Exists(StartMenuShortcutPath))
            File.Delete(StartMenuShortcutPath);
    }
}
```

- [ ] **Step 3: Write RegistryService.cs**

Write `src/Plith.Installer/Services/RegistryService.cs`:

```csharp
using System.IO;
using Microsoft.Win32;

namespace Plith.Installer.Services;

/// <summary>
/// Manages the Add/Remove Programs registry entry under HKLM\...\Uninstall\Plith,
/// plus the per-user HKCU\...\Run autostart entry. Idempotent.
/// </summary>
public sealed class RegistryService
{
    private const string UninstallKeyPath = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith";
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";

    /// <summary>Writes the Add/Remove Programs entry. UninstallString points at the
    /// installer copied to ProgramFiles\Plith\Setup\Plith-Uninstaller.exe with --uninstall.</summary>
    public void WriteUninstallEntry(string installDir, string installedExePath, string version, string uninstallerPath)
    {
        using var key = Registry.LocalMachine.CreateSubKey(UninstallKeyPath, writable: true)
            ?? throw new InvalidOperationException(
                "Failed to create HKLM\\...\\Uninstall\\Plith — admin required?");

        key.SetValue("DisplayName", "Plith", RegistryValueKind.String);
        key.SetValue("DisplayVersion", version, RegistryValueKind.String);
        key.SetValue("Publisher", "Plith Self-Signed", RegistryValueKind.String);
        key.SetValue("InstallLocation", installDir, RegistryValueKind.String);
        key.SetValue("DisplayIcon", installedExePath, RegistryValueKind.String);
        key.SetValue("UninstallString", $"\"{uninstallerPath}\" --uninstall", RegistryValueKind.String);
        key.SetValue("NoModify", 1, RegistryValueKind.DWord);
        key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
        key.SetValue("EstimatedSize", ComputeEstimatedKb(installDir), RegistryValueKind.DWord);
    }

    public void RemoveUninstallEntry()
    {
        Registry.LocalMachine.DeleteSubKeyTree(UninstallKeyPath, throwOnMissingSubKey: false);
    }

    public void WriteAutoStart(string installedExePath)
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        key?.SetValue("Plith", $"\"{installedExePath}\"", RegistryValueKind.String);
    }

    public void RemoveAutoStart()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        key?.DeleteValue("Plith", throwOnMissingValue: false);
    }

    private static int ComputeEstimatedKb(string dir)
    {
        if (!Directory.Exists(dir)) return 0;
        long bytes = 0;
        foreach (var file in Directory.EnumerateFiles(dir, "*", SearchOption.AllDirectories))
        {
            try { bytes += new FileInfo(file).Length; } catch { /* skip unreadable */ }
        }
        return (int)(bytes / 1024);
    }
}
```

- [ ] **Step 4: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 5: Commit**

```bash
git add src/Plith.Installer/Services/SignToolWrapper.cs src/Plith.Installer/Services/ShortcutService.cs src/Plith.Installer/Services/RegistryService.cs
git commit -m "$(cat <<'EOF'
feat(installer): SignToolWrapper + ShortcutService + RegistryService

Three thin Win32/COM glue services:
- SignToolWrapper: PATH-first signtool lookup with Windows SDK fallback;
  invokes via ProcessStartInfo ArgumentList; stdout/stderr captured to log.
- ShortcutService: WScript.Shell COM creates/removes %ProgramData%\Start
  Menu Plith.lnk so the all-users install posture is consistent.
- RegistryService: HKLM Add/Remove Programs entry (DisplayName, version,
  publisher, UninstallString pointing at the copied uninstaller binary)
  + HKCU\Run autostart toggle.

No unit tests — thin wrappers around platform APIs covered by manual install smoke.
EOF
)"
```

---

## Task 10: ViewModels — InstallStepViewModel + InstallerViewModel

Two viewmodels:
- `InstallStepViewModel` — per-step status (Pending/Running/Done/Failed) + Title. Bound by ProgressPage and UninstallProgressPage to render the animated step list.
- `InstallerViewModel` — top-level state machine. Holds Advanced options (Game mode / autostart / open-after), detected existing version, and the current page (Welcome / Progress / Finish / Error).

**Files:**
- Create: `src/Plith.Installer/ViewModels/InstallStepViewModel.cs`
- Create: `src/Plith.Installer/ViewModels/InstallerViewModel.cs`

- [ ] **Step 1: Write InstallStepViewModel.cs**

Write `src/Plith.Installer/ViewModels/InstallStepViewModel.cs`:

```csharp
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Plith.Installer.ViewModels;

public enum InstallStepStatus
{
    Pending,
    Running,
    Done,
    Failed,
}

public sealed class InstallStepViewModel : INotifyPropertyChanged
{
    private string _title = string.Empty;
    private InstallStepStatus _status = InstallStepStatus.Pending;
    private string? _failureMessage;

    public string Title
    {
        get => _title;
        set { if (_title != value) { _title = value; OnPropertyChanged(); } }
    }

    public InstallStepStatus Status
    {
        get => _status;
        set { if (_status != value) { _status = value; OnPropertyChanged(); } }
    }

    public string? FailureMessage
    {
        get => _failureMessage;
        set { if (_failureMessage != value) { _failureMessage = value; OnPropertyChanged(); } }
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
```

- [ ] **Step 2: Write InstallerViewModel.cs**

Write `src/Plith.Installer/ViewModels/InstallerViewModel.cs`:

```csharp
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace Plith.Installer.ViewModels;

public enum InstallerMode
{
    FreshInstall,
    Reinstall,
    Update,
}

public sealed class InstallerViewModel : INotifyPropertyChanged
{
    private InstallerMode _mode = InstallerMode.FreshInstall;
    private string? _existingVersion;
    private string _newVersion = "0.1.0";
    private bool _gameModeEnabled = true;
    private bool _autoStartEnabled = true;
    private bool _openAfterInstall = true;
    private double _progress;
    private string _errorMessage = string.Empty;
    private string _failedStepTitle = string.Empty;

    public ObservableCollection<InstallStepViewModel> Steps { get; } = new();

    public InstallerMode Mode
    {
        get => _mode;
        set { _mode = value; OnPropertyChanged(); OnPropertyChanged(nameof(PrimaryButtonLabel)); }
    }

    public string? ExistingVersion
    {
        get => _existingVersion;
        set { _existingVersion = value; OnPropertyChanged(); OnPropertyChanged(nameof(PrimaryButtonLabel)); }
    }

    public string NewVersion
    {
        get => _newVersion;
        set { _newVersion = value; OnPropertyChanged(); OnPropertyChanged(nameof(PrimaryButtonLabel)); }
    }

    public bool GameModeEnabled
    {
        get => _gameModeEnabled;
        set { _gameModeEnabled = value; OnPropertyChanged(); }
    }

    public bool AutoStartEnabled
    {
        get => _autoStartEnabled;
        set { _autoStartEnabled = value; OnPropertyChanged(); }
    }

    public bool OpenAfterInstall
    {
        get => _openAfterInstall;
        set { _openAfterInstall = value; OnPropertyChanged(); }
    }

    public double Progress
    {
        get => _progress;
        set { _progress = value; OnPropertyChanged(); }
    }

    public string ErrorMessage
    {
        get => _errorMessage;
        set { _errorMessage = value; OnPropertyChanged(); }
    }

    public string FailedStepTitle
    {
        get => _failedStepTitle;
        set { _failedStepTitle = value; OnPropertyChanged(); }
    }

    /// <summary>Computed text for the Welcome page primary button.</summary>
    public string PrimaryButtonLabel => Mode switch
    {
        InstallerMode.Reinstall => $"Reinstall Plith v{ExistingVersion}",
        InstallerMode.Update => $"Update Plith v{ExistingVersion} → v{NewVersion}",
        _ => "Install Plith",
    };

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
```

- [ ] **Step 3: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 4: Commit**

```bash
git add src/Plith.Installer/ViewModels/
git commit -m "$(cat <<'EOF'
feat(installer): InstallStepViewModel + InstallerViewModel

InstallStepViewModel: per-step status (Pending/Running/Done/Failed) + Title
+ optional FailureMessage. Bound by ProgressPage / UninstallProgressPage to
render the animated step list.

InstallerViewModel: top-level state. Holds Mode (FreshInstall / Reinstall /
Update) + ExistingVersion / NewVersion for the computed PrimaryButtonLabel
("Install Plith" / "Reinstall Plith v0.1.0" / "Update Plith v0.1.0 → v0.2.0").
Advanced options bound state (GameModeEnabled, AutoStartEnabled,
OpenAfterInstall), progress 0..1, and ErrorMessage / FailedStepTitle for
the Error page handoff.
EOF
)"
```

---

## Task 11: InstallOrchestrator — five-step install pipeline

Composes all the services into the 5-step install pipeline. Each step updates the corresponding `InstallStepViewModel` (Running → Done) and the `InstallerViewModel.Progress` percentage. Catches exceptions, marks the failing step, surfaces details into `InstallerViewModel.ErrorMessage`, and re-throws so the caller can navigate to ErrorPage.

**Files:**
- Create: `src/Plith.Installer/Services/InstallOrchestrator.cs`

- [ ] **Step 1: Write InstallOrchestrator.cs**

Write `src/Plith.Installer/Services/InstallOrchestrator.cs`:

```csharp
using System.Diagnostics;
using System.IO;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Services;

public sealed class InstallOrchestrator
{
    public const string InstallDir = @"C:\Program Files\Plith";
    public static readonly string InstalledExe = Path.Combine(InstallDir, "Plith.exe");
    public static readonly string UninstallerDir = Path.Combine(InstallDir, "Setup");
    public static readonly string UninstallerExe = Path.Combine(UninstallerDir, "Plith-Uninstaller.exe");
    private static readonly string StageDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Plith", "Installer", "stage");

    private readonly LogService _log;
    private readonly CertService _cert;
    private readonly SignToolWrapper _signtool;
    private readonly ShortcutService _shortcut;
    private readonly RegistryService _registry;
    private readonly InstallerViewModel _vm;

    public InstallOrchestrator(LogService log, CertService cert, SignToolWrapper signtool,
        ShortcutService shortcut, RegistryService registry, InstallerViewModel vm)
    {
        _log = log;
        _cert = cert;
        _signtool = signtool;
        _shortcut = shortcut;
        _registry = registry;
        _vm = vm;
    }

    public void PrepareSteps()
    {
        _vm.Steps.Clear();
        _vm.Steps.Add(new InstallStepViewModel { Title = "Setting up certificate" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Extracting Plith files" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Signing executable" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Copying to Program Files" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Registering Plith" });
        _vm.Progress = 0;
    }

    public async Task RunInstallAsync()
    {
        _log.Info("Install: starting");
        try
        {
            string thumbprint = await RunStep(0, () => _cert.EnsureCert());
            await RunStep(1, () => ExtractBundle());
            await RunStep(2, () => _signtool.Sign(Path.Combine(StageDir, "Plith.exe"), thumbprint));
            await RunStep(3, () => CopyToProgramFiles());
            await RunStep(4, () => RegisterPlith());
            _log.Info("Install: done");
        }
        catch (Exception ex)
        {
            _log.Error("Install pipeline", ex);
            throw;
        }
    }

    private async Task<T> RunStep<T>(int stepIndex, Func<T> action)
    {
        var step = _vm.Steps[stepIndex];
        step.Status = InstallStepStatus.Running;
        try
        {
            var result = await Task.Run(action);
            step.Status = InstallStepStatus.Done;
            _vm.Progress = (stepIndex + 1) / (double)_vm.Steps.Count;
            return result;
        }
        catch (Exception ex)
        {
            step.Status = InstallStepStatus.Failed;
            step.FailureMessage = ex.Message;
            _vm.FailedStepTitle = step.Title;
            _vm.ErrorMessage = ex.Message;
            throw;
        }
    }

    private async Task RunStep(int stepIndex, Action action)
        => await RunStep(stepIndex, () => { action(); return true; });

    private void ExtractBundle()
    {
        if (Directory.Exists(StageDir)) Directory.Delete(StageDir, recursive: true);
        Directory.CreateDirectory(StageDir);
        var extractor = EmbeddedExtractor.FromEmbeddedResource();
        extractor.ExtractTo(StageDir);
    }

    private void CopyToProgramFiles()
    {
        foreach (var proc in Process.GetProcessesByName("Plith"))
        {
            try { proc.Kill(entireProcessTree: true); proc.WaitForExit(2000); } catch { }
        }

        Directory.CreateDirectory(InstallDir);
        Directory.CreateDirectory(UninstallerDir);

        MirrorCopy(StageDir, InstallDir);
        File.Copy(Environment.ProcessPath!, UninstallerExe, overwrite: true);

        if (_vm.AutoStartEnabled) _registry.WriteAutoStart(InstalledExe);
        else _registry.RemoveAutoStart();
    }

    private void RegisterPlith()
    {
        _shortcut.CreateStartMenuShortcut(InstalledExe,
            "Modern Windows audio OSD with Voicemeeter-first design and media controls.");

        var versionInfo = FileVersionInfo.GetVersionInfo(InstalledExe);
        var version = versionInfo.ProductVersion ?? versionInfo.FileVersion ?? "0.0.0";

        _registry.WriteUninstallEntry(InstallDir, InstalledExe, version, UninstallerExe);
    }

    private static void MirrorCopy(string source, string target)
    {
        Directory.CreateDirectory(target);

        foreach (var sourceFile in Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories))
        {
            var relative = Path.GetRelativePath(source, sourceFile);
            var targetFile = Path.Combine(target, relative);
            Directory.CreateDirectory(Path.GetDirectoryName(targetFile)!);
            File.Copy(sourceFile, targetFile, overwrite: true);
        }

        // Mirror semantics: delete target-only files (except the Setup\ subdir which holds
        // the uninstaller we copy in after this method).
        foreach (var targetFile in Directory.EnumerateFiles(target, "*", SearchOption.AllDirectories))
        {
            if (targetFile.StartsWith(Path.Combine(target, "Setup"), StringComparison.OrdinalIgnoreCase))
                continue;
            var relative = Path.GetRelativePath(target, targetFile);
            var sourceFile = Path.Combine(source, relative);
            if (!File.Exists(sourceFile)) File.Delete(targetFile);
        }
    }

    public void PrepareUninstallSteps()
    {
        _vm.Steps.Clear();
        _vm.Steps.Add(new InstallStepViewModel { Title = "Stopping Plith" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Removing files" });
        _vm.Steps.Add(new InstallStepViewModel { Title = "Cleaning up registry" });
        _vm.Progress = 0;
    }

    public async Task RunUninstallAsync()
    {
        _log.Info("Uninstall: starting");
        try
        {
            await RunStep(0, () =>
            {
                foreach (var proc in Process.GetProcessesByName("Plith"))
                {
                    try { proc.Kill(entireProcessTree: true); proc.WaitForExit(2000); } catch { }
                }
            });

            await RunStep(1, () =>
            {
                _shortcut.RemoveStartMenuShortcut();
                // Spawn the self-delete child process for InstallDir — runs AFTER this process exits.
                // The child waits 3 s then removes Program Files\Plith\ including this uninstaller binary.
                SpawnSelfDelete();
            });

            await RunStep(2, () =>
            {
                _registry.RemoveAutoStart();
                _registry.RemoveUninstallEntry();
            });

            _log.Info("Uninstall: done");
        }
        catch (Exception ex)
        {
            _log.Error("Uninstall pipeline", ex);
            throw;
        }
    }

    private static void SpawnSelfDelete()
    {
        // Spawned cmd.exe outlives this process (Plith-Uninstaller.exe) and deletes the
        // install dir which contains this binary. Standard Windows uninstaller pattern.
        var psi = new ProcessStartInfo("cmd.exe")
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            WindowStyle = ProcessWindowStyle.Hidden,
        };
        psi.ArgumentList.Add("/c");
        psi.ArgumentList.Add($"timeout /t 3 /nobreak >nul && rd /s /q \"{InstallDir}\"");
        Process.Start(psi);
    }
}
```

- [ ] **Step 2: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 3: Commit**

```bash
git add src/Plith.Installer/Services/InstallOrchestrator.cs
git commit -m "$(cat <<'EOF'
feat(installer): InstallOrchestrator — 5-step install + 3-step uninstall pipeline

Composes Cert / Extractor / SignTool / Shortcut / Registry services into
sequential async steps, each updating an InstallStepViewModel
(Pending → Running → Done | Failed) plus the linear progress percentage.
Caught exceptions surface step name + message into InstallerViewModel for
ErrorPage; re-thrown so MainWindow can navigate.

CopyToProgramFiles kills any running Plith, mirrors stage to
\Program Files\Plith\ (mirror semantics: delete target-only files except
the Setup\ subdir), copies the running installer to Setup\Plith-Uninstaller.exe
so Add/Remove Programs has a stable UninstallString target.

Uninstall pipeline mirrors with 3 steps. Self-delete via spawned
`cmd /c timeout 3 && rd /s /q <installdir>` child process — child outlives
the uninstaller binary it's about to delete.
EOF
)"
```

---

## Task 12: WelcomePage — branding, primary button, Advanced options expander

The most complex page. Renders the InstallerViewModel state: PrimaryButtonLabel computed from Mode + ExistingVersion + NewVersion. Advanced options Expander with three checkboxes bound to InstallerViewModel. Small `?` info tooltip next to Game mode label.

**Files:**
- Create: `src/Plith.Installer/Pages/WelcomePage.xaml` + `.cs`

- [ ] **Step 1: Write WelcomePage.xaml**

Write `src/Plith.Installer/Pages/WelcomePage.xaml`:

```xml
<UserControl x:Class="Plith.Installer.Pages.WelcomePage"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             RenderTransformOrigin="0.5,0.5">
    <UserControl.RenderTransform>
        <TranslateTransform />
    </UserControl.RenderTransform>
    <Grid Margin="32,24,32,24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="*" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <!-- Icon -->
        <Image Grid.Row="0"
               Source="pack://application:,,,/Resources/icons/plith.ico"
               Width="64" Height="64"
               HorizontalAlignment="Center"
               Margin="0,4,0,12" />

        <!-- Headline -->
        <TextBlock Grid.Row="1"
                   Text="Welcome to Plith"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="22"
                   FontWeight="SemiBold"
                   Foreground="{DynamicResource TextPrimary}"
                   HorizontalAlignment="Center"
                   Margin="0,0,0,6" />

        <!-- Subtitle -->
        <TextBlock Grid.Row="2"
                   Text="Modern Windows audio OSD with Voicemeeter-first design."
                   FontFamily="{StaticResource UiFont}"
                   FontSize="13"
                   Foreground="{DynamicResource TextSecondary}"
                   HorizontalAlignment="Center"
                   TextAlignment="Center"
                   TextWrapping="Wrap"
                   Margin="0,0,0,18" />

        <!-- Spacer eats vertical room so button + expander hug the bottom -->

        <!-- Primary button -->
        <Button Grid.Row="4"
                x:Name="PrimaryButton"
                Content="{Binding PrimaryButtonLabel}"
                Style="{StaticResource AccentButtonStyle}"
                MinHeight="42"
                Margin="0,0,0,12" />

        <!-- Advanced options expander -->
        <Expander Grid.Row="5"
                  x:Name="AdvancedExpander"
                  Header="Advanced options"
                  Foreground="{DynamicResource TextSecondary}"
                  FontFamily="{StaticResource UiFont}"
                  FontSize="12"
                  IsExpanded="False">
            <StackPanel Margin="0,8,0,0">
                <Grid Margin="0,4">
                    <Grid.ColumnDefinitions>
                        <ColumnDefinition Width="*" />
                        <ColumnDefinition Width="Auto" />
                    </Grid.ColumnDefinitions>
                    <StackPanel Grid.Column="0" Orientation="Horizontal">
                        <CheckBox x:Name="GameModeCheck"
                                  Content="Game mode (UIAccess)"
                                  IsChecked="{Binding GameModeEnabled}"
                                  Foreground="{DynamicResource TextPrimary}" />
                        <TextBlock Text="?"
                                   FontWeight="Bold"
                                   Foreground="{DynamicResource TextTertiary}"
                                   Margin="6,0,0,0"
                                   ToolTip="Plith uses a self-signed certificate to enable UIAccess. The cert is only trusted on this machine." />
                    </StackPanel>
                </Grid>
                <TextBlock Text="OSD over fullscreen games"
                           FontSize="11"
                           Foreground="{DynamicResource TextTertiary}"
                           Margin="22,-2,0,8" />

                <CheckBox x:Name="AutoStartCheck"
                          Content="Launch at Windows login"
                          IsChecked="{Binding AutoStartEnabled}"
                          Foreground="{DynamicResource TextPrimary}"
                          Margin="0,4" />

                <CheckBox x:Name="OpenAfterCheck"
                          Content="Open Plith after install"
                          IsChecked="{Binding OpenAfterInstall}"
                          Foreground="{DynamicResource TextPrimary}"
                          Margin="0,4" />
            </StackPanel>
        </Expander>
    </Grid>
</UserControl>
```

- [ ] **Step 2: Write WelcomePage.xaml.cs**

Write `src/Plith.Installer/Pages/WelcomePage.xaml.cs`:

```csharp
using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class WelcomePage : UserControl
{
    public event EventHandler? PrimaryClicked;

    public WelcomePage(InstallerViewModel vm)
    {
        InitializeComponent();
        DataContext = vm;

        PrimaryButton.Click += (_, _) => PrimaryClicked?.Invoke(this, EventArgs.Empty);

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb)
                sb.Begin(this);
        };
    }
}
```

- [ ] **Step 3: Add `AccentButtonStyle` to InstallerStyles.xaml so the button renders**

Use Edit tool on `src/Plith.Installer/Resources/InstallerStyles.xaml`. Replace the empty body:

```xml
    <!-- Page-specific installer styles populated by later tasks (step list template, hero icons,
         large accent button). Empty for now so MainWindow can reference it without errors. -->
```

with:

```xml
    <!-- Page-specific installer styles. -->

    <!-- Large primary action button — accent green fill, white text. Used on Welcome,
         Finish (Open Plith), UninstallConfirm. -->
    <Style x:Key="AccentButtonStyle" TargetType="Button">
        <Setter Property="Background" Value="{DynamicResource Accent}" />
        <Setter Property="Foreground" Value="#FFFFFF" />
        <Setter Property="FontFamily" Value="{StaticResource UiFont}" />
        <Setter Property="FontSize" Value="14" />
        <Setter Property="FontWeight" Value="SemiBold" />
        <Setter Property="BorderThickness" Value="0" />
        <Setter Property="Padding" Value="20,10" />
        <Setter Property="Cursor" Value="Hand" />
        <Setter Property="Template">
            <Setter.Value>
                <ControlTemplate TargetType="Button">
                    <Border x:Name="bg"
                            Background="{TemplateBinding Background}"
                            CornerRadius="6">
                        <ContentPresenter HorizontalAlignment="Center"
                                          VerticalAlignment="Center" />
                    </Border>
                    <ControlTemplate.Triggers>
                        <Trigger Property="IsMouseOver" Value="True">
                            <Setter TargetName="bg" Property="Background" Value="{DynamicResource AccentHover}" />
                        </Trigger>
                        <Trigger Property="IsPressed" Value="True">
                            <Setter TargetName="bg" Property="Background" Value="{DynamicResource AccentPressed}" />
                        </Trigger>
                    </ControlTemplate.Triggers>
                </ControlTemplate>
            </Setter.Value>
        </Setter>
    </Style>

    <!-- Ghost button — outlined, accent-on-hover. For Cancel, View on GitHub, Close on
         Finish/Error pages. -->
    <Style x:Key="GhostButtonStyle" TargetType="Button">
        <Setter Property="Background" Value="Transparent" />
        <Setter Property="Foreground" Value="{DynamicResource TextSecondary}" />
        <Setter Property="FontFamily" Value="{StaticResource UiFont}" />
        <Setter Property="FontSize" Value="13" />
        <Setter Property="FontWeight" Value="Medium" />
        <Setter Property="BorderThickness" Value="1" />
        <Setter Property="BorderBrush" Value="{DynamicResource CardBorderStrong}" />
        <Setter Property="Padding" Value="14,8" />
        <Setter Property="Cursor" Value="Hand" />
        <Setter Property="Template">
            <Setter.Value>
                <ControlTemplate TargetType="Button">
                    <Border x:Name="bg"
                            Background="{TemplateBinding Background}"
                            BorderBrush="{TemplateBinding BorderBrush}"
                            BorderThickness="{TemplateBinding BorderThickness}"
                            CornerRadius="6">
                        <ContentPresenter HorizontalAlignment="Center"
                                          VerticalAlignment="Center" />
                    </Border>
                    <ControlTemplate.Triggers>
                        <Trigger Property="IsMouseOver" Value="True">
                            <Setter TargetName="bg" Property="Background" Value="{DynamicResource OverlayHover}" />
                        </Trigger>
                        <Trigger Property="IsPressed" Value="True">
                            <Setter TargetName="bg" Property="Background" Value="{DynamicResource OverlayPressed}" />
                        </Trigger>
                    </ControlTemplate.Triggers>
                </ControlTemplate>
            </Setter.Value>
        </Setter>
    </Style>
```

- [ ] **Step 4: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 5: Commit**

```bash
git add src/Plith.Installer/Pages/WelcomePage.xaml src/Plith.Installer/Pages/WelcomePage.xaml.cs src/Plith.Installer/Resources/InstallerStyles.xaml
git commit -m "$(cat <<'EOF'
feat(installer): WelcomePage with Advanced options expander

Renders InstallerViewModel: icon 64dp, "Welcome to Plith" headline, subtitle,
primary button bound to PrimaryButtonLabel (Install / Reinstall vX.Y.Z /
Update vX.Y.Z → vN.M.P), and a collapsed Expander with three checkboxes
(Game mode + ? info tooltip, Launch at Windows login, Open Plith after install).
SlideFadeIn storyboard on Loaded.

InstallerStyles.xaml gets AccentButtonStyle and GhostButtonStyle — both used
by Welcome / Finish / Error pages downstream.
EOF
)"
```

---

## Task 13: ProgressPage with animated step list

Renders the `Steps` ObservableCollection from InstallerViewModel as a vertical list, each row showing status indicator (○ pending / ● running / ✓ done / ⚠ failed) + Title. Linear progress bar at the bottom.

**Files:**
- Create: `src/Plith.Installer/Pages/ProgressPage.xaml` + `.cs`
- Modify: `src/Plith.Installer/Resources/InstallerStyles.xaml` (add StepStatusTemplate)

- [ ] **Step 1: Add step status template to InstallerStyles.xaml**

Use Edit tool on `src/Plith.Installer/Resources/InstallerStyles.xaml`. Append the new style after the existing `GhostButtonStyle`:

```xml
    <!-- Step row template for ProgressPage / UninstallProgressPage. The dot color and glyph
         depend on InstallStepViewModel.Status. -->
    <DataTemplate x:Key="StepRowTemplate">
        <Grid Margin="0,6">
            <Grid.ColumnDefinitions>
                <ColumnDefinition Width="24" />
                <ColumnDefinition Width="*" />
            </Grid.ColumnDefinitions>

            <!-- Status indicator. We render one of four glyphs/dots based on Status -->
            <Grid Grid.Column="0" Width="14" Height="14" HorizontalAlignment="Center">
                <!-- Pending: hollow circle -->
                <Ellipse Width="10" Height="10"
                         Stroke="{DynamicResource TextTertiary}"
                         StrokeThickness="1.5"
                         Visibility="{Binding Status, Converter={StaticResource StatusToVis}, ConverterParameter=Pending}" />
                <!-- Running: filled accent dot with pulsing scale -->
                <Ellipse Width="10" Height="10"
                         Fill="{DynamicResource Accent}"
                         RenderTransformOrigin="0.5,0.5"
                         Visibility="{Binding Status, Converter={StaticResource StatusToVis}, ConverterParameter=Running}">
                    <Ellipse.RenderTransform>
                        <ScaleTransform />
                    </Ellipse.RenderTransform>
                    <Ellipse.Triggers>
                        <EventTrigger RoutedEvent="Ellipse.Loaded">
                            <BeginStoryboard>
                                <Storyboard RepeatBehavior="Forever" AutoReverse="True">
                                    <DoubleAnimation Storyboard.TargetProperty="(UIElement.RenderTransform).(ScaleTransform.ScaleX)"
                                                     From="0.85" To="1.15" Duration="0:0:0.6">
                                        <DoubleAnimation.EasingFunction>
                                            <SineEase EasingMode="EaseInOut" />
                                        </DoubleAnimation.EasingFunction>
                                    </DoubleAnimation>
                                    <DoubleAnimation Storyboard.TargetProperty="(UIElement.RenderTransform).(ScaleTransform.ScaleY)"
                                                     From="0.85" To="1.15" Duration="0:0:0.6">
                                        <DoubleAnimation.EasingFunction>
                                            <SineEase EasingMode="EaseInOut" />
                                        </DoubleAnimation.EasingFunction>
                                    </DoubleAnimation>
                                </Storyboard>
                            </BeginStoryboard>
                        </EventTrigger>
                    </Ellipse.Triggers>
                </Ellipse>
                <!-- Done: checkmark text glyph in accent -->
                <TextBlock Text="✓"
                           FontFamily="Segoe UI"
                           FontWeight="Bold"
                           FontSize="13"
                           Foreground="{DynamicResource Accent}"
                           HorizontalAlignment="Center"
                           VerticalAlignment="Center"
                           Visibility="{Binding Status, Converter={StaticResource StatusToVis}, ConverterParameter=Done}" />
                <!-- Failed: warning glyph in amber -->
                <TextBlock Text="⚠"
                           FontFamily="Segoe UI Symbol"
                           FontSize="12"
                           Foreground="{DynamicResource WarningAmber}"
                           HorizontalAlignment="Center"
                           VerticalAlignment="Center"
                           Visibility="{Binding Status, Converter={StaticResource StatusToVis}, ConverterParameter=Failed}" />
            </Grid>

            <TextBlock Grid.Column="1"
                       Text="{Binding Title}"
                       FontFamily="{StaticResource UiFont}"
                       FontSize="13"
                       Foreground="{DynamicResource TextPrimary}"
                       VerticalAlignment="Center"
                       Margin="8,0,0,0" />
        </Grid>
    </DataTemplate>
```

- [ ] **Step 2: Create the StatusToVis converter**

Write `src/Plith.Installer/ViewModels/StatusToVisibilityConverter.cs`:

```csharp
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace Plith.Installer.ViewModels;

/// <summary>Shows the bound element only when InstallStepViewModel.Status matches the
/// parameter (e.g. ConverterParameter=Running → Visible only while step is running).</summary>
public sealed class StatusToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is InstallStepStatus status && parameter is string target
            && Enum.TryParse<InstallStepStatus>(target, out var targetStatus))
        {
            return status == targetStatus ? Visibility.Visible : Visibility.Collapsed;
        }
        return Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
```

- [ ] **Step 3: Register the converter in App.xaml resources**

Use Edit tool on `src/Plith.Installer/App.xaml`. Replace:

```xml
<Application.Resources>
    <ResourceDictionary>
        <ResourceDictionary.MergedDictionaries>
            <ResourceDictionary Source="Resources/Palette.Dark.xaml" />
            <ResourceDictionary Source="Resources/Theme.xaml" />
            <ResourceDictionary Source="Resources/Animations.xaml" />
            <ResourceDictionary Source="Resources/InstallerStyles.xaml" />
        </ResourceDictionary.MergedDictionaries>
    </ResourceDictionary>
</Application.Resources>
```

with:

```xml
<Application.Resources>
    <ResourceDictionary>
        <ResourceDictionary.MergedDictionaries>
            <ResourceDictionary Source="Resources/Palette.Dark.xaml" />
            <ResourceDictionary Source="Resources/Theme.xaml" />
            <ResourceDictionary Source="Resources/Animations.xaml" />
            <ResourceDictionary Source="Resources/InstallerStyles.xaml" />
        </ResourceDictionary.MergedDictionaries>
        <vm:StatusToVisibilityConverter x:Key="StatusToVis" />
    </ResourceDictionary>
</Application.Resources>
```

And add the xmlns at the Application root:

```xml
<Application x:Class="Plith.Installer.App"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:vm="clr-namespace:Plith.Installer.ViewModels"
             ShutdownMode="OnMainWindowClose">
```

- [ ] **Step 4: Write ProgressPage.xaml**

Write `src/Plith.Installer/Pages/ProgressPage.xaml`:

```xml
<UserControl x:Class="Plith.Installer.Pages.ProgressPage"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             RenderTransformOrigin="0.5,0.5">
    <UserControl.RenderTransform>
        <TranslateTransform />
    </UserControl.RenderTransform>
    <Grid Margin="32,24,32,24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="*" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <TextBlock Grid.Row="0"
                   x:Name="HeadlineText"
                   Text="Installing Plith…"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="20"
                   FontWeight="SemiBold"
                   Foreground="{DynamicResource TextPrimary}"
                   Margin="0,0,0,18" />

        <ItemsControl Grid.Row="1"
                      ItemsSource="{Binding Steps}"
                      ItemTemplate="{StaticResource StepRowTemplate}" />

        <ProgressBar Grid.Row="2"
                     Height="4"
                     Background="{DynamicResource TrackInactive}"
                     Foreground="{DynamicResource Accent}"
                     BorderThickness="0"
                     Minimum="0" Maximum="1"
                     Value="{Binding Progress, Mode=OneWay}"
                     Margin="0,18,0,0" />
    </Grid>
</UserControl>
```

- [ ] **Step 5: Write ProgressPage.xaml.cs**

Write `src/Plith.Installer/Pages/ProgressPage.xaml.cs`:

```csharp
using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class ProgressPage : UserControl
{
    public ProgressPage(InstallerViewModel vm, string headline)
    {
        InitializeComponent();
        DataContext = vm;
        HeadlineText.Text = headline;

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
```

- [ ] **Step 6: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 7: Commit**

```bash
git add src/Plith.Installer/Pages/ProgressPage.xaml src/Plith.Installer/Pages/ProgressPage.xaml.cs src/Plith.Installer/ViewModels/StatusToVisibilityConverter.cs src/Plith.Installer/App.xaml src/Plith.Installer/Resources/InstallerStyles.xaml
git commit -m "$(cat <<'EOF'
feat(installer): ProgressPage with animated step list

ItemsControl binds to InstallerViewModel.Steps. StepRowTemplate switches
between hollow circle (Pending) / pulsing accent dot (Running, scale 0.85↔1.15
sine 600ms) / accent ✓ (Done) / amber ⚠ (Failed) via the new
StatusToVisibilityConverter. Linear ProgressBar tracks vm.Progress (0..1).
Headline text injected via ctor so install / uninstall flows can reuse the
same page with different copy ("Installing Plith…" vs "Uninstalling Plith…").
EOF
)"
```

---

## Task 14: FinishPage + ErrorPage

Two simpler pages reusing the same Accent/Ghost button styles.

**Files:**
- Create: `src/Plith.Installer/Pages/FinishPage.xaml` + `.cs`
- Create: `src/Plith.Installer/Pages/ErrorPage.xaml` + `.cs`

- [ ] **Step 1: Write FinishPage.xaml**

Write `src/Plith.Installer/Pages/FinishPage.xaml`:

```xml
<UserControl x:Class="Plith.Installer.Pages.FinishPage"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             RenderTransformOrigin="0.5,0.5">
    <UserControl.RenderTransform>
        <TranslateTransform />
    </UserControl.RenderTransform>
    <Grid Margin="32,24,32,24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="*" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <!-- Hero checkmark -->
        <Ellipse Grid.Row="0"
                 Width="64" Height="64"
                 Fill="{DynamicResource AccentGlow}"
                 HorizontalAlignment="Center"
                 Margin="0,4,0,12" />
        <TextBlock Grid.Row="0"
                   Text="✓"
                   FontFamily="Segoe UI"
                   FontSize="32"
                   FontWeight="Bold"
                   Foreground="{DynamicResource Accent}"
                   HorizontalAlignment="Center"
                   VerticalAlignment="Center"
                   Margin="0,4,0,12" />

        <TextBlock Grid.Row="1"
                   x:Name="HeadlineText"
                   Text="Plith is ready"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="22"
                   FontWeight="SemiBold"
                   Foreground="{DynamicResource TextPrimary}"
                   HorizontalAlignment="Center"
                   Margin="0,0,0,6" />

        <TextBlock Grid.Row="2"
                   x:Name="SubtitleText"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="13"
                   Foreground="{DynamicResource TextSecondary}"
                   HorizontalAlignment="Center"
                   TextAlignment="Center"
                   TextWrapping="Wrap"
                   Margin="0,0,0,18" />

        <Button Grid.Row="4"
                x:Name="OpenPlithButton"
                Content="Open Plith"
                Style="{StaticResource AccentButtonStyle}"
                MinHeight="42"
                Margin="0,0,0,10" />

        <StackPanel Grid.Row="5" Orientation="Horizontal" HorizontalAlignment="Center">
            <Button x:Name="GitHubButton"
                    Content="View on GitHub"
                    Style="{StaticResource GhostButtonStyle}"
                    Margin="0,0,10,0" />
            <Button x:Name="CloseButton"
                    Content="Close"
                    Style="{StaticResource GhostButtonStyle}" />
        </StackPanel>
    </Grid>
</UserControl>
```

- [ ] **Step 2: Write FinishPage.xaml.cs**

Write `src/Plith.Installer/Pages/FinishPage.xaml.cs`:

```csharp
using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class FinishPage : UserControl
{
    public FinishPage(InstallerViewModel vm, string installedExePath)
    {
        InitializeComponent();
        DataContext = vm;

        SubtitleText.Text = vm.GameModeEnabled
            ? "Game mode is active — OSD draws over fullscreen games."
            : "OSD draws over borderless fullscreen games.";

        OpenPlithButton.Visibility = vm.OpenAfterInstall ? Visibility.Visible : Visibility.Collapsed;

        OpenPlithButton.Click += (_, _) =>
        {
            // Launch via explorer.exe so the new process runs in the user context (not admin).
            // Direct Process.Start from an elevated installer fails for UIAccess binaries
            // ("A referral was returned from the server").
            Process.Start(new ProcessStartInfo("explorer.exe", $"\"{installedExePath}\""));
            Application.Current.Shutdown();
        };

        GitHubButton.Click += (_, _) => Process.Start(new ProcessStartInfo("https://github.com/berkeerdo/Plith") { UseShellExecute = true });
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
```

- [ ] **Step 3: Write ErrorPage.xaml**

Write `src/Plith.Installer/Pages/ErrorPage.xaml`:

```xml
<UserControl x:Class="Plith.Installer.Pages.ErrorPage"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             RenderTransformOrigin="0.5,0.5">
    <UserControl.RenderTransform>
        <TranslateTransform />
    </UserControl.RenderTransform>
    <Grid Margin="32,24,32,24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="*" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <TextBlock Grid.Row="0"
                   Text="⚠"
                   FontFamily="Segoe UI Symbol"
                   FontSize="48"
                   Foreground="{DynamicResource WarningAmber}"
                   HorizontalAlignment="Center"
                   Margin="0,4,0,12" />

        <TextBlock Grid.Row="1"
                   Text="Install failed"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="22"
                   FontWeight="SemiBold"
                   Foreground="{DynamicResource WarningAmber}"
                   HorizontalAlignment="Center"
                   Margin="0,0,0,6" />

        <StackPanel Grid.Row="2" Margin="0,0,0,18">
            <TextBlock x:Name="FailedStepText"
                       FontFamily="{StaticResource UiFont}"
                       FontSize="13"
                       Foreground="{DynamicResource TextSecondary}"
                       HorizontalAlignment="Center"
                       Margin="0,0,0,4" />
            <TextBlock x:Name="ErrorMessageText"
                       FontFamily="{StaticResource UiFont}"
                       FontSize="12"
                       Foreground="{DynamicResource TextTertiary}"
                       HorizontalAlignment="Center"
                       TextAlignment="Center"
                       TextWrapping="Wrap" />
        </StackPanel>

        <StackPanel Grid.Row="4" Orientation="Horizontal" HorizontalAlignment="Center" Margin="0,0,0,10">
            <Button x:Name="CopyLogButton"
                    Content="Copy log"
                    Style="{StaticResource GhostButtonStyle}"
                    Margin="0,0,10,0" />
            <Button x:Name="OpenLogButton"
                    Content="Open log"
                    Style="{StaticResource GhostButtonStyle}" />
        </StackPanel>

        <Button Grid.Row="5"
                x:Name="CloseButton"
                Content="Close"
                Style="{StaticResource GhostButtonStyle}"
                MinHeight="36"
                HorizontalAlignment="Center" />
    </Grid>
</UserControl>
```

- [ ] **Step 4: Write ErrorPage.xaml.cs**

Write `src/Plith.Installer/Pages/ErrorPage.xaml.cs`:

```csharp
using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;
using Plith.Installer.Services;
using Plith.Installer.ViewModels;

namespace Plith.Installer.Pages;

public partial class ErrorPage : UserControl
{
    public ErrorPage(InstallerViewModel vm, LogService log)
    {
        InitializeComponent();
        DataContext = vm;

        FailedStepText.Text = $"Step: \"{vm.FailedStepTitle}\"";
        ErrorMessageText.Text = vm.ErrorMessage;

        CopyLogButton.Click += (_, _) => Clipboard.SetText(log.ReadAll());
        OpenLogButton.Click += (_, _) => Process.Start(new ProcessStartInfo(log.LogPath) { UseShellExecute = true });
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
```

- [ ] **Step 5: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 6: Commit**

```bash
git add src/Plith.Installer/Pages/FinishPage.xaml src/Plith.Installer/Pages/FinishPage.xaml.cs src/Plith.Installer/Pages/ErrorPage.xaml src/Plith.Installer/Pages/ErrorPage.xaml.cs
git commit -m "$(cat <<'EOF'
feat(installer): FinishPage + ErrorPage

FinishPage: 64dp accent green hero checkmark, "Plith is ready" headline,
conditional subtitle (Game mode active vs borderless-only), Open Plith
button (shown only if vm.OpenAfterInstall) that launches via explorer.exe
to avoid the UIAccess-from-elevated-parent referral error, GitHub + Close
ghost buttons.

ErrorPage: 48dp WarningAmber ⚠, "Install failed" headline, failed step
title + error message excerpt, Copy log (Clipboard.SetText) + Open log
(Process.Start LogService.LogPath) + Close buttons.

Both use SlideFadeIn on Loaded for smooth page transitions.
EOF
)"
```

---

## Task 15: Uninstall sub-flow pages (Confirm + Progress + Finish)

Three small pages for the `--uninstall` mode. UninstallConfirmPage asks for confirmation, UninstallProgressPage reuses ProgressPage's step list rendering, UninstallFinishPage closes the app.

**Files:**
- Create: `src/Plith.Installer/Pages/UninstallConfirmPage.xaml` + `.cs`
- Create: `src/Plith.Installer/Pages/UninstallFinishPage.xaml` + `.cs`

(No new ProgressPage equivalent — we reuse the existing one with a different headline string.)

- [ ] **Step 1: Write UninstallConfirmPage.xaml**

Write `src/Plith.Installer/Pages/UninstallConfirmPage.xaml`:

```xml
<UserControl x:Class="Plith.Installer.Pages.UninstallConfirmPage"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             RenderTransformOrigin="0.5,0.5">
    <UserControl.RenderTransform>
        <TranslateTransform />
    </UserControl.RenderTransform>
    <Grid Margin="32,24,32,24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="*" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <TextBlock Grid.Row="0"
                   Text="Uninstall Plith?"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="22"
                   FontWeight="SemiBold"
                   Foreground="{DynamicResource TextPrimary}"
                   HorizontalAlignment="Center"
                   Margin="0,8,0,12" />

        <TextBlock Grid.Row="1"
                   Text="This will stop Plith, remove its files from \Program Files\Plith\, and clean up the Start menu shortcut + Add/Remove Programs entry. Your settings in %LOCALAPPDATA%\Plith\ are left alone."
                   FontFamily="{StaticResource UiFont}"
                   FontSize="13"
                   Foreground="{DynamicResource TextSecondary}"
                   HorizontalAlignment="Center"
                   TextAlignment="Center"
                   TextWrapping="Wrap"
                   Margin="20,0,20,0" />

        <StackPanel Grid.Row="3" Orientation="Horizontal" HorizontalAlignment="Center">
            <Button x:Name="UninstallButton"
                    Content="Uninstall"
                    Style="{StaticResource AccentButtonStyle}"
                    MinWidth="120"
                    Margin="0,0,10,0" />
            <Button x:Name="CancelButton"
                    Content="Cancel"
                    Style="{StaticResource GhostButtonStyle}"
                    MinWidth="100" />
        </StackPanel>
    </Grid>
</UserControl>
```

- [ ] **Step 2: Write UninstallConfirmPage.xaml.cs**

Write `src/Plith.Installer/Pages/UninstallConfirmPage.xaml.cs`:

```csharp
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace Plith.Installer.Pages;

public partial class UninstallConfirmPage : UserControl
{
    public event EventHandler? UninstallClicked;

    public UninstallConfirmPage()
    {
        InitializeComponent();

        UninstallButton.Click += (_, _) => UninstallClicked?.Invoke(this, EventArgs.Empty);
        CancelButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
```

- [ ] **Step 3: Write UninstallFinishPage.xaml**

Write `src/Plith.Installer/Pages/UninstallFinishPage.xaml`:

```xml
<UserControl x:Class="Plith.Installer.Pages.UninstallFinishPage"
             xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             RenderTransformOrigin="0.5,0.5">
    <UserControl.RenderTransform>
        <TranslateTransform />
    </UserControl.RenderTransform>
    <Grid Margin="32,24,32,24">
        <Grid.RowDefinitions>
            <RowDefinition Height="Auto" />
            <RowDefinition Height="Auto" />
            <RowDefinition Height="*" />
            <RowDefinition Height="Auto" />
        </Grid.RowDefinitions>

        <TextBlock Grid.Row="0"
                   Text="✓"
                   FontFamily="Segoe UI"
                   FontSize="48"
                   FontWeight="Bold"
                   Foreground="{DynamicResource Accent}"
                   HorizontalAlignment="Center"
                   Margin="0,12,0,12" />

        <TextBlock Grid.Row="1"
                   Text="Plith uninstalled"
                   FontFamily="{StaticResource UiFont}"
                   FontSize="22"
                   FontWeight="SemiBold"
                   Foreground="{DynamicResource TextPrimary}"
                   HorizontalAlignment="Center"
                   Margin="0,0,0,6" />

        <Button Grid.Row="3"
                x:Name="CloseButton"
                Content="Close"
                Style="{StaticResource AccentButtonStyle}"
                MinHeight="42"
                HorizontalAlignment="Center"
                MinWidth="120" />
    </Grid>
</UserControl>
```

- [ ] **Step 4: Write UninstallFinishPage.xaml.cs**

Write `src/Plith.Installer/Pages/UninstallFinishPage.xaml.cs`:

```csharp
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media.Animation;

namespace Plith.Installer.Pages;

public partial class UninstallFinishPage : UserControl
{
    public UninstallFinishPage()
    {
        InitializeComponent();
        CloseButton.Click += (_, _) => Application.Current.Shutdown();

        Loaded += (_, _) =>
        {
            if (TryFindResource("SlideFadeIn") is Storyboard sb) sb.Begin(this);
        };
    }
}
```

- [ ] **Step 5: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 6: Commit**

```bash
git add src/Plith.Installer/Pages/UninstallConfirmPage.xaml src/Plith.Installer/Pages/UninstallConfirmPage.xaml.cs src/Plith.Installer/Pages/UninstallFinishPage.xaml src/Plith.Installer/Pages/UninstallFinishPage.xaml.cs
git commit -m "$(cat <<'EOF'
feat(installer): UninstallConfirmPage + UninstallFinishPage

ConfirmPage: explanation copy + Uninstall (accent) / Cancel (ghost) buttons.
Settings in %LOCALAPPDATA%\Plith\ stay; documented in body text.
FinishPage: accent ✓ + "Plith uninstalled" + Close button. ProgressPage is
reused with "Uninstalling Plith…" headline (no separate page needed).
EOF
)"
```

---

## Task 16: App.xaml.cs routing — wire pages to flows

Replace the placeholder MainWindow with full page navigation. App.OnStartup builds the InstallerViewModel + services, detects existing install, branches to install or uninstall flow, navigates pages.

**Files:**
- Modify: `src/Plith.Installer/App.xaml.cs`
- Modify: `src/Plith.Installer/MainWindow.xaml.cs` (if needed for orchestration hookup)

- [ ] **Step 1: Rewrite App.xaml.cs with full routing**

Use Edit tool to replace the entire contents of `src/Plith.Installer/App.xaml.cs`:

```csharp
using System.Threading;
using System.Windows;
using Plith.Installer.Pages;
using Plith.Installer.Services;
using Plith.Installer.ViewModels;

namespace Plith.Installer;

public partial class App : Application
{
    private const string SingleInstanceMutexName = "Global\\Plith.Installer.SingleInstance.7F9C8E1A";
    private Mutex? _singleInstanceMutex;

    public bool IsUninstallMode { get; private set; }

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _singleInstanceMutex = new Mutex(initiallyOwned: true, SingleInstanceMutexName, out bool createdNew);
        if (!createdNew)
        {
            MessageBox.Show("Plith Setup is already running.",
                "Plith Setup", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown();
            return;
        }

        IsUninstallMode = e.Args.Length > 0 && e.Args[0] == "--uninstall";

        var log = new LogService();
        var vm = new InstallerViewModel();
        var cert = new CertService();
        var signtool = new SignToolWrapper(log);
        var shortcut = new ShortcutService();
        var registry = new RegistryService();
        var orchestrator = new InstallOrchestrator(log, cert, signtool, shortcut, registry, vm);

        var window = new MainWindow();
        window.Show();

        if (IsUninstallMode)
            RouteUninstallFlow(window, vm, orchestrator, log);
        else
            RouteInstallFlow(window, vm, orchestrator, log);
    }

    private static void RouteInstallFlow(MainWindow window, InstallerViewModel vm,
        InstallOrchestrator orchestrator, LogService log)
    {
        var detector = new InstallDetector();
        var existing = detector.GetInstalledVersion();
        vm.NewVersion = typeof(App).Assembly.GetName().Version?.ToString(3) ?? "0.1.0";

        if (existing is null) vm.Mode = InstallerMode.FreshInstall;
        else if (existing == vm.NewVersion) { vm.Mode = InstallerMode.Reinstall; vm.ExistingVersion = existing; }
        else { vm.Mode = InstallerMode.Update; vm.ExistingVersion = existing; }

        var welcome = new WelcomePage(vm);
        welcome.PrimaryClicked += async (_, _) =>
        {
            orchestrator.PrepareSteps();
            window.NavigateTo(new ProgressPage(vm, vm.Mode == InstallerMode.Update
                ? "Updating Plith…"
                : "Installing Plith…"));
            try
            {
                await orchestrator.RunInstallAsync();
                window.NavigateTo(new FinishPage(vm, InstallOrchestrator.InstalledExe));
            }
            catch
            {
                window.NavigateTo(new ErrorPage(vm, log));
            }
        };
        window.NavigateTo(welcome);
    }

    private static void RouteUninstallFlow(MainWindow window, InstallerViewModel vm,
        InstallOrchestrator orchestrator, LogService log)
    {
        var confirm = new UninstallConfirmPage();
        confirm.UninstallClicked += async (_, _) =>
        {
            orchestrator.PrepareUninstallSteps();
            window.NavigateTo(new ProgressPage(vm, "Uninstalling Plith…"));
            try
            {
                await orchestrator.RunUninstallAsync();
                window.NavigateTo(new UninstallFinishPage());
            }
            catch
            {
                window.NavigateTo(new ErrorPage(vm, log));
            }
        };
        window.NavigateTo(confirm);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _singleInstanceMutex?.ReleaseMutex();
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
```

- [ ] **Step 2: Build**

```powershell
dotnet build src\Plith.Installer\Plith.Installer.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 3: Run tests — Plith.Tests + Plith.Installer.Tests both green**

```powershell
dotnet test tests\Plith.Tests\Plith.Tests.csproj 2>&1 | Select-Object -Last 6
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 36` and `Passed: 6`.

- [ ] **Step 4: Smoke (dev mode) — launch installer, confirm Welcome appears**

```powershell
dotnet run --project src\Plith.Installer
```

Expected: UAC prompt (manifest requireAdministrator), then installer window appears with WelcomePage showing icon + "Welcome to Plith" + subtitle + "Install Plith" button + collapsed "Advanced options" expander. Click expander → toggles appear. Close window without installing (we don't want a dev-build install during plan execution).

- [ ] **Step 5: Commit**

```bash
git add src/Plith.Installer/App.xaml.cs
git commit -m "$(cat <<'EOF'
feat(installer): App.xaml.cs routes install + uninstall flows

OnStartup builds services + viewmodel, then branches on --uninstall flag.
Install flow: InstallDetector picks mode (FreshInstall / Reinstall / Update)
based on existing Plith.exe ProductVersion vs the installer's own assembly
version. WelcomePage.PrimaryClicked → ProgressPage → orchestrator.RunInstallAsync
→ FinishPage (or ErrorPage on throw). Uninstall flow:
UninstallConfirmPage.UninstallClicked → ProgressPage → orchestrator.RunUninstallAsync
→ UninstallFinishPage (or ErrorPage on throw).
EOF
)"
```

---

## Task 17: scripts/build-release.ps1 + delete legacy scripts

The release builder script: runs tests, single-file publishes Plith.Installer, renames to `Plith-Setup-X.Y.Z.exe`, signs with the self-signed cert. Then this task deletes the Phase 4h PowerShell scripts.

**Files:**
- Create: `scripts/build-release.ps1`
- Delete: `scripts/setup-cert.ps1`, `scripts/install-local.ps1`, `scripts/uninstall-local.ps1`

- [ ] **Step 1: Write build-release.ps1**

Write `scripts/build-release.ps1`:

```powershell
# scripts/build-release.ps1 — builds the single-file installer EXE and signs it.
# Run from an admin PowerShell (signtool needs to access the cert in CurrentUser\My,
# and certificate lookup can be admin-gated depending on system policy).

[CmdletBinding()]
param(
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$installerProj = Join-Path $repoRoot 'src\Plith.Installer\Plith.Installer.csproj'
$plithTests = Join-Path $repoRoot 'tests\Plith.Tests\Plith.Tests.csproj'
$installerTests = Join-Path $repoRoot 'tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj'
$releaseDir = Join-Path $repoRoot 'release'

# 1. Tests must pass.
Write-Host "Running Plith.Tests..."
& dotnet test $plithTests
if ($LASTEXITCODE -ne 0) { throw "Plith.Tests failed." }

Write-Host "Running Plith.Installer.Tests..."
& dotnet test $installerTests
if ($LASTEXITCODE -ne 0) { throw "Plith.Installer.Tests failed." }

# 2. Publish installer as single file with self-extract bundle.
Write-Host "Publishing installer..."
if (Test-Path $releaseDir) { Remove-Item -Path $releaseDir -Recurse -Force }
& dotnet publish $installerProj -c $Configuration -r win-x64 `
    -p:PublishSingleFile=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:SelfContained=true `
    -p:EnableCompressionInSingleFile=true `
    -o $releaseDir
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed." }

# 3. Rename to Plith-Setup-<version>.exe
$installerExe = Join-Path $releaseDir 'Plith.Installer.exe'
$version = (Get-Item $installerExe).VersionInfo.ProductVersion
if (-not $version) { $version = '0.1.0' }
$setupExe = Join-Path $releaseDir "Plith-Setup-$version.exe"
Move-Item -Path $installerExe -Destination $setupExe -Force

# 4. Sign the installer with the self-signed cert.
$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq 'CN=Plith Self-Signed' -and $_.NotAfter -gt (Get-Date) } |
    Select-Object -First 1

if (-not $cert) {
    Write-Warning "No Plith Self-Signed cert found in CurrentUser\My. The release artifact will be UNSIGNED."
    Write-Host "Release artifact (unsigned): $setupExe"
    exit 0
}

# Locate signtool (same logic as installer's SignToolWrapper).
$signtoolPath = $null
$cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
if ($cmd) { $signtoolPath = $cmd.Source }
if (-not $signtoolPath) {
    $candidates = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
        -Filter 'signtool.exe' -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match 'x64' } |
        Sort-Object FullName -Descending
    if ($candidates) { $signtoolPath = $candidates[0].FullName }
}
if (-not $signtoolPath) { throw "signtool.exe not found." }

Write-Host "Signing $setupExe..."
& $signtoolPath sign /sha1 $cert.Thumbprint /fd SHA256 `
    /tr 'http://timestamp.digicert.com' /td SHA256 $setupExe | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed." }

Write-Host ""
Write-Host "Release artifact ready: $setupExe"
```

- [ ] **Step 2: Delete legacy install scripts**

```powershell
Remove-Item scripts\setup-cert.ps1, scripts\install-local.ps1, scripts\uninstall-local.ps1
```

- [ ] **Step 3: Verify the new script parses**

```powershell
pwsh -NoProfile -Command "
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile('scripts/build-release.ps1', [ref]$null, [ref]$errors) | Out-Null
if ($errors.Count -eq 0) { 'build-release.ps1: syntax OK' } else { $errors | ForEach-Object { Write-Host $_.Message } }
"
```

Expected: `build-release.ps1: syntax OK`.

- [ ] **Step 4: Commit**

```bash
git add scripts/build-release.ps1
git rm scripts/setup-cert.ps1 scripts/install-local.ps1 scripts/uninstall-local.ps1
git commit -m "$(cat <<'EOF'
feat(scripts): build-release.ps1 + retire Phase 4h install scripts

build-release.ps1 is the only thing left in scripts/. It runs both test
suites (Plith.Tests + Plith.Installer.Tests), single-file publishes the
installer to release/Plith-Setup-<version>.exe, then signs it with the
self-signed cert (same dual-store cert the installer itself uses at install
time). Falls back to producing an unsigned artifact with a warning if no
cert is available.

scripts/setup-cert.ps1, install-local.ps1, and uninstall-local.ps1 are
deleted — the wizard replaces them entirely.
EOF
)"
```

---

## Task 18: README updates

Replace the Game mode install section with Plith-Setup.exe instructions; add an Uninstall subsection; add a Building a release subsection that references `scripts/build-release.ps1`.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace the Game mode section**

Use Edit tool on `README.md`. Replace the existing Game mode section (the one added in Phase 4h, between `### Game mode (works over fullscreen games)` and the closing of that subsection):

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

with:

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "$(cat <<'EOF'
docs(readme): document Plith-Setup wizard install + uninstall + release build

Replaces the Phase 4h "run install-local.ps1 as admin" instructions with
"download Plith-Setup-<version>.exe and double-click". Adds an Install
subsection covering the SmartScreen prompt for self-signed binaries, an
Uninstall subsection pointing at Add/Remove Programs (and the
Plith-Uninstaller.exe in Setup\ as a fallback), and a Build a release
artifact subsection for scripts/build-release.ps1. Game mode section
shortened — install now implies Game mode.
EOF
)"
```

---

## Task 19: End-to-end verification

Build everything, run all tests, manually drive the installer once to confirm install + open-Plith + uninstall round-trip works.

- [ ] **Step 1: Full Debug build**

```powershell
dotnet build src\Plith\Plith.csproj -c Debug 2>&1 | Select-Object -Last 6
dotnet build src\Plith.Installer\Plith.Installer.csproj -c Debug 2>&1 | Select-Object -Last 6
```

Expected: both `Build succeeded. 0 Warning(s) 0 Error(s)`.

- [ ] **Step 2: Full Release strict build**

```powershell
dotnet build src\Plith\Plith.csproj -c Release -warnaserror 2>&1 | Select-Object -Last 6
dotnet build src\Plith.Installer\Plith.Installer.csproj -c Release -warnaserror 2>&1 | Select-Object -Last 6
```

Expected: both succeed.

- [ ] **Step 3: All tests green**

```powershell
dotnet test tests\Plith.Tests\Plith.Tests.csproj 2>&1 | Select-Object -Last 6
dotnet test tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj 2>&1 | Select-Object -Last 6
```

Expected: `Passed: 36` and `Passed: 6`.

- [ ] **Step 4: Build the release artifact (admin PowerShell)**

```powershell
# Open Windows Terminal as administrator first
pwsh scripts\build-release.ps1
```

Expected: produces `release\Plith-Setup-0.1.0.exe`, signed.

- [ ] **Step 5: Run the installer to install Plith fresh**

Double-click `release\Plith-Setup-0.1.0.exe`. Verify:
- UAC prompt → Yes
- Wizard window appears, custom titlebar, Mica
- WelcomePage shows "Install Plith" button (no existing install)
- Click Advanced options → 3 checkboxes appear, all checked by default
- Click Install Plith → ProgressPage animates through 5 steps (Setting up cert → Extracting → Signing → Copying → Registering)
- FinishPage shows ✓ + "Plith is ready" + "Open Plith" button
- Click Open Plith → installer closes, Plith launches, tray icon appears, Game mode badge in Settings reads "Active"

- [ ] **Step 6: Run the installer again (reinstall mode)**

Re-launch `release\Plith-Setup-0.1.0.exe`. Verify:
- Welcome button now reads "Reinstall Plith v0.1.0"
- Click → install completes again successfully
- Plith re-launches cleanly

- [ ] **Step 7: Uninstall via Add/Remove Programs**

`Win + I → Apps → Installed apps → Plith → ⋯ → Uninstall`. Verify:
- Plith-Uninstaller.exe launches with wizard window
- UninstallConfirmPage shows
- Click Uninstall → ProgressPage 3 steps animate → UninstallFinishPage "Plith uninstalled"
- Click Close → window dismisses
- After ~3 seconds (the self-delete timeout) verify `C:\Program Files\Plith\` is gone
- Start menu search "Plith" → no results
- Add/Remove Programs → no Plith entry

- [ ] **Step 8: Commit any small fixups discovered during smoke**

If everything works clean, no commit needed here — Phase 5 is complete.

If a small bug surfaced (e.g., a missing brush key), commit a focused fix:

```bash
git add <file>
git commit -m "fix(installer): <one-line problem>"
```

---

## Verification Summary

After all 19 tasks, the following must be true:

- `dotnet build src/Plith.Installer/Plith.Installer.csproj -c Release -warnaserror` → 0 warnings, 0 errors.
- `dotnet test tests/Plith.Tests/Plith.Tests.csproj` → 36 passed.
- `dotnet test tests/Plith.Installer.Tests/Plith.Installer.Tests.csproj` → 6 passed.
- `pwsh scripts/build-release.ps1` (admin) → produces signed `release/Plith-Setup-0.1.0.exe`.
- Installing via the wizard puts Plith in `\Program Files\Plith\`, Start menu, Add/Remove Programs, with Game mode badge "Active".
- Uninstalling via Add/Remove Programs removes all those traces.
- Re-launching the installer with a fresh existing install shows "Reinstall Plith v0.1.0" button.

---

## Notes for the Implementing Agent

- **Conventional Commits + English-only + No AI attribution.** Repository rule.
- **PowerShell on Windows for commits.** The Bash tool's HEREDOC works; the pattern in this plan (`git commit -m "$(cat <<'EOF' ... EOF)"`) is what's used throughout. Don't switch styles.
- **No `--no-verify`, no `--amend`.** Fresh commits for every step.
- **Manual smoke at Task 19 requires admin PowerShell that the user opens themselves.** The script runner can't elevate — the user opens admin Terminal, runs `pwsh scripts\build-release.ps1`, then double-clicks the resulting Setup.exe. Pause and ask before proceeding.
- **Don't modify `src/Plith/Interop/BandWindow/*.cs`.** That's MIT-credited port from FancyOSD (Phase 4h scope).
- **Don't merge tasks.** Each task's commit isolates a single change for bisectability.
- **If a step fails during execution:** investigate, fix forward, commit the fix as part of the same task. Don't try to bend the plan to skip the failure.
