# scripts/nuke-msaccount-with-block.ps1 - stop everything that could recreate
# the account first, THEN nuke, THEN verify it stays gone.
#
# The previous nuke-msaccount-final.ps1 deleted the two known registrations
# (UserExtendedProperties subkey + Credential Manager entries) but Windows
# recreated both of them within seconds. Something running in the background
# is treating the deleted state as inconsistent and re-syncing the account
# from its own cache. Candidates: LiveIdSvc, Outlook / New Outlook, Xbox app
# background service, Microsoft Store, Teams, OneDrive sign-in state.
#
# This script:
#   1. Enumerates suspects that could be the re-writer.
#   2. Kills every Microsoft app process that syncs accounts.
#   3. Stops the two Windows services most implicated.
#   4. Deletes the UserExtendedProperties subkey + Credential Manager entries.
#   5. Waits 15 seconds without touching the machine.
#   6. Re-scans. If the entries are back, the re-writer is one of the still-
#      running services or apps — the script reports which processes are up
#      so you can sign them out or uninstall.
#
# Idempotent. Backs up UEP as .reg to Desktop before deletion.
#
# Usage (ADMIN):
#   .\scripts\nuke-msaccount-with-block.ps1 -Email 'birsensancak76@hotmail.com'

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Email
)

$ErrorActionPreference = 'Continue'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Requires admin PowerShell."
}

function Step([string]$t) { Write-Host ""; Write-Host "==> $t" -ForegroundColor Cyan }
function OK([string]$m)   { Write-Host "  $m" -ForegroundColor Green }
function Warn([string]$m) { Write-Host "  $m" -ForegroundColor Yellow }
function Info([string]$m) { Write-Host "  $m" -ForegroundColor Gray }

# Suspect processes — anything that syncs Microsoft accounts in the background.
$suspectProcs = @(
    'OUTLOOK',              # classic Outlook
    'olk',                  # new Outlook
    'msoutlook',
    'HxOutlook', 'HxTsr',   # Windows Mail / Calendar (old)
    'WindowsPackageManagerServer',
    'ms-teams', 'msteams', 'Teams',
    'XboxApp', 'XboxAppServices',
    'GameBar', 'GameBarPresenceWriter',
    'MicrosoftStore', 'WinStore.App',
    'OneDrive',
    'PeopleExperienceHost',
    'SettingsSyncHost',
    'AuthHost',
    'UserOOBEBroker'
)

# Suspect services — the OS-level identity syncers.
$suspectSvcs = @(
    'wlidsvc',              # Microsoft Account Sign-in Assistant (LiveIdSvc)
    'UserDataSvc',          # User Data Access — email / contacts sync
    'UserDataAccessSvc',
    'UnistoreSvc',          # Unified Store — Mail/Calendar data
    'OneSyncSvc'            # Sync Host — cloud settings
)

# 1. Report current state
Step "1/6 - Current running suspects (before nuke)"
$foundProcs = @()
foreach ($n in $suspectProcs) {
    $p = Get-Process -Name $n -ErrorAction SilentlyContinue
    if ($p) {
        foreach ($proc in $p) {
            Info "PROC $($proc.Name) pid $($proc.Id)"
            $foundProcs += $proc
        }
    }
}
if ($foundProcs.Count -eq 0) { Info "no suspect processes running" }

$foundSvcs = @()
foreach ($sn in $suspectSvcs) {
    $s = Get-Service -Name $sn -ErrorAction SilentlyContinue
    if ($s -and $s.Status -eq 'Running') {
        Info "SVC  $($s.Name) - $($s.DisplayName)"
        $foundSvcs += $s
    }
}
if ($foundSvcs.Count -eq 0) { Info "no suspect services running" }

# 2. Kill suspect processes
Step "2/6 - Killing suspect processes"
foreach ($proc in $foundProcs) {
    try {
        Stop-Process -Id $proc.Id -Force -ErrorAction Stop
        OK "killed $($proc.Name) pid $($proc.Id)"
    } catch {
        Warn "couldn't kill $($proc.Name) pid $($proc.Id): $($_.Exception.Message)"
    }
}

# 3. Stop suspect services (WITHOUT disabling them — we'll leave them stopped
#    for the duration of the nuke + wait window)
Step "3/6 - Stopping suspect services (temporary)"
foreach ($s in $foundSvcs) {
    try {
        Stop-Service -Name $s.Name -Force -ErrorAction Stop
        OK "stopped $($s.Name)"
    } catch {
        Warn "couldn't stop $($s.Name): $($_.Exception.Message)"
    }
}

