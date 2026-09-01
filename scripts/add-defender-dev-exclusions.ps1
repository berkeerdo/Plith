# scripts/add-defender-dev-exclusions.ps1 — one-shot Windows Defender exclusion
# setup for a developer workstation. Run from an admin PowerShell after
# uninstalling Norton (Defender activates automatically when the third-party AV
# is gone). Idempotent — safe to re-run any time; existing exclusions are kept.
#
# Adds:
#   - C:\Projects (all Plith / Moditra / Finexot / etc. repos)
#   - %USERPROFILE%\.dotnet, .nuget, .npm  (SDK caches AV loves to scan)
#   - %LOCALAPPDATA%\Temp\.net             (single-file bundle self-extract dir)
#   - %LOCALAPPDATA%\Programs\Microsoft VS Code, JetBrains (IDE self-updates)
#   - dotnet.exe / MSBuild.exe / node.exe / powershell processes
#
# Undo any entry with Remove-MpPreference -ExclusionPath 'C:\Projects'.
# List all exclusions:   Get-MpPreference | Select-Object -Expand ExclusionPath

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "This script requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

# Verify Defender is the active AV. If Norton or another third-party AV is
# still registered, most Add-MpPreference calls silently no-op because
# Defender itself is in passive mode.
$avStatus = Get-MpComputerStatus -ErrorAction SilentlyContinue
if (-not $avStatus) {
    throw "Windows Defender is not available on this system. Uninstall your third-party AV first."
}
if (-not $avStatus.AntivirusEnabled) {
    Write-Warning "Windows Defender is registered but not the active antivirus (probably a third-party AV is running in the foreground). Exclusions will only apply once Defender takes over. Uninstall the other AV and re-run."
}

# Folder exclusions — all dev work, all SDK caches, and .NET's single-file
# bundle self-extract directory (the one that used to trigger Norton crashes).
$folders = @(
    'C:\Projects',
    "$env:USERPROFILE\.dotnet",
    "$env:USERPROFILE\.nuget",
    "$env:USERPROFILE\.npm",
    "$env:LOCALAPPDATA\Temp\.net",
    "$env:LOCALAPPDATA\NuGet",
    "$env:LOCALAPPDATA\Programs\Microsoft VS Code",
    "$env:LOCALAPPDATA\JetBrains"
)

# Process exclusions — the .NET SDK toolchain and common dev binaries. Excluding
# by process name means Defender skips ANY file these processes read/write, so
# a compile that touches thousands of files stops paying the per-file scan cost.
$processes = @(
    'dotnet.exe',
    'MSBuild.exe',
    'VBCSCompiler.exe',
    'node.exe',
    'npm.cmd',
    'Code.exe',
    'devenv.exe',
    'powershell.exe',
    'pwsh.exe'
)

Write-Host "Adding $($folders.Count) folder exclusions..."
foreach ($f in $folders) {
    if (Test-Path $f) {
        try {
            Add-MpPreference -ExclusionPath $f -ErrorAction Stop
            Write-Host "  OK  $f"
        } catch {
            Write-Warning "  skipped $f -> $($_.Exception.Message)"
        }
    } else {
        Write-Host "  ..  $f  (doesn't exist yet — skipped)"
    }
}

Write-Host ""
Write-Host "Adding $($processes.Count) process exclusions..."
foreach ($p in $processes) {
    try {
        Add-MpPreference -ExclusionProcess $p -ErrorAction Stop
        Write-Host "  OK  $p"
    } catch {
        Write-Warning "  skipped $p -> $($_.Exception.Message)"
    }
}

Write-Host ""
Write-Host "Done. Verify with:"
Write-Host "  Get-MpPreference | Select-Object -Expand ExclusionPath"
Write-Host "  Get-MpPreference | Select-Object -Expand ExclusionProcess"
