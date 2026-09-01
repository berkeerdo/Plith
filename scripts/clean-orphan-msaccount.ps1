# scripts/clean-orphan-msaccount.ps1 — remove a stale Microsoft account cache
# entry from Windows' OneAuth token store.
#
# When you sign into Outlook / Teams / any Microsoft app with the wrong
# Microsoft account by mistake, Windows' unified auth cache (OneAuth) keeps
# a record of that account and starts offering it as a sign-in default in
# every other Microsoft app that reads the same cache (Microsoft Defender
# consumer app, Store, Xbox, Teams, Office...). This script surgically
# removes one account entry by email, backs it up first, and clears the
# MSAL token cache so the next sign-in prompt reads clean.
#
# Usage from PowerShell (no admin needed — writes only to %LOCALAPPDATA%):
#
#   # List all cached MS accounts first
#   .\scripts\clean-orphan-msaccount.ps1 -List
#
#   # Remove one account by email
#   .\scripts\clean-orphan-msaccount.ps1 -Email 'wrongaccount@hotmail.com'
#
#   # Or remove by internal OneAuth id if you already have it
#   .\scripts\clean-orphan-msaccount.ps1 -AccountId 'e6f3a41207172275'
#
# The removed entry is copied to Desktop\OneAuth-BACKUP-<timestamp>\ before
# deletion; restore by copying back and restarting Windows.

[CmdletBinding(DefaultParameterSetName = 'ByEmail')]
param(
    [Parameter(ParameterSetName = 'List')]
    [switch]$List,

    [Parameter(ParameterSetName = 'ByEmail', Mandatory)]
    [string]$Email,

    [Parameter(ParameterSetName = 'ById', Mandatory)]
    [string]$AccountId,

    [switch]$SkipMsalCacheClear
)

$ErrorActionPreference = 'Stop'

$oneAuthRoot = Join-Path $env:LOCALAPPDATA 'Microsoft\OneAuth'
$accountsDir = Join-Path $oneAuthRoot 'accounts'
if (-not (Test-Path $accountsDir)) {
    Write-Host "OneAuth accounts directory not found at $accountsDir — nothing to clean." -ForegroundColor Yellow
    exit 0
}

# Helper: read the email out of a OneAuth account cache file. The file is a
# UTF-16LE blob with the email embedded as a plain substring; regex is enough
# to fish it back out.
function Get-AccountEmail {
    param([string]$AcctDir)
    $inner = Join-Path $AcctDir (Split-Path $AcctDir -Leaf)
    if (-not (Test-Path $inner)) { return $null }
    try {
        $bytes = [IO.File]::ReadAllBytes($inner)
        $text  = [System.Text.Encoding]::Unicode.GetString($bytes)
        $m = [regex]::Matches($text, '[\w\.\-]+@[\w\.\-]+\.[a-zA-Z]{2,}') |
             Select-Object -ExpandProperty Value -Unique
        return $m
    } catch { return $null }
}

# LIST MODE: enumerate every cached account and its email(s).
if ($List) {
    Write-Host "OneAuth cached Microsoft accounts:" -ForegroundColor Cyan
    Get-ChildItem $accountsDir -Directory -EA 0 | ForEach-Object {
        $emails = Get-AccountEmail -AcctDir $_.FullName
        $tag = if ($emails) { $emails -join ', ' } else { '(no email found in cache blob)' }
        Write-Host "  $($_.Name)  ->  $tag"
    }
    exit 0
}

# TARGET RESOLUTION: locate the account directory to delete.
$targetDir = $null
if ($AccountId) {
    $candidate = Join-Path $accountsDir $AccountId
    if (Test-Path $candidate) { $targetDir = $candidate }
} elseif ($Email) {
    $lookup = $Email.ToLowerInvariant()
    foreach ($d in Get-ChildItem $accountsDir -Directory -EA 0) {
        $emails = Get-AccountEmail -AcctDir $d.FullName
        if ($emails) {
            foreach ($e in $emails) {
                if ($e.ToLowerInvariant() -eq $lookup) { $targetDir = $d.FullName; break }
            }
        }
        if ($targetDir) { break }
    }
}
if (-not $targetDir) {
    $key = if ($AccountId) { "id '$AccountId'" } else { "email '$Email'" }
    Write-Host "No cached account matches $key. Run with -List to see what's actually cached." -ForegroundColor Yellow
    exit 1
}

# BACKUP: copy the whole account subdirectory to Desktop before removal.
$backupRoot = Join-Path $env:USERPROFILE ("Desktop\OneAuth-BACKUP-$(Get-Date -Format 'yyyyMMdd-HHmmss')")
New-Item -ItemType Directory -Path $backupRoot -Force | Out-Null
Copy-Item -Path $targetDir -Destination $backupRoot -Recurse -Force
Write-Host "Backup: $backupRoot" -ForegroundColor Green

# DELETE the target account cache.
Remove-Item -Path $targetDir -Recurse -Force
Write-Host "Removed: $targetDir" -ForegroundColor Green

# CLEAR MSAL token cache — otherwise apps may still hand out the last-known
# token for the deleted account until it naturally expires (hours to days).
if (-not $SkipMsalCacheClear) {
    $msal = Join-Path $oneAuthRoot '.msalcache.bin'
    if (Test-Path $msal) {
        Remove-Item $msal -Force
        Write-Host "Cleared MSAL cache: $msal" -ForegroundColor Green
    } else {
        Write-Host "MSAL cache already absent." -ForegroundColor DarkGray
    }
}

# VERIFY: list what's still cached so the user sees the desired end state.
Write-Host ""
Write-Host "Remaining cached accounts:" -ForegroundColor Cyan
$leftover = Get-ChildItem $accountsDir -Directory -EA 0
if (-not $leftover) {
    Write-Host "  (none — cache empty)" -ForegroundColor DarkGray
} else {
    foreach ($d in $leftover) {
        $emails = Get-AccountEmail -AcctDir $d.FullName
        $tag = if ($emails) { $emails -join ', ' } else { '(no email in cache — will refill on next sign-in)' }
        Write-Host "  $($d.Name)  ->  $tag"
    }
}

Write-Host ""
Write-Host "Restart Windows for every Microsoft app to pick up the change." -ForegroundColor Yellow
Write-Host "After restart, sign back into Microsoft Defender / Teams / Store with the account you actually want."