# 4. Nuke known holdouts
Step "4/6 - Deleting known holdouts"

# UserExtendedProperties subkey
$uep = "HKCU:\Software\Microsoft\IdentityCRL\UserExtendedProperties\$Email"
if (Test-Path $uep) {
    try {
        $bak = Join-Path $env:USERPROFILE ("Desktop\UEP-$Email-$(Get-Date -Format 'yyyyMMdd-HHmmss').reg")
        & reg export "HKCU\Software\Microsoft\IdentityCRL\UserExtendedProperties\$Email" $bak /y 2>&1 | Out-Null
        Remove-Item $uep -Recurse -Force -ErrorAction Stop
        OK "deleted $uep (backup: $bak)"
    } catch { Warn "UEP delete failed: $($_.Exception.Message)" }
} else {
    Info "UEP subkey not present"
}

# Credential Manager entries
$raw = & cmdkey /list 2>&1 | Out-String
$blocks = $raw -split '(?ms)(?=^\s*Target:)'
$cmdkeyHits = @()
foreach ($b in $blocks) {
    if ($b -match [regex]::Escape($Email)) {
        if ($b -match '(?ms)^\s*Target:\s*(.+?)\s*$') {
            $cmdkeyHits += $Matches[1].Trim()
        }
    }
}
foreach ($t in $cmdkeyHits) {
    $short = if ($t -match '^\s*[A-Za-z]+:\s*target=(.+)$') { $Matches[1] } else { $t }
    $r = & cmdkey /delete:"$short" 2>&1
    if ($LASTEXITCODE -eq 0) { OK "deleted credential: $short" }
    else {
        $r2 = & cmdkey /delete:"$t" 2>&1
        if ($LASTEXITCODE -eq 0) { OK "deleted credential (full): $t" }
        else { Warn "credential delete failed: $t ($r $r2)" }
    }
}
if ($cmdkeyHits.Count -eq 0) { Info "no credential manager entries for $Email" }

# 5. Wait — key test: does anything re-create the entries with services stopped?
Step "5/6 - Wait window (15 seconds — see if anything recreates)"
for ($i = 15; $i -gt 0; $i -= 3) {
    Write-Host "  ...$i seconds remaining"
    Start-Sleep -Seconds 3
}

# 6. Re-scan for the holdouts
Step "6/6 - Post-wait verification"
$reappeared = @()
if (Test-Path $uep) {
    $reappeared += "UEP subkey"
    Warn "UEP came back at $uep"
}
$raw2 = & cmdkey /list 2>&1 | Out-String
if ($raw2 -match [regex]::Escape($Email)) {
    $reappeared += "Credential Manager"
    Warn "cmdkey still shows $Email"
}

Write-Host ""
if ($reappeared.Count -eq 0) {
    Write-Host "SUCCESS: nothing re-created the entries." -ForegroundColor Green
    Write-Host "  Restart Windows to make it permanent."
    Write-Host "  After restart the services will auto-start; if the entries come back," -ForegroundColor DarkGray
    Write-Host "  something legitimately-running with your sign-in re-adds them." -ForegroundColor DarkGray
} else {
    Write-Host "STILL COMING BACK. Re-created: $($reappeared -join ', ')" -ForegroundColor Red
    Write-Host ""
    Write-Host "Something is actively rewriting the account even with these" -ForegroundColor Yellow
    Write-Host "services stopped. Currently running Microsoft-account-related" -ForegroundColor Yellow
    Write-Host "processes right now:" -ForegroundColor Yellow
    Get-Process | Where-Object {
        $_.Name -match 'outlook|olk|teams|xbox|onedrive|store|edge|widget|search|photos|game'
    } | Select-Object Name, Id, StartTime -First 20 | Format-Table -AutoSize
    Write-Host ""
    Write-Host "Try: sign out of Outlook (File > Account > Sign Out), close it," -ForegroundColor Yellow
    Write-Host "then re-run this script." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Restarting suspect services back to normal..."
foreach ($s in $foundSvcs) {
    try { Start-Service -Name $s.Name -ErrorAction Stop; OK "started $($s.Name)" }
    catch { Warn "couldn't start $($s.Name): $($_.Exception.Message)" }
}
