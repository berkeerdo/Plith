# scripts/find-msaccount-everywhere.ps1 - locate every place on this PC that
# references a specific Microsoft account email. Used to figure out why the
# account keeps showing up in Windows Settings even after the standard
# clean-orphan and purge scripts.
#
# Read-only. Makes no changes.
#
# Usage:
#   .\scripts\find-msaccount-everywhere.ps1 -Email 'wrongaccount@hotmail.com'

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Email
)

$ErrorActionPreference = 'Continue'
$found = @()

function Report([string]$location, [string]$detail) {
    $script:found += [PSCustomObject]@{ Location = $location; Detail = $detail }
    Write-Host "  HIT: $location" -ForegroundColor Yellow
    if ($detail) { Write-Host "       $detail" -ForegroundColor DarkYellow }
}

# 1. OneAuth accounts subdir
Write-Host ""
Write-Host "=== OneAuth accounts ===" -ForegroundColor Cyan
$path = "$env:LOCALAPPDATA\Microsoft\OneAuth\accounts"
if (Test-Path $path) {
    Get-ChildItem $path -Directory -EA 0 | ForEach-Object {
        $inner = Join-Path $_.FullName $_.Name
        if (Test-Path $inner) {
            try {
                $bytes = [IO.File]::ReadAllBytes($inner)
                $text = [Text.Encoding]::Unicode.GetString($bytes)
                if ($text -match [regex]::Escape($Email)) {
                    Report $inner "OneAuth account cache"
                }
            } catch {}
        }
    }
}

# 2. WAM broker (Windows Web Account Manager)
Write-Host ""
Write-Host "=== WAM broker (TokenBroker\Accounts) ===" -ForegroundColor Cyan
$wam = "$env:LOCALAPPDATA\Packages\Microsoft.AAD.BrokerPlugin_cw5n1h2txyewy\AC\TokenBroker\Accounts"
if (Test-Path $wam) {
    Get-ChildItem $wam -File -EA 0 | ForEach-Object {
        try {
            $bytes = [IO.File]::ReadAllBytes($_.FullName)
            $text = [Text.Encoding]::Unicode.GetString($bytes)
            if ($text -match [regex]::Escape($Email)) {
                Report $_.FullName "WAM broker account file"
            }
        } catch {}
    }
}

# 3. IdentityCRL StoredIdentities
Write-Host ""
Write-Host "=== IdentityCRL StoredIdentities ===" -ForegroundColor Cyan
Get-ChildItem 'HKCU:\Software\Microsoft\IdentityCRL\StoredIdentities' -EA 0 | ForEach-Object {
    Report $_.Name "StoredIdentities subkey"
}

# 4. IdentityCRL UserExtendedProperties
Write-Host ""
Write-Host "=== IdentityCRL UserExtendedProperties ===" -ForegroundColor Cyan
Get-ChildItem 'HKCU:\Software\Microsoft\IdentityCRL\UserExtendedProperties' -EA 0 | ForEach-Object {
    $name = $_.PSChildName
    if ($name -match [regex]::Escape($Email)) {
        Report $_.Name "UserExtendedProperties subkey"
    }
}

# 5. IdentityStore Cache (HKLM)
Write-Host ""
Write-Host "=== HKLM IdentityStore Cache ===" -ForegroundColor Cyan
Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\IdentityStore\Cache' -Recurse -EA 0 | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath -EA 0
    if ($p) {
        $blob = "$($p.IdentityName)|$($p.UserName)|$($p.DisplayName)|$($p.SamAccountName)"
        if ($blob -match [regex]::Escape($Email)) {
            Report $_.Name "IdentityStore\Cache subkey"
        }
    }
}

