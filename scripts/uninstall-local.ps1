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
    Write-Host "$installDir not found -- nothing to remove."
}

# 3. Clean up the HKCU\Run autostart entry that pointed at the now-deleted exe.
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$runValue = Get-ItemProperty -Path $runKey -Name 'Plith' -ErrorAction SilentlyContinue
if ($runValue) {
    Remove-ItemProperty -Path $runKey -Name 'Plith'
    Write-Host "Removed HKCU\Run autostart entry."
}

Write-Host "Done. Cert remains in CurrentUser\My + LocalMachine\TrustedPublisher for next install."
