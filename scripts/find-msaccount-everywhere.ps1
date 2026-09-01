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

# 9. MSA Broker Plugin (separate from AAD broker — this is the personal
#    @hotmail.com account broker specifically, likely holdout source)
Write-Host ""
Write-Host "=== MSA Broker Plugin (Microsoft.MicrosoftAccountBrokerPlugin) ===" -ForegroundColor Cyan
$msaBroker = "$env:LOCALAPPDATA\Packages\Microsoft.MicrosoftAccountBrokerPlugin_cw5n1h2txyewy"
if (Test-Path $msaBroker) {
    Get-ChildItem $msaBroker -Recurse -File -EA 0 | ForEach-Object {
        try {
            $b = [IO.File]::ReadAllBytes($_.FullName)
            $t1 = [Text.Encoding]::Unicode.GetString($b)
            $t2 = [Text.Encoding]::UTF8.GetString($b)
            if (($t1 -match [regex]::Escape($Email)) -or ($t2 -match [regex]::Escape($Email))) {
                Report $_.FullName "MSA broker file references email"
            }
        } catch {}
    }
} else {
    Skipped "MSA broker package folder absent"
}

# 10. WinRT PasswordVault (separate from Credential Manager — different API,
#     survives cmdkey /delete because it's not in the same store)
Write-Host ""
Write-Host "=== WinRT PasswordVault ===" -ForegroundColor Cyan
try {
    Add-Type -AssemblyName Windows.Security -EA SilentlyContinue
    [Windows.Security.Credentials.PasswordVault, Windows.Security.Credentials, ContentType=WindowsRuntime] > $null
    $vault = New-Object Windows.Security.Credentials.PasswordVault
    $all = $vault.RetrieveAll()
    foreach ($c in $all) {
        if (($c.UserName -match [regex]::Escape($Email)) -or ($c.Resource -match [regex]::Escape($Email))) {
            Report "PasswordVault" "resource=$($c.Resource) user=$($c.UserName)"
        }
    }
} catch {
    Skipped "PasswordVault API unavailable: $($_.Exception.Message)"
}

# 11. PeopleExperienceHost (People app + Start Menu account tile source)
Write-Host ""
Write-Host "=== PeopleExperienceHost ===" -ForegroundColor Cyan
$peh = "$env:LOCALAPPDATA\Packages\Microsoft.Windows.PeopleExperienceHost_cw5n1h2txyewy"
if (Test-Path $peh) {
    Get-ChildItem $peh -Recurse -File -EA 0 | Select-Object -First 200 | ForEach-Object {
        try {
            $raw = Get-Content $_.FullName -Raw -EA 0 -TotalCount 200
            if ($raw -and ($raw -match [regex]::Escape($Email))) {
                Report $_.FullName "PeopleExperienceHost file references email"
            }
        } catch {}
    }
} else {
    Skipped "PeopleExperienceHost package absent"
}

function Skipped([string]$m) { Write-Host "  ..$m" -ForegroundColor DarkGray }

# 12. IdentityCRL Immersive (deep MSA token cache)
Write-Host ""
Write-Host "=== IdentityCRL Immersive tokens ===" -ForegroundColor Cyan
$immersive = 'HKCU:\Software\Microsoft\IdentityCRL\Immersive'
if (Test-Path $immersive) {
    Get-ChildItem $immersive -Recurse -EA 0 | ForEach-Object {
        if ($_.Name -match [regex]::Escape($Email)) {
            Report $_.Name "IdentityCRL Immersive subkey path"
        }
        $p = Get-ItemProperty $_.PSPath -EA 0
        if ($p) {
            $blob = "$($p | Out-String)"
            if ($blob -match [regex]::Escape($Email)) {
                Report $_.Name "IdentityCRL Immersive subkey value"
            }
        }
    }
}

# 13. Local AppData search (targeted files, not full recursive)
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
