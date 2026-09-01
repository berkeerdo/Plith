# scripts/nuke-msaccount-nuclear.ps1 - the last-resort sweep for a Microsoft
# account that persistently re-appears in Settings > Email & accounts.
#
# Diagnosis (from research):
# Settings > Email & accounts is an AGGREGATOR VIEW over WAM
# (Windows.Security.Authentication.Web.Core / Windows.Security.Credentials.
# PasswordVault). Any signed-in WAM PROVIDER APP (New Outlook, Store, Xbox,
# Teams personal, Edge profile, OneDrive) republishes its WebAccounts into
# the aggregation, and Settings will show them with no Remove button
# because the account is owned by the PROVIDER, not by the user.
#
# The most common re-injector is New Outlook (olk.exe). It MUST be removed
# or signed out BEFORE this script runs. Do this first:
#
#   Get-AppxPackage *Microsoft.OutlookForWindows* | Remove-AppxPackage
#   Get-AppxPackage *microsoft.windowscommunicationsapps* | Remove-AppxPackage
#   # Sign out of Microsoft Store, Xbox app, Teams personal, Edge profile, OneDrive
#
# Then run this script. It:
#   1. Stops TokenBroker and wlidsvc so no service can rewrite mid-nuke.
#   2. Deletes every OneAuth / IdentityCache / TokenBroker cache directory.
#   3. Purges the AAD + MSA broker plugin package data (they cache MSAs too
#      despite the misleading AAD name).
#   4. Resets the AccountsControl + AAD/MSA broker AppX packages so their
#      cached WebAccount list gets rebuilt from an empty state.
#   5. Cleans the registry tail sweep the previous scripts missed
#      (IdentityCRL\Immersive\production\Token, Office 16.0 Identity,
#      Local Settings package classes).
#   6. Restarts services, prompts you to reboot.
#
# Usage (ADMIN):
#   .\scripts\nuke-msaccount-nuclear.ps1 -Email 'birsensancak76@hotmail.com'

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Email,

    [switch]$SkipAppxReset
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

# Warn about the New Outlook prerequisite
Step "PRE-CHECK - New Outlook package status"
$olk = Get-AppxPackage *Microsoft.OutlookForWindows* -ErrorAction SilentlyContinue
if ($olk) {
    Warn "New Outlook package is STILL INSTALLED. It will republish the account."
    Warn "Remove it first: Get-AppxPackage *Microsoft.OutlookForWindows* | Remove-AppxPackage"
    Warn "Press Ctrl+C to abort or wait 10 seconds to continue anyway..."
    Start-Sleep -Seconds 10
} else {
    OK "New Outlook not installed. Good."
}
$mail = Get-AppxPackage *microsoft.windowscommunicationsapps* -ErrorAction SilentlyContinue
if ($mail) {
    Warn "Windows Mail/Calendar package installed. It can also republish."
    Warn "Remove: Get-AppxPackage *microsoft.windowscommunicationsapps* | Remove-AppxPackage"
    Start-Sleep -Seconds 5
}

# 1. Stop the two services that would rewrite mid-nuke
Step "1/6 - Stopping TokenBroker + wlidsvc"
foreach ($svc in @('TokenBroker', 'wlidsvc')) {
    $s = Get-Service -Name $svc -ErrorAction SilentlyContinue
    if ($s -and $s.Status -eq 'Running') {
        try { Stop-Service -Name $svc -Force -ErrorAction Stop; OK "stopped $svc" }
        catch { Warn "couldn't stop $svc : $($_.Exception.Message)" }
    } else { Info "$svc not running" }
}

# 2. Nuclear cache sweep
Step "2/6 - Deleting OneAuth / IdentityCache / TokenBroker caches"
$dirs = @(
    "$env:LOCALAPPDATA\Microsoft\OneAuth",
    "$env:LOCALAPPDATA\Microsoft\IdentityCache",
    "$env:LOCALAPPDATA\Microsoft\TokenBroker\Cache"
)
foreach ($d in $dirs) {
    if (Test-Path $d) {
        try { Remove-Item $d -Recurse -Force -ErrorAction Stop; OK "deleted $d" }
        catch { Warn "couldn't delete $d : $($_.Exception.Message)" }
    } else { Info "not present: $d" }
}

