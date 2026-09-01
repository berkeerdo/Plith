# scripts/add-defender-dev-exclusions.ps1 — dev-friendly Defender setup with
# defense-in-depth guardrails. Safer than the naive "exclude everything dev"
# approach.
#
# What we exclude — folders (medium-risk items included per user preference for
# lower scan overhead; user accepts the trade-off, ASR rules below compensate):
#   - C:\Projects                — your own repos; only trusted sources cloned here
#   - %USERPROFILE%\.dotnet      — SDK install cache (Microsoft-signed content)
#   - %USERPROFILE%\.nuget       — NuGet package cache (signed feed)
#   - %USERPROFILE%\.npm         — npm cache (supply chain risk — real 2018-2024
#                                  compromises; ASR rules + eventual npm audit
#                                  are the mitigation)
#   - %LOCALAPPDATA%\Temp\.net   — .NET single-file self-extract stage
#   - %LOCALAPPDATA%\NuGet       — NuGet's second cache location
#   - %LOCALAPPDATA%\Programs\Microsoft VS Code   IDE + extensions (medium risk:
#                                                 malicious extension attack path)
#   - %LOCALAPPDATA%\JetBrains                    Same
#
# What we exclude by PROCESS (compile-only, no living-off-the-land value):
#   - dotnet.exe / MSBuild.exe / VBCSCompiler.exe   .NET build pipeline
#   - node.exe                                       Node runtime (bundlers, tests)
#
# What we DELIBERATELY DO NOT exclude even at user request:
#   - powershell.exe, pwsh.exe   PowerShell is the #1 attacker toolkit — even
#                                after ASR, excluding this process disables
#                                Defender's behavioral engine on the vector
#                                attackers use most. Non-negotiable.
#   - Code.exe, devenv.exe       IDE extensions self-update through here.
#                                Excluding the PROCESS is worse than excluding
#                                the folder: everything IDE reads/writes goes
#                                unscanned. Folder exclusion above is the
#                                more surgical trade the user opted into.
#
# We also enable a set of Defender Attack Surface Reduction (ASR) rules that
# compensate for the exclusions above by blocking common malware behaviors
# (Office macro drop, credential theft via LSASS, ransomware-style bulk writes).
#
# Undo: Remove-MpPreference -ExclusionPath '<path>'  /  -ExclusionProcess '<name>'
# List: Get-MpPreference | Select -Expand ExclusionPath  /  ExclusionProcess

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "This script requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$avStatus = Get-MpComputerStatus -ErrorAction SilentlyContinue
if (-not $avStatus) {
    throw "Windows Defender is not available on this system. Uninstall your third-party AV first."
}
if (-not $avStatus.AntivirusEnabled) {
    Write-Warning "Windows Defender is registered but not the active antivirus (probably a third-party AV is running in the foreground). Exclusions will only apply once Defender takes over. Uninstall the other AV and re-run."
}

# ---------------------------------------------------------------------------
# Folder exclusions — narrow, cache-only where possible.
# ---------------------------------------------------------------------------
$folders = @(
    'C:\Projects',
    "$env:USERPROFILE\.dotnet",
    "$env:USERPROFILE\.nuget",
    "$env:USERPROFILE\.npm",
    "$env:LOCALAPPDATA\Temp\.net",
    "$env:LOCALAPPDATA\NuGet",
    "$env:LOCALAPPDATA\Programs\Microsoft VS Code",
    "$env:LOCALAPPDATA\JetBrains"
)

# ---------------------------------------------------------------------------
# Process exclusions — pure build tools only. NO shells, NO IDEs.
# ---------------------------------------------------------------------------
$processes = @(
    'dotnet.exe',
    'MSBuild.exe',
    'VBCSCompiler.exe',
    'node.exe'
)

# ---------------------------------------------------------------------------
# ASR rules for defense-in-depth. Values from Microsoft docs
# https://learn.microsoft.com/microsoft-365/security/defender-endpoint/attack-surface-reduction-rules-reference
# Mode 1 = Block. Mode 2 = Audit (log only, useful before flipping to Block).
# ---------------------------------------------------------------------------
$asrRules = @(
    # Block Office apps from creating executable content (macro drop)
    '3B576869-A4EC-4529-8536-B80A7769E899',
    # Block credential stealing from Windows local security authority (LSASS)
    '9E6C4E1F-7D60-472F-BA1A-A39EF669E4B2',
    # Block ransomware behavior (bulk-write patterns)
    'C1DB55AB-C21A-4637-BB3F-A12568109D35',
    # Block Win32 API calls from Office macros
    '92E97FA1-2EDF-4476-BDD6-9DD0B4DDDC7B',
    # Block untrusted / unsigned processes running from USB drives
    'B2B3F03D-6A65-4F7B-A9C7-1C7EF74A9BA4'
)

Write-Host ""
Write-Host "==> Adding $($folders.Count) folder exclusions" -ForegroundColor Cyan
foreach ($f in $folders) {
    if (Test-Path $f) {
        try {
            Add-MpPreference -ExclusionPath $f -ErrorAction Stop
            Write-Host "  OK  $f"
        } catch {
            Write-Warning "  skipped $f -> $($_.Exception.Message)"
        }
    } else {
        Write-Host "  ..  $f  (doesn't exist yet — skipped)"
    }
}

Write-Host ""
Write-Host "==> Adding $($processes.Count) process exclusions (compile tools only, no shells / IDEs)" -ForegroundColor Cyan
foreach ($p in $processes) {
    try {
        Add-MpPreference -ExclusionProcess $p -ErrorAction Stop
        Write-Host "  OK  $p"
    } catch {
        Write-Warning "  skipped $p -> $($_.Exception.Message)"
    }
}

Write-Host ""
Write-Host "==> Enabling Attack Surface Reduction rules (defense-in-depth)" -ForegroundColor Cyan
foreach ($rule in $asrRules) {
    try {
        Add-MpPreference -AttackSurfaceReductionRules_Ids $rule -AttackSurfaceReductionRules_Actions Enabled -ErrorAction Stop
        Write-Host "  OK  $rule (Block)"
    } catch {
        Write-Warning "  skipped $rule -> $($_.Exception.Message)"
    }
}

Write-Host ""
Write-Host "Done. Verify with:" -ForegroundColor Green
Write-Host "  Get-MpPreference | Select-Object -Expand ExclusionPath"
Write-Host "  Get-MpPreference | Select-Object -Expand ExclusionProcess"
Write-Host "  Get-MpPreference | Select-Object -Expand AttackSurfaceReductionRules_Ids"
Write-Host ""
Write-Host "If an ASR rule ever fires a false positive on your build, downgrade it:"
Write-Host "  Set-MpPreference -AttackSurfaceReductionRules_Ids <ID> -AttackSurfaceReductionRules_Actions AuditMode"
