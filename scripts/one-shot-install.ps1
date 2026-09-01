# scripts/one-shot-install.ps1 - end-to-end "make Plith 0.1.5 exist on this box"
#
# Runs everything from git pull to launching the installer, with progress
# printed at each step and a post-run verification of the installed version.
# Idempotent: safe to re-run. Uses the folder+zip build path so Norton's
# self-extract sabotage never triggers.
#
# Usage:
#   Right-click PowerShell -> Run as administrator, then:
#     cd C:\Projects\plith
#     .\scripts\one-shot-install.ps1
#
# Optional switches:
#   -SkipBuild       reuse the existing release/Plith-Setup-*.zip if one is on disk
#   -SkipGitPull     don't touch git (offline / mid-edit)
#   -NoLaunch        stop after extract + everything ready, don't run installer

[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$SkipGitPull,
    [switch]$NoLaunch
)

$ErrorActionPreference = 'Stop'

function Step([string]$title) {
    Write-Host ""
    Write-Host "==> $title" -ForegroundColor Cyan
}

function Fail([string]$msg) {
    Write-Host ""
    Write-Host "FAIL: $msg" -ForegroundColor Red
    exit 1
}

# --- 0. Admin check --------------------------------------------------------
$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Fail "This script requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location $repoRoot

# --- 1. Git pull -----------------------------------------------------------
if (-not $SkipGitPull) {
    Step "git pull"
    & git pull origin main
    if ($LASTEXITCODE -ne 0) { Fail "git pull failed. Fix conflicts or re-run with -SkipGitPull." }
}

# --- 2. Kill running Plith ------------------------------------------------
Step "Stopping any running Plith"
$plithProcs = Get-Process -Name 'Plith' -ErrorAction SilentlyContinue
if ($plithProcs) {
    Write-Host "  Killing $($plithProcs.Count) Plith process(es)..."
    & taskkill /F /IM Plith.exe /T 2>&1 | Out-Host
    Start-Sleep -Seconds 2
    $stillAlive = Get-Process -Name 'Plith' -ErrorAction SilentlyContinue
    if ($stillAlive) {
        Write-Warning "  $($stillAlive.Count) Plith process(es) still alive after taskkill. The installer will retry with SeDebugPrivilege."
    } else {
        Write-Host "  Plith is dead."
    }
} else {
    Write-Host "  No Plith process running."
}

# --- 3. Build --------------------------------------------------------------
$releaseDir = Join-Path $repoRoot 'release'
$existingZip = Get-ChildItem -Path $releaseDir -Filter 'Plith-Setup-*.zip' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($SkipBuild -and $existingZip) {
    Step "Using existing build: $($existingZip.Name) ($([Math]::Round($existingZip.Length / 1MB, 1)) MB)"
    $zipPath = $existingZip.FullName
} else {
    Step "Building installer (folder+zip mode, Norton-resistant)"
    & (Join-Path $repoRoot 'scripts\build-release.ps1')
    if ($LASTEXITCODE -ne 0) { Fail "build-release.ps1 failed. See output above." }

    $newZip = Get-ChildItem -Path $releaseDir -Filter 'Plith-Setup-*.zip' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $newZip) { Fail "Build reported success but no release\Plith-Setup-*.zip was produced." }
    $zipPath = $newZip.FullName
    Write-Host "  Built: $($newZip.Name)"
}

# --- 4. Extract to Desktop -------------------------------------------------
Step "Extracting to Desktop"
$extractDir = Join-Path $env:USERPROFILE 'Desktop\Plith-Install'
if (Test-Path $extractDir) {
    Write-Host "  Removing previous extract at $extractDir..."
    Remove-Item -Path $extractDir -Recurse -Force
}
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force
$launcher = Join-Path $extractDir 'Plith-Installer.exe'
if (-not (Test-Path $launcher)) { Fail "Extract completed but Plith-Installer.exe missing from $extractDir." }

# Clear Zone.Identifier ADS so Windows doesn't prompt "publisher unknown" every launch.
Get-ChildItem -Path $extractDir -Recurse -File | ForEach-Object {
    try { Unblock-File -Path $_.FullName -ErrorAction SilentlyContinue } catch { }
}
Write-Host "  Ready: $launcher"

if ($NoLaunch) {
    Step "Skipping launch (-NoLaunch)"
    Write-Host "Run manually: & '$launcher'"
    exit 0
}

# --- 5. Launch installer ---------------------------------------------------
# Note: the installer is a GUI process. We fire and forget rather than
# waiting on it, so this script exits and the user drives the wizard.
Step "Launching installer"
Start-Process -FilePath $launcher -Verb RunAs
Write-Host "  Installer launched — follow the wizard. Post-install verification below runs after you finish."

# --- 6. Post-install verification -----------------------------------------
# Poll for a fresh Plith.exe with a version >= what we just built. Times out
# after 3 minutes so a stuck installer doesn't hang the script forever.
Step "Waiting for install to complete (checking C:\Program Files\Plith\Plith.exe)"
$installedExe = 'C:\Program Files\Plith\Plith.exe'
$expectedVersion = (Select-String -Path (Join-Path $repoRoot 'src\Plith\Plith.csproj') -Pattern '<Version>(.*?)</Version>').Matches[0].Groups[1].Value
Write-Host "  Expected version: $expectedVersion"

$deadline = (Get-Date).AddMinutes(3)
$lastReported = ''
while ((Get-Date) -lt $deadline) {
    if (Test-Path $installedExe) {
        try {
            $info = (Get-Item $installedExe).VersionInfo
            $current = ($info.ProductVersion -split '\+')[0]
            if ($current -ne $lastReported) {
                Write-Host "  Installed: $current" -NoNewline
                Write-Host ""
                $lastReported = $current
            }
            if ($current -eq $expectedVersion) {
                Write-Host ""
                Write-Host "SUCCESS: Plith $expectedVersion is installed at $installedExe" -ForegroundColor Green
                Write-Host "Launch it from the Start Menu, or run: & '$installedExe'"
                exit 0
            }
        } catch { }
    }
    Start-Sleep -Seconds 2
}

Write-Host ""
Write-Warning "Post-install check timed out after 3 minutes. Either the wizard is still running (finish it and re-run this script with -SkipBuild -SkipGitPull to re-verify), OR the install failed silently. Check %LOCALAPPDATA%\Plith\Installer\install.log for details."
exit 2