# 3. Purge broker plugin package data
Step "3/6 - Purging broker plugin package data (AAD + MSA)"
$brokerPkgs = @(
    "$env:LOCALAPPDATA\Packages\Microsoft.AAD.BrokerPlugin_cw5n1h2txyewy",
    "$env:LOCALAPPDATA\Packages\Microsoft.MicrosoftAccountBrokerPlugin_cw5n1h2txyewy",
    "$env:LOCALAPPDATA\Packages\Microsoft.AccountsControl_cw5n1h2txyewy"
)
foreach ($p in $brokerPkgs) {
    if (Test-Path $p) {
        # Delete contents but preserve the package folder (else Windows may not
        # re-hydrate the package data structure correctly).
        try {
            Get-ChildItem $p -Force -ErrorAction SilentlyContinue |
                Remove-Item -Recurse -Force -ErrorAction Stop
            OK "cleaned $p"
        } catch { Warn "partial clean of $p : $($_.Exception.Message)" }
    } else { Info "package folder absent: $p" }
}

# 4. Reset the broker AppX packages so they rebuild from empty
if (-not $SkipAppxReset) {
    Step "4/6 - Reset broker AppX packages"
    $pkgs = @()
    $pkgs += Get-AppxPackage -Name 'Microsoft.AAD.BrokerPlugin' -ErrorAction SilentlyContinue
    $pkgs += Get-AppxPackage -Name 'Microsoft.MicrosoftAccountBrokerPlugin' -ErrorAction SilentlyContinue
    $pkgs += Get-AppxPackage -Name 'Microsoft.AccountsControl' -ErrorAction SilentlyContinue
    foreach ($pk in $pkgs | Where-Object { $_ }) {
        try {
            Reset-AppxPackage -Package $pk.PackageFullName -ErrorAction Stop
            OK "reset $($pk.Name)"
        } catch { Warn "reset failed for $($pk.Name): $($_.Exception.Message)" }
    }
} else {
    Info "AppX reset skipped (--SkipAppxReset)"
}

# 5. Registry tail sweep
Step "5/6 - Registry tail sweep"

# IdentityCRL Immersive tokens
$immToken = 'HKCU:\Software\Microsoft\IdentityCRL\Immersive\production\Token'
if (Test-Path $immToken) {
    $hit = 0
    Get-ChildItem $immToken -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
        $p = Get-ItemProperty $_.PSPath -EA 0
        $blob = "$($p | Out-String)"
        if ($blob -match [regex]::Escape($Email) -or $_.Name -match [regex]::Escape($Email)) {
            try { Remove-Item $_.PSPath -Recurse -Force -EA Stop; OK "deleted $($_.PSChildName)"; $hit++ }
            catch { Warn "couldn't delete $($_.PSChildName): $($_.Exception.Message)" }
        }
    }
    if ($hit -eq 0) { Info "no Immersive\Token subkey matched" }
}

# Office 16.0 Identity roots (Office republishes to WAM from here)
$officeIdentities = 'HKCU:\Software\Microsoft\Office\16.0\Common\Identity\Identities'
if (Test-Path $officeIdentities) {
    Get-ChildItem $officeIdentities -EA 0 | ForEach-Object {
        $p = Get-ItemProperty $_.PSPath -EA 0
        if ($p.EmailAddress -eq $Email -or ($p | Out-String) -match [regex]::Escape($Email)) {
            try { Remove-Item $_.PSPath -Recurse -Force -EA Stop; OK "deleted Office identity $($_.PSChildName)" }
            catch { Warn "couldn't delete Office identity: $($_.Exception.Message)" }
        }
    }
}

# UserExtendedProperties (again — should be gone after service stop)
$uep = "HKCU:\Software\Microsoft\IdentityCRL\UserExtendedProperties\$Email"
if (Test-Path $uep) {
    try { Remove-Item $uep -Recurse -Force -EA Stop; OK "deleted UEP subkey" }
    catch { Warn "UEP delete failed: $($_.Exception.Message)" }
}

# 6. Restart services + prompt reboot
Step "6/6 - Restart services + reboot required"
foreach ($svc in @('TokenBroker', 'wlidsvc')) {
    $s = Get-Service -Name $svc -ErrorAction SilentlyContinue
    if ($s) {
        try { Start-Service -Name $svc -ErrorAction Stop; OK "started $svc" }
        catch { Warn "couldn't start $svc : $($_.Exception.Message)" }
    }
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Green
Write-Host "  Nuclear sweep complete." -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  RESTART WINDOWS NOW."
Write-Host ""
Write-Host "  After restart, check Settings > Accounts > Email & accounts."
Write-Host "  '$Email' should be gone."
Write-Host ""
Write-Host "  If it STILL comes back after reboot, either:"
Write-Host "    (a) New Outlook is still installed (this script warned you at the top)"
Write-Host "    (b) An app you signed into with '$Email' is set to auto-launch"
Write-Host "        (Store / Xbox / Teams personal / OneDrive / Edge with a synced profile)"
Write-Host "    (c) The last resort is a fresh Windows profile — Microsoft's own"
Write-Host "        documented answer for this case. See:"
Write-Host "        https://learn.microsoft.com/en-us/answers/questions/4280663"
Write-Host ""
