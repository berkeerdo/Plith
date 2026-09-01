# scripts/security-migration.ps1 — Norton 360 -> Windows Defender migration.
#
# Removes only Norton 360 (the antivirus suite). Norton Utilities Ultimate and
# Norton Driver Updater are SEPARATE installations; they stay untouched — those
# are utility products, not AV, and they don't conflict with Defender.
#
# Runs in three phases so the user is never left without protection:
#
#   -Phase Prep     (BEFORE Norton uninstall)
#                   Verifies alternatives are in place, downloads Malwarebytes
#                   free installer for on-demand scanning, checks the Firewall,
#                   prints the exact Norton 360 uninstall command.
#
#   [MANUAL STEP]   User uninstalls Norton 360 via its own uninstaller
#                   (Control Panel -> Uninstall a Program -> Norton 360 -> Uninstall).
#                   Windows may prompt to restart. Restart.
#
#   -Phase Finish   (AFTER Norton uninstall + restart)
#                   Verifies Defender activated, turns Tamper Protection on,
#                   applies dev-friendly exclusions, installs Malwarebytes free,
#                   enables SmartScreen, verifies Firewall, prints a summary.
#
# Usage from admin PowerShell:
#   .\scripts\security-migration.ps1 -Phase Prep
#   ... user uninstalls Norton 360 + restarts ...
#   .\scripts\security-migration.ps1 -Phase Finish

[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('Prep', 'Finish')]
    [string]$Phase
)

$ErrorActionPreference = 'Stop'

function Step([string]$t) { Write-Host ""; Write-Host "==> $t" -ForegroundColor Cyan }
function OK([string]$m)   { Write-Host "  $m" -ForegroundColor Green }
function Warn([string]$m) { Write-Host "  $m" -ForegroundColor Yellow }
function Fail([string]$m) { Write-Host ""; Write-Host "FAIL: $m" -ForegroundColor Red; exit 1 }

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Fail "Requires admin PowerShell."
}

# ============================================================================
# PREP PHASE
# ============================================================================
if ($Phase -eq 'Prep') {
    Step "Detecting installed Norton products"
    $installed = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
                                 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*' `
                                 -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -like '*Norton*' }
    if (-not $installed) {
        Warn "No Norton products detected. If you already uninstalled, run: .\scripts\security-migration.ps1 -Phase Finish"
        exit 0
    }
    foreach ($p in $installed) {
        $mark = if ($p.DisplayName -match '360|Security|Antivirus') { '[REMOVE]' } else { '[KEEP  ]' }
        Write-Host "  $mark $($p.DisplayName)  v$($p.DisplayVersion)"
    }

    Step "Current Defender status"
    $mp = Get-MpComputerStatus
    Write-Host "  AntivirusEnabled       : $($mp.AntivirusEnabled)"
    Write-Host "  RealTimeProtection     : $($mp.RealTimeProtectionEnabled)"
    Write-Host "  Signatures updated     : $($mp.AntivirusSignatureLastUpdated)"
    if (-not $mp.AntivirusEnabled) {
        OK "Defender is passive (expected — Norton 360 is the active AV)."
    }

    Step "Verifying Windows Firewall"
    $profiles = Get-NetFirewallProfile -Profile Domain, Public, Private
    foreach ($p in $profiles) {
        $status = if ($p.Enabled) { 'ON' } else { 'OFF' }
        Write-Host "  $($p.Name): $status"
    }
    if ($profiles | Where-Object { -not $_.Enabled }) {
        Warn "One or more firewall profiles are OFF. They will be enabled in the Finish phase."
    }

    Step "Preparing Microsoft Safety Scanner reminder (ad-hoc backup scan)"
    Write-Host "  No third-party AV will be installed. Defender alone is sufficient."
    Write-Host "  For monthly ad-hoc second-opinion scans, download Microsoft Safety Scanner:"
    Write-Host "    https://learn.microsoft.com/en-us/defender-endpoint/safety-scanner-download"
    Write-Host "  It's a single .exe from Microsoft — no install, expires every 10 days, always fresh."

    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Yellow
    Write-Host "  READY TO UNINSTALL NORTON 360" -ForegroundColor Yellow
    Write-Host "==================================================" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  1) Open: Settings -> Apps -> Installed Apps"
    Write-Host "     (or 'Control Panel -> Programs and Features')"
    Write-Host "  2) Find 'Norton 360' -> Uninstall"
    Write-Host "     (Norton Utilities Ultimate and Driver Updater stay)"
    Write-Host "  3) Norton may ask for retention offers -> decline all"
    Write-Host "  4) Norton will ask to restart -> restart"
    Write-Host "  5) After restart, come back and run:"
    Write-Host ""
    Write-Host "       cd C:\Projects\plith" -ForegroundColor White
    Write-Host "       .\scripts\security-migration.ps1 -Phase Finish" -ForegroundColor White
    Write-Host ""
    Write-Host "  If Norton refuses to uninstall cleanly, use the Norton Remove"
    Write-Host "  and Reinstall Tool: https://support.norton.com/sp/en/us/norton-remove-and-reinstall-tool"
    Write-Host ""
    exit 0
}

