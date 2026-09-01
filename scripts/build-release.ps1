# scripts/build-release.ps1 — builds the installer as a folder + zip archive.
#
# History: earlier revisions shipped a single-file self-extracting .exe (~79 MB).
# That form crashes on machines running Norton and similar AV with "Failed to
# resolve full path of the current executable" — the AV kills the .exe mid
# self-extract before .NET's AppHost can init. Folder-mode publish sidesteps
# the entire self-extract path: the runtime just runs from the folder as-is.
# Users get Plith-Setup-<version>.zip; extract anywhere, run Plith-Installer.exe
# as administrator.
#
# Run from an admin PowerShell (signtool needs the cert in CurrentUser\My and
# lookup can be admin-gated depending on system policy).

[CmdletBinding()]
param(
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "build-release.ps1 requires administrator privileges (CertService tests + signtool cert lookup). Right-click PowerShell -> Run as administrator."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$installerProj = Join-Path $repoRoot 'src\Plith.Installer\Plith.Installer.csproj'
$plithTests = Join-Path $repoRoot 'tests\Plith.Tests\Plith.Tests.csproj'
$installerTests = Join-Path $repoRoot 'tests\Plith.Installer.Tests\Plith.Installer.Tests.csproj'
$releaseDir = Join-Path $repoRoot 'release'
$publishDir = Join-Path $releaseDir 'publish'

# 1. Tests must pass.
Write-Host "Running Plith.Tests..."
& dotnet test $plithTests
if ($LASTEXITCODE -ne 0) { throw "Plith.Tests failed." }

Write-Host "Running Plith.Installer.Tests..."
& dotnet test $installerTests
if ($LASTEXITCODE -ne 0) { throw "Plith.Installer.Tests failed." }

# 2. Publish installer as a folder (NOT single-file). Norton's SONAR / Download
# Insight / Data Protector engines all target .NET's temp-directory self-extract
# dance and yank the bundle out mid-init; folder mode has no self-extract, so
# those engines get nothing to bite on. Larger on disk (~100 MB unzipped, ~80 MB
# zipped) but this is the shape that actually installs on locked-down machines.
Write-Host "Publishing installer as folder..."
if (Test-Path $releaseDir) { Remove-Item -Path $releaseDir -Recurse -Force }
New-Item -ItemType Directory -Path $publishDir | Out-Null
& dotnet publish $installerProj -c $Configuration -r win-x64 `
    -p:PublishSingleFile=false `
    -p:SelfContained=true `
    -o $publishDir
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed." }

# 3. Rename Plith.Installer.exe -> Plith-Installer.exe (dash, not dot, so users
# see it clearly in Explorer next to the other DLLs without thinking it's a
# nested Plith.exe). The published main exe carries the ProductVersion.
$mainExe = Join-Path $publishDir 'Plith.Installer.exe'
$version = (Get-Item $mainExe).VersionInfo.ProductVersion
if (-not $version) { $version = '0.1.0' }
$plusIdx = $version.IndexOf('+')
if ($plusIdx -ge 0) { $version = $version.Substring(0, $plusIdx) }

$launcherName = 'Plith-Installer.exe'
$launcherExe = Join-Path $publishDir $launcherName
Move-Item -Path $mainExe -Destination $launcherExe -Force

# 4. Sign the launcher with the self-signed cert (best-effort — folder mode
# is meant to survive AV interference either way, but a valid signature still
# helps SmartScreen and any AV that respects it).
$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq 'CN=Plith Self-Signed' -and $_.NotAfter -gt (Get-Date) } |
    Select-Object -First 1

if ($cert) {
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
    if ($signtoolPath) {
        Write-Host "Signing $launcherName..."
        & $signtoolPath sign /sha1 $cert.Thumbprint /fd SHA256 `
            /tr 'http://timestamp.digicert.com' /td SHA256 $launcherExe | Out-Host
        if ($LASTEXITCODE -ne 0) { Write-Warning "signtool failed; artifact will be UNSIGNED." }
    } else {
        Write-Warning "signtool.exe not found; artifact will be UNSIGNED."
    }
} else {
    Write-Warning "No Plith Self-Signed cert found in CurrentUser\My. Artifact will be UNSIGNED."
}

# 5. Drop a plain README so users know what to do without opening a browser.
$readmePath = Join-Path $publishDir 'README.txt'
@"
Plith $version - Installer

To install:
  1. Right-click Plith-Installer.exe -> Run as administrator.
  2. Follow the on-screen prompts.

If Windows or your antivirus complains about a "publisher unknown" warning
click More info -> Run anyway. Plith is signed with a self-signed developer
certificate; the installer registers that certificate in your local trust
store so subsequent launches don't warn.

If your antivirus (particularly Norton) prevents the installer from writing
into C:\Program Files\Plith, add C:\Program Files\Plith to its scan
exclusion list before re-running.

Uninstall via Windows Settings -> Apps -> Plith -> Uninstall.
"@ | Set-Content -Path $readmePath -Encoding UTF8

# 6. Zip the whole publish folder as Plith-Setup-<version>.zip. This is the
# artifact that ships to GitHub Releases and to the in-app updater. Named to
# match the earlier single-file convention so existing docs and the release
# URL pattern still make sense.
$zipPath = Join-Path $releaseDir "Plith-Setup-$version.zip"
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
Compress-Archive -Path (Join-Path $publishDir '*') -DestinationPath $zipPath -CompressionLevel Optimal

# 7. Clean up the intermediate publish/ folder now that the zip is done.
Remove-Item -Path $publishDir -Recurse -Force

Write-Host ""
Write-Host "Release artifact ready: $zipPath"
