# scripts/build-release.ps1 — builds the installer as a single-file
# self-extracting .exe (~40 MB compressed).
#
# History: 0.1.4 and 0.1.5 initial build shipped folder+zip during the Norton
# era on the dev machine — Norton's SONAR / Download Insight engines killed
# the self-extract mid-init. Reverted to single-file for 0.1.5 publish now
# that the dev machine is on Windows Defender, which does not sandbox this
# path. If Norton-heavy users report install crashes down the line, add the
# folder+zip back as a second output alongside the .exe.
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

# 2. Publish installer as a single-file self-extracting .exe.
# IncludeNativeLibrariesForSelfExtract=true is required for WPF apps so
# PresentationNative and friends land inside the bundle instead of loose
# alongside it. EnableCompressionInSingleFile=true shrinks the artifact
# from ~100 MB to ~40 MB at the cost of ~1 s extra first-launch decompress.
Write-Host "Publishing installer as single-file self-extract .exe..."
if (Test-Path $releaseDir) { Remove-Item -Path $releaseDir -Recurse -Force }
New-Item -ItemType Directory -Path $publishDir | Out-Null
& dotnet publish $installerProj -c $Configuration -r win-x64 `
    -p:PublishSingleFile=true `
    -p:SelfContained=true `
    -p:IncludeNativeLibrariesForSelfExtract=true `
    -p:EnableCompressionInSingleFile=true `
    -o $publishDir
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed." }

# 3. Move the published .exe to the release root as Plith-Setup-<version>.exe.
# The published name from dotnet is Plith.Installer.exe; users see and
# download Plith-Setup-<version>.exe on GitHub Releases.
$mainExe = Join-Path $publishDir 'Plith.Installer.exe'
if (-not (Test-Path $mainExe)) { throw "Published exe not found at $mainExe" }
$version = (Get-Item $mainExe).VersionInfo.ProductVersion
if (-not $version) { $version = '0.1.0' }
$plusIdx = $version.IndexOf('+')
if ($plusIdx -ge 0) { $version = $version.Substring(0, $plusIdx) }

$finalExe = Join-Path $releaseDir "Plith-Setup-$version.exe"
Move-Item -Path $mainExe -Destination $finalExe -Force

# 4. Sign the .exe with the self-signed cert. Best-effort — the artifact
# still runs unsigned but SmartScreen and cooperating AVs treat a valid
# signature more kindly, and our installer's cert-installation step lets
# subsequent launches skip the "publisher unknown" nag.
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
        Write-Host "Signing Plith-Setup-$version.exe..."
        & $signtoolPath sign /sha1 $cert.Thumbprint /fd SHA256 `
            /tr 'http://timestamp.digicert.com' /td SHA256 $finalExe | Out-Host
        if ($LASTEXITCODE -ne 0) { Write-Warning "signtool failed; artifact will be UNSIGNED." }
    } else {
        Write-Warning "signtool.exe not found; artifact will be UNSIGNED."
    }
} else {
    Write-Warning "No Plith Self-Signed cert found in CurrentUser\My. Artifact will be UNSIGNED."
}

# 5. Clean up the intermediate publish/ folder — only the .exe ships.
Remove-Item -Path $publishDir -Recurse -Force

Write-Host ""
Write-Host "Release artifact ready: $finalExe"
