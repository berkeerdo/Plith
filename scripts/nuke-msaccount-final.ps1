# scripts/nuke-msaccount-final.ps1 - final surgical removal of a Microsoft
# account when find-msaccount-everywhere.ps1 shows it's still lodged in
# IdentityCRL\UserExtendedProperties and Windows Credential Manager.
#
# The UserExtendedProperties subkey is the specific registration Windows
# Settings > Email & accounts reads to decide the account is "known". While
# that key exists, Settings shows the account and disables its Remove button
# (because Windows treats it as load-bearing for other subsystems). Deleting
# the subkey de-registers the account entirely — Settings loses the entry.
#
# Credential Manager stores auto-fill username/password pairs. Windows apps
# that once used the account (Store, Outlook, Xbox) leave targets there; when
# those apps re-launch, they read the target list and populate the account
# picker with the leftover email. Cleaning them removes that source.
#
# Usage from ADMIN PowerShell:
#   .\scripts\nuke-msaccount-final.ps1 -Email 'birsensancak76@hotmail.com'
#
# -DryRun to preview without changes.

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Email,

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Requires admin PowerShell."
}

function Step([string]$t) { Write-Host ""; Write-Host "==> $t" -ForegroundColor Cyan }
function OK([string]$m)   { Write-Host "  $m" -ForegroundColor Green }
function Warn([string]$m) { Write-Host "  $m" -ForegroundColor Yellow }
function Skipped([string]$m) { Write-Host "  $m" -ForegroundColor DarkGray }

# ---------------------------------------------------------------------------
# 1. HKCU:\Software\Microsoft\IdentityCRL\UserExtendedProperties\<email>
# ---------------------------------------------------------------------------
Step "IdentityCRL UserExtendedProperties"
$uepKey = "HKCU:\Software\Microsoft\IdentityCRL\UserExtendedProperties\$Email"
if (Test-Path $uepKey) {
    if ($DryRun) {
        Warn "would delete $uepKey"
    } else {
        try {
            # Back up first — Desktop\IdentityCRL-UEP-<email>-<ts>.reg
            $bakFile = Join-Path $env:USERPROFILE ("Desktop\IdentityCRL-UEP-$Email-$(Get-Date -Format 'yyyyMMdd-HHmmss').reg")
            & reg export "HKCU\Software\Microsoft\IdentityCRL\UserExtendedProperties\$Email" $bakFile /y 2>&1 | Out-Null
            OK "backed up to $bakFile"
            Remove-Item $uepKey -Recurse -Force -ErrorAction Stop
            OK "deleted $uepKey"
        } catch {
            Warn "couldn't delete $uepKey : $($_.Exception.Message)"
        }
    }
} else {
    Skipped "no key at $uepKey (already gone)"
}

# ---------------------------------------------------------------------------
# 2. Windows Credential Manager (cmdkey targets referencing the email)
# ---------------------------------------------------------------------------
Step "Windows Credential Manager"
$rawList = & cmdkey /list 2>&1 | Out-String
$targets = @()
# Parse cmdkey /list output. Each credential block starts with "Target:" line.
# Group blocks that contain the email, capture their Target string.
$blocks = $rawList -split '(?ms)(?=^\s*Target:)'
foreach ($b in $blocks) {
    if ($b -match [regex]::Escape($Email)) {
        if ($b -match '(?ms)^\s*Target:\s*(.+?)\s*$') {
            $t = $Matches[1].Trim()
            $targets += $t
        }
    }
}

if ($targets.Count -eq 0) {
    Skipped "no Credential Manager targets reference '$Email'"
} else {
    foreach ($t in $targets) {
        # cmdkey /list prefixes some targets with 'LegacyGeneric:target=' etc.
        # cmdkey /delete accepts either the full prefixed name or just the
        # part after 'target='. Try the full form first.
        if ($DryRun) {
            Warn "would delete Credential Manager target: $t"
        } else {
            $short = if ($t -match '^\s*[A-Za-z]+:\s*target=(.+)$') { $Matches[1] } else { $t }
            $r = & cmdkey /delete:"$short" 2>&1
            if ($LASTEXITCODE -eq 0) {
                OK "deleted credential target: $short"
            } else {
                # Fallback: try the full label from cmdkey /list
                $r2 = & cmdkey /delete:"$t" 2>&1
                if ($LASTEXITCODE -eq 0) { OK "deleted credential target (full form): $t" }
                else { Warn "couldn't delete '$t': $r $r2" }
            }
        }
    }
}

# ---------------------------------------------------------------------------
# 3. Restart shell hosts + Settings app so Windows re-reads state
# ---------------------------------------------------------------------------
Step "Restarting shell hosts + Settings app"
if ($DryRun) {
    Warn "would kill StartMenuExperienceHost / ShellExperienceHost / Explorer / SystemSettings"
} else {
    Stop-Process -Name 'SystemSettings' -Force -EA SilentlyContinue
    Stop-Process -Name 'StartMenuExperienceHost' -Force -EA SilentlyContinue
    Stop-Process -Name 'ShellExperienceHost' -Force -EA SilentlyContinue
    Stop-Process -Name 'explorer' -Force -EA SilentlyContinue
    Start-Sleep -Seconds 2
    if (-not (Get-Process -Name explorer -EA SilentlyContinue)) { Start-Process explorer.exe }
    OK "shell hosts + Settings restarted"
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Green
Write-Host "  DONE." -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Now open Settings > Accounts > Email & accounts."
Write-Host "  '$Email' should be gone from the list entirely."
Write-Host ""
Write-Host "  If it's STILL there:"
Write-Host "    1. Restart Windows (full restart, not just sign out)."
Write-Host "    2. If still there after restart, run:"
Write-Host "         .\scripts\find-msaccount-everywhere.ps1 -Email '$Email'"
Write-Host "       and share the output — there's a Windows subsystem we haven't"
Write-Host "       reached yet (rare but possible)."
Write-Host ""
