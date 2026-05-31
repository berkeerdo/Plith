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