# 6. Owned Identities (this is the primary-account list Windows Settings reads)
Write-Host ""
Write-Host "=== HKLM IdentityStore OwnedIdentities ===" -ForegroundColor Cyan
$owned = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Authentication\IdentityStore\OwnedIdentities'
if (Test-Path $owned) {
    Get-ChildItem $owned -Recurse -EA 0 | ForEach-Object {
        $p = Get-ItemProperty $_.PSPath -EA 0
        if ($p) {
            $blob = "$($p | Out-String)"
            if ($blob -match [regex]::Escape($Email)) {
                Report $_.Name "OwnedIdentities subkey"
            }
        }
    }
}

# 7. LiveIdSvc SharedAccount (this is where Windows tracks the primary MS acct)
Write-Host ""
Write-Host "=== LiveIdSvc SharedAccount ===" -ForegroundColor Cyan
$live = 'HKCU:\Software\Microsoft\LiveId'
if (Test-Path $live) {
    Get-ChildItem $live -Recurse -EA 0 | ForEach-Object {
        $p = Get-ItemProperty $_.PSPath -EA 0
        if ($p) {
            $blob = "$($p | Out-String)"
            if ($blob -match [regex]::Escape($Email)) {
                Report $_.Name "LiveId subkey"
            }
        }
    }
}

# 8. Credential Manager Vault entries
Write-Host ""
Write-Host "=== Windows Credential Manager ===" -ForegroundColor Cyan
try {
    $creds = & cmdkey /list 2>&1 | Out-String
    if ($creds -match [regex]::Escape($Email)) {
        Report "cmdkey /list" "Contains '$Email' — inspect with 'cmdkey /list' manually"
    }
} catch {}

# 9. Local AppData search (targeted files, not full recursive)
Write-Host ""
Write-Host "=== AppData targeted config files ===" -ForegroundColor Cyan
$targets = @(
    "$env:LOCALAPPDATA\Packages\Microsoft.WindowsStore_8wekyb3d8bbwe\LocalCache",
    "$env:LOCALAPPDATA\Packages\Microsoft.XboxGamingOverlay_8wekyb3d8bbwe\LocalCache",
    "$env:LOCALAPPDATA\Packages\Microsoft.XboxApp_8wekyb3d8bbwe\LocalCache",
    "$env:LOCALAPPDATA\Packages\Microsoft.GamingApp_8wekyb3d8bbwe\LocalCache",
    "$env:LOCALAPPDATA\Packages\microsoft.windowscommunicationsapps_8wekyb3d8bbwe",
    "$env:APPDATA\Microsoft\Windows\Recent",
    "$env:LOCALAPPDATA\Microsoft\Windows\INetCookies"
)
foreach ($t in $targets) {
    if (Test-Path $t) {
        Get-ChildItem $t -Recurse -File -EA 0 | Select-Object -First 500 | ForEach-Object {
            try {
                $raw = Get-Content $_.FullName -Raw -EA 0 -TotalCount 200
                if ($raw -and ($raw -match [regex]::Escape($Email))) {
                    Report $_.FullName "config file references email"
                }
            } catch {}
        }
    }
}

# 10. Registry Run entries / auto-launch
Write-Host ""
Write-Host "=== Autostart entries mentioning email ===" -ForegroundColor Cyan
'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run',
'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' | ForEach-Object {
    if (Test-Path $_) {
        $k = Get-Item $_
        $k.GetValueNames() | ForEach-Object {
            $val = $k.GetValue($_)
            if ("$val" -match [regex]::Escape($Email)) {
                Report "$($_.PSPath) : $_" "$val"
            }
        }
    }
}

Write-Host ""
Write-Host "===================================" -ForegroundColor Green
Write-Host "  SUMMARY" -ForegroundColor Green
Write-Host "===================================" -ForegroundColor Green
if ($found.Count -eq 0) {
    Write-Host "  Zero locations reference '$Email'." -ForegroundColor Green
    Write-Host "  If it still shows in Settings, restart Windows and re-run this script."
} else {
    Write-Host "  Found $($found.Count) location(s) referencing '$Email':"
    $found | ForEach-Object { Write-Host "    - $($_.Location)" }
    Write-Host ""
    Write-Host "  Next step: send the output above to Claude for surgical removal."
}
