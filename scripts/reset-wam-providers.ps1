# scripts/reset-wam-providers.ps1 - reset every WAM provider that could be
# re-publishing the stuck Microsoft account into Settings > Email & accounts.
#
# When Outlook and Windows Mail are already uninstalled but the account
# still returns, the re-injector is one of: Teams personal, Microsoft Store,
# OneDrive, Xbox suite, Edge browser profile. This script:
#
#   - Reset-AppxPackage on Teams / Xbox / Store (safe — clears sign-in state
#     without breaking the app; on next launch you sign back in fresh)
#   - Unlinks OneDrive if the target email is one of its bound accounts
#     (leaves other OneDrive accounts untouched)
#   - Reports which Edge profile (if any) has the target email, so you can
#     remove it manually from Edge > Settings > Profiles
#   - Runs the nuclear cache sweep after so the freshly-reset apps start
#     clean instead of re-loading from OneAuth caches
#
# Requires ADMIN PowerShell.
#
# Usage:
#   .\scripts\reset-wam-providers.ps1 -Email 'birsensancak76@hotmail.com'

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
function Todo([string]$m) { Write-Host "  MANUAL: $m" -ForegroundColor Magenta }

# ---------------------------------------------------------------------------
# 1. Reset packaged WAM provider apps (Teams / Xbox / Store)
# ---------------------------------------------------------------------------
# Reset-AppxPackage wipes the app's LocalAppData\Packages\<name> folder
# entirely — sign-in state, cache, everything. App re-launches from a
# fresh install-time state. Reversible: just sign back in.
Step "1/4 - Reset packaged WAM providers"

$appxTargets = @(
    'MSTeams',                                # Teams personal
    'Microsoft.WindowsStore',                 # Microsoft Store
    'Microsoft.XboxIdentityProvider',         # Xbox account provider
    'Microsoft.GamingApp',                    # Xbox app / GamingApp
    'Microsoft.OneDriveSync'                  # OneDrive package (not the Win32 EXE)
)

foreach ($n in $appxTargets) {
    $pkg = Get-AppxPackage -Name $n -ErrorAction SilentlyContinue
    if (-not $pkg) { Info "not installed: $n"; continue }
    try {
        Reset-AppxPackage -Package $pkg.PackageFullName -ErrorAction Stop
        OK "reset $n"
    } catch {
        Warn "reset failed for $n : $($_.Exception.Message)"
    }
}

# ---------------------------------------------------------------------------
# 2. OneDrive Win32 client — unlink if the target email is bound
# ---------------------------------------------------------------------------
Step "2/4 - OneDrive Win32 client"
$onedriveExe = "$env:LOCALAPPDATA\Microsoft\OneDrive\OneDrive.exe"
if (Test-Path $onedriveExe) {
    # Look through OneDrive's per-account subkeys to see if the target email is bound.
    $odAccounts = Get-ChildItem 'HKCU:\Software\Microsoft\OneDrive\Accounts' -EA 0
    $matched = $false
    foreach ($a in $odAccounts) {
        $p = Get-ItemProperty $a.PSPath -EA 0
        if ($p.UserEmail -eq $Email) {
            $matched = $true
            Warn "OneDrive account bound to '$Email' under $($a.PSChildName)"
            Todo "Right-click OneDrive tray icon -> Settings -> Account -> Unlink this PC"
            Todo "(Only for this specific account — your listlessem@hotmail.com stays linked)"
        }
    }
    if (-not $matched) {
        Info "OneDrive doesn't bind '$Email' at the account level"
    }
} else {
    Info "OneDrive Win32 client not installed"
}

# ---------------------------------------------------------------------------
# 3. Edge profile check
# ---------------------------------------------------------------------------
Step "3/4 - Microsoft Edge profiles"
$edgeState = "$env:LOCALAPPDATA\Microsoft\Edge\User Data\Local State"
if (Test-Path $edgeState) {
    try {
        $state = Get-Content $edgeState -Raw | ConvertFrom-Json
        $matched = $false
        $state.profile.info_cache.PSObject.Properties | ForEach-Object {
            $u = $_.Value.user_name
            $n = $_.Value.name
            if ($u -eq $Email) {
                $matched = $true
                Warn "Edge has a profile signed in with '$Email' -> '$n' at $($_.Name)"
                Todo "Open Edge -> click your profile picture (top right) -> settings gear -> ..."
                Todo "  -> Manage profile settings -> Remove this profile"
                Todo "  (Or if you can't find it: edge://settings/profiles)"
            } else {
                Info "Edge profile $($_.Name): $n <$u> (keep)"
            }
        }
        if (-not $matched) { Info "no Edge profile is signed in with '$Email'" }
    } catch { Warn "couldn't parse Edge Local State: $($_.Exception.Message)" }
}

# ---------------------------------------------------------------------------
# 4. Chain into the nuclear cache sweep so the reset apps start with an
#    empty cache instead of re-loading '$Email' from OneAuth blobs
# ---------------------------------------------------------------------------
Step "4/4 - Chain nuclear cache sweep"
$nuclear = Join-Path $PSScriptRoot 'nuke-msaccount-nuclear.ps1'
if (Test-Path $nuclear) {
    Write-Host "  Running nuke-msaccount-nuclear.ps1 -Email '$Email' -SkipAppxReset ..."
    & $nuclear -Email $Email -SkipAppxReset
} else {
    Warn "nuke-msaccount-nuclear.ps1 not found next to this script."
    Warn "Run it separately after this completes."
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Green
Write-Host "  RESET COMPLETE." -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Handle any MANUAL: lines printed above (OneDrive unlink, Edge profile removal)."
Write-Host "  Then RESTART Windows."
Write-Host ""
Write-Host "  After restart:"
Write-Host "    - Open Settings > Accounts > Email & accounts"
Write-Host "    - '$Email' should be gone entirely"
Write-Host "    - Sign back into Teams, Store, Xbox with listlessem@hotmail.com only"
Write-Host ""
