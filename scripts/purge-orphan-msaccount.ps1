# scripts/purge-orphan-msaccount.ps1 — deep-clean a Microsoft account that
# won't go away through the normal Settings UI.
#
# When the wrong Microsoft account gets attached at multiple system layers
# (Start Menu tile, AccountPictures on disk with SYSTEM-only ACLs, WAM
# broker registry, Email & Accounts pane), sign-out and Settings > Remove
# don't fully undo it — Windows keeps re-populating the tile from the
# leftovers. This script surgically removes every layer we can reach.
#
# Layers cleaned per email:
#   1. OneAuth account cache        %LOCALAPPDATA%\Microsoft\OneAuth\accounts\<id>
#   2. MSAL token cache             %LOCALAPPDATA%\Microsoft\OneAuth\.msalcache.bin
#   3. AccountPictures on disk      C:\Users\Public\AccountPictures\<SID>\*.jpg
#                                   (takeown + icacls first — SYSTEM ACL blocks
#                                    even elevated admin from a plain Remove-Item)
#   4. AccountPicture registry      HKLM:\...\AccountPicture\Users\<SID>\Image*
#   5. IdentityStore\Cache          HKLM:\...\IdentityStore\Cache\<SID>\<providers>
#   6. Start Menu / Shell hosts     killed so the tile rebuilds from clean state
#
# Layers NOT touched:
#   - Windows sign-in account       your local sari\sari stays untouched
#   - listlessem (or any account other than the one you name)
#
# Usage from ADMIN PowerShell:
#
#   .\scripts\purge-orphan-msaccount.ps1 -Email 'birsensancak76@hotmail.com'
#
# Optional switches:
#   -SkipPictures         don't touch the AccountPictures on-disk files
#   -SkipStartMenuKick    don't restart StartMenuExperienceHost / Explorer
#   -DryRun               show what WOULD be deleted; make no changes

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Email,

    [switch]$SkipPictures,
    [switch]$SkipStartMenuKick,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Requires admin PowerShell — some layers (AccountPictures, HKLM registry) need elevation. Right-click PowerShell -> Run as administrator."
}

function Step([string]$t) { Write-Host ""; Write-Host "==> $t" -ForegroundColor Cyan }
function OK([string]$m)   { Write-Host "  $m" -ForegroundColor Green }
function Warn([string]$m) { Write-Host "  $m" -ForegroundColor Yellow }
function Skipped([string]$m) { Write-Host "  $m" -ForegroundColor DarkGray }

$sid = (Get-LocalUser -Name $env:USERNAME).SID.Value
Write-Host "Purging Microsoft account: $Email"
Write-Host "  Local Windows SID: $sid"
if ($DryRun) { Write-Host "  DRY RUN (no changes will be made)" -ForegroundColor Yellow }

# ---------------------------------------------------------------------------
# 1. OneAuth accounts directory
# ---------------------------------------------------------------------------
Step "Layer 1/6 - OneAuth accounts"
$oneAuthAccts = "$env:LOCALAPPDATA\Microsoft\OneAuth\accounts"
$oneAuthHits = @()
if (Test-Path $oneAuthAccts) {
    Get-ChildItem $oneAuthAccts -Directory -EA 0 | ForEach-Object {
        $inner = Join-Path $_.FullName $_.Name
        if (Test-Path $inner) {
            try {
                $t = [Text.Encoding]::Unicode.GetString([IO.File]::ReadAllBytes($inner))
                if ($t -match [regex]::Escape($Email)) { $oneAuthHits += $_.FullName }
            } catch {}
        }
    }
}
if ($oneAuthHits.Count -eq 0) {
    Skipped "OneAuth: no cache entries reference '$Email'."
} else {
    foreach ($h in $oneAuthHits) {
        if ($DryRun) { Warn "would delete $h" }
        else { Remove-Item $h -Recurse -Force -EA SilentlyContinue; OK "deleted $h" }
    }
}

# ---------------------------------------------------------------------------
# 2. MSAL token cache
# ---------------------------------------------------------------------------
Step "Layer 2/6 - MSAL token cache"
$msal = "$env:LOCALAPPDATA\Microsoft\OneAuth\.msalcache.bin"
if (Test-Path $msal) {
    if ($DryRun) { Warn "would clear $msal" }
    else { Remove-Item $msal -Force -EA SilentlyContinue; OK "cleared $msal" }
} else {
    Skipped "MSAL cache absent."
}