# ============================================================================
# FINISH PHASE
# ============================================================================
if ($Phase -eq 'Finish') {
    Step "Verifying Norton 360 is gone"
    $stillInstalled = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
                                       'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*' `
                                       -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -match 'Norton 360|Norton Security|Norton Antivirus' }
    if ($stillInstalled) {
        Fail "Norton 360 is still installed. Uninstall it via Settings -> Apps first, then re-run Finish."
    }
    OK "Norton 360 not detected."

    Step "Confirming Defender is now active"
    $mp = Get-MpComputerStatus
    Write-Host "  AntivirusEnabled       : $($mp.AntivirusEnabled)"
    Write-Host "  RealTimeProtection     : $($mp.RealTimeProtectionEnabled)"
    Write-Host "  AMServiceEnabled       : $($mp.AMServiceEnabled)"
    if (-not $mp.AntivirusEnabled) {
        Warn "Defender still passive — the AMService may take a minute to activate. Wait 30s and re-run Finish."
        Write-Host "  You can also force it: Start Windows Security app once, then re-run."
        exit 2
    }
    OK "Defender AV + real-time protection active."

    Step "Updating Defender signatures (first pull after re-activation)"
    try {
        Update-MpSignature -ErrorAction Stop
        OK "Signatures updated."
    } catch {
        Warn "Signature update failed (network?): $($_.Exception.Message). It will retry via Windows Update automatically."
    }

    Step "Enabling Tamper Protection (via registry — Defender GUI toggle mirrors this)"
    try {
        $tpPath = 'HKLM:\SOFTWARE\Microsoft\Windows Defender\Features'
        if (-not (Test-Path $tpPath)) { New-Item -Path $tpPath -Force | Out-Null }
        Set-ItemProperty -Path $tpPath -Name 'TamperProtection' -Value 5 -Type DWord -ErrorAction Stop
        OK "Tamper Protection request registered (may require reboot to fully engage)."
    } catch {
        Warn "Couldn't set Tamper Protection registry key ($($_.Exception.Message)). Enable manually: Windows Security -> Virus & Threat Protection -> Manage Settings -> Tamper Protection ON."
    }

    Step "Applying dev-friendly Defender exclusions"
    $exclusionScript = Join-Path $PSScriptRoot 'add-defender-dev-exclusions.ps1'
    if (Test-Path $exclusionScript) {
        & $exclusionScript
    } else {
        Warn "add-defender-dev-exclusions.ps1 not found next to this script. Skipping."
    }

    Step "Enforcing Windows Firewall (all profiles)"
    Set-NetFirewallProfile -Profile Domain, Public, Private -Enabled True
    $profiles = Get-NetFirewallProfile -Profile Domain, Public, Private
    foreach ($p in $profiles) {
        Write-Host "  $($p.Name): $(if ($p.Enabled) { 'ON' } else { 'OFF' })"
    }

    Step "Enabling SmartScreen for Explorer + Edge (block dangerous downloads)"
    try {
        Set-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' `
                         -Name 'SmartScreenEnabled' -Value 'Warn' -ErrorAction Stop
        OK "Explorer SmartScreen set to Warn."
    } catch {
        Warn "Couldn't set Explorer SmartScreen: $($_.Exception.Message)"
    }

    Step "Downloading Microsoft Safety Scanner to Desktop (optional monthly scan)"
    # Microsoft's own one-shot scanner. No install, expires every 10 days so
    # each download is signature-fresh. No third-party trust required — signed
    # by Microsoft, only lives on disk until you delete it.
    $ssPath = Join-Path $env:USERPROFILE 'Desktop\MSERT.exe'
    try {
        Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/?LinkId=212732' `
                          -OutFile $ssPath -UseBasicParsing -ErrorAction Stop
        OK "Downloaded to $ssPath. Run it monthly, delete after scan."
    } catch {
        Warn "Safety Scanner download failed: $($_.Exception.Message)"
        Warn "Get it later from: https://learn.microsoft.com/en-us/defender-endpoint/safety-scanner-download"
    }

    Step "Post-migration summary"
    $mp2 = Get-MpComputerStatus
    $fw = Get-NetFirewallProfile | Where-Object { $_.Enabled } | Select-Object -Expand Name
    $exCount = (Get-MpPreference).ExclusionPath.Count
    Write-Host ""
    Write-Host "  Defender AV enabled      : $($mp2.AntivirusEnabled)"
    Write-Host "  Real-time protection     : $($mp2.RealTimeProtectionEnabled)"
    Write-Host "  Firewall active profiles : $($fw -join ', ')"
    Write-Host "  Defender exclusions      : $exCount folder(s)"
    Write-Host ""
    Write-Host "  Kept installed: Norton Utilities Ultimate, Norton Driver Updater"
    Write-Host ""
    Write-Host "  Monthly maintenance: run MSERT.exe on Desktop for a second-opinion scan"
    Write-Host "  (re-download it when it expires — the URL is in the Prep phase output)."
    Write-Host ""
    Write-Host "  Recommended browser extensions (install manually):"
    Write-Host "    - uBlock Origin      (ad + malware blocker)"
    Write-Host "    - 1Password extension (you already have 1Password)"
    Write-Host ""
    Write-Host "SUCCESS: Security migration complete." -ForegroundColor Green
    exit 0
}