# ---------------------------------------------------------------------------
# 3. AccountPictures on disk (needs takeown + icacls — SYSTEM-owned)
# ---------------------------------------------------------------------------
Step "Layer 3/6 - AccountPictures on disk"
if ($SkipPictures) {
    Skipped "skipped by -SkipPictures"
} else {
    $picDir = "C:\Users\Public\AccountPictures\$sid"
    if (Test-Path $picDir) {
        $pics = Get-ChildItem $picDir -File -Filter '*.jpg' -EA 0
        if ($pics.Count -eq 0) {
            Skipped "no .jpg files in $picDir"
        } else {
            Write-Host "  Found $($pics.Count) picture file(s). Reclaiming ownership..."
            if ($DryRun) {
                Warn "would takeown /F <dir> /A + icacls /grant Administrators:F + Remove-Item"
            } else {
                & takeown /F $picDir /A 2>&1 | Out-Null
                & takeown /F "$picDir\*" /A 2>&1 | Out-Null
                & icacls $picDir /grant '*S-1-5-32-544:F' /T /C /Q 2>&1 | Out-Null
                $deleted = 0
                foreach ($p in $pics) {
                    try { Remove-Item $p.FullName -Force -EA Stop; $deleted++ }
                    catch { Warn "still couldn't delete $($p.Name): $($_.Exception.Message)" }
                }
                OK "deleted $deleted / $($pics.Count) picture(s)"
            }
        }
    } else {
        Skipped "no picture dir at $picDir"
    }
}

# ---------------------------------------------------------------------------
# 4. AccountPicture registry pointers
# ---------------------------------------------------------------------------
Step "Layer 4/6 - AccountPicture registry"
$accPicKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AccountPicture\Users\$sid"
if (Test-Path $accPicKey) {
    if ($DryRun) { Warn "would clear all Image* values under $accPicKey" }
    else {
        $props = (Get-ItemProperty $accPicKey).PSObject.Properties | Where-Object { $_.Name -like 'Image*' }
        foreach ($p in $props) {
            try { Remove-ItemProperty -Path $accPicKey -Name $p.Name -EA Stop } catch {}
        }
        OK "cleared $($props.Count) Image* registry pointer(s)"
    }
} else {
    Skipped "no registry key at $accPicKey"
}

# ---------------------------------------------------------------------------
# 5. IdentityStore\Cache (system-level identity broker map)
# ---------------------------------------------------------------------------
Step "Layer 5/6 - IdentityStore cache"
$idCache = "HKLM:\SOFTWARE\Microsoft\IdentityStore\Cache\$sid"
if (Test-Path $idCache) {
    $found = 0
    Get-ChildItem $idCache -Recurse -EA 0 | ForEach-Object {
        $p = Get-ItemProperty $_.PSPath -EA 0
        $blob = "$($p.IdentityName)|$($p.UserName)|$($p.DisplayName)"
        if ($blob -match [regex]::Escape($Email)) {
            if ($DryRun) { Warn "would delete $($_.Name)" }
            else {
                try { Remove-Item $_.PSPath -Recurse -Force -EA Stop; OK "deleted $($_.PSChildName)"; $found++ }
                catch { Warn "couldn't delete $($_.PSChildName): $($_.Exception.Message)" }
            }
        }
    }
    if ($found -eq 0) { Skipped "no IdentityStore entries matched '$Email'" }
} else {
    Skipped "no IdentityStore cache for your SID"
}

# ---------------------------------------------------------------------------
# 6. Restart Start Menu + Explorer so the tile rebuilds from clean state
# ---------------------------------------------------------------------------
Step "Layer 6/6 - Restart shell hosts"
if ($SkipStartMenuKick) {
    Skipped "skipped by -SkipStartMenuKick"
} elseif ($DryRun) {
    Warn "would kill StartMenuExperienceHost / ShellExperienceHost / Explorer (they auto-restart)"
} else {
    Stop-Process -Name 'StartMenuExperienceHost' -Force -EA SilentlyContinue
    Stop-Process -Name 'ShellExperienceHost' -Force -EA SilentlyContinue
    Stop-Process -Name 'explorer' -Force -EA SilentlyContinue
    Start-Sleep -Seconds 2
    if (-not (Get-Process -Name explorer -EA SilentlyContinue)) { Start-Process explorer.exe }
    OK "shell hosts restarted"
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Yellow
Write-Host "  DONE. Two manual UI steps still needed:" -ForegroundColor Yellow
Write-Host "==================================================" -ForegroundColor Yellow
Write-Host ""
Write-Host "  A) The Settings > Email & accounts entry for '$Email' may STILL"
Write-Host "     show up because Windows registered it via the 'Add a Microsoft"
Write-Host "     account' flow, which lives in the account database we can't"
Write-Host "     surgically edit from PowerShell without breaking anything."
Write-Host ""
Write-Host "     Manual: Settings > Accounts > Email & accounts"
Write-Host "             -> click $Email -> if 'Remove' greyed / missing:"
Write-Host "               1. change 'Sign in options' to"
Write-Host "                  'All apps need to ask me to use this account'"
Write-Host "               2. reload the page (F5 the Settings app)"
Write-Host "               3. 'Remove' should now appear -> Remove"
Write-Host ""
Write-Host "  B) Restart Windows one full time. The Start Menu account tile"
Write-Host "     picture cache lives in a couple of places that only refresh"
Write-Host "     on next login. After the restart the tile should show the"
Write-Host "     default silhouette + your listlessem@hotmail.com account."
Write-Host ""
