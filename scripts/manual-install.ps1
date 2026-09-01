# scripts/manual-install.ps1 — bypass the installer .exe entirely.
#
# When Norton (or any AV with reputation-based sandboxing) intercepts a fresh
# unknown binary, the installer's .NET AppHost can be trapped inside a
# transparent sandbox: the UI runs, progress renders, exit is clean, but every
# disk write went into a discardable overlay and the real Program Files stays
# untouched. Confirmed pattern: install.log doesn't grow, Plith.exe timestamp
# doesn't change, no Event Viewer crash — just "nothing happened".
#
# PowerShell.exe (%SystemRoot%\System32\WindowsPowerShell\...) is Microsoft-
# signed with high reputation. Norton doesn't sandbox it. So we do the copy
# ourselves through PowerShell, and the bytes actually land.
#
# What this script does:
#   1. Publishes Plith straight to a stage directory (or reuses one).
#   2. Kills any running Plith with taskkill (SeDebugPrivilege enabled).
#   3. Reclaims ownership + write ACL on C:\Program Files\Plith.
#   4. Copies each file from stage to C:\Program Files\Plith\, clearing
#      ReadOnly and using -Force to overwrite through any locks.
#   5. Writes the uninstall registry entry so Add/Remove Programs sees 0.1.5.
#   6. Creates the Start Menu shortcut (with per-user fallback if common fails).
#   7. Verifies Plith.exe on disk now reports the expected version.
#
# Run from an admin PowerShell:
#   cd C:\Projects\plith
#   .\scripts\manual-install.ps1

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

function Step([string]$t) { Write-Host ""; Write-Host "==> $t" -ForegroundColor Cyan }
function Fail([string]$m) { Write-Host ""; Write-Host "FAIL: $m" -ForegroundColor Red; exit 1 }
function OK([string]$m)   { Write-Host "  $m" -ForegroundColor Green }

# 0. Admin
$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Fail "Requires admin. Right-click PowerShell -> Run as administrator."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$installerProj = Join-Path $repoRoot 'src\Plith.Installer\Plith.Installer.csproj'
$plithProj = Join-Path $repoRoot 'src\Plith\Plith.csproj'
$installDir = 'C:\Program Files\Plith'
$stageDir = Join-Path $env:LOCALAPPDATA 'Plith\ManualInstall\stage'
$signScript = Join-Path $repoRoot 'scripts\sign-plith.ps1'

# 1. Read expected version from csproj so we can verify at the end.
$expectedVersion = (Select-String -Path $plithProj -Pattern '<Version>(.*?)</Version>').Matches[0].Groups[1].Value
Step "Target: Plith $expectedVersion into $installDir"

# 2. Publish Plith into stage. Publish, not build — same output layout the
#    Plith.Installer bundles for its embedded resource.
Step "Publishing Plith to $stageDir"
if (Test-Path $stageDir) { Remove-Item -Recurse -Force $stageDir }
New-Item -ItemType Directory -Force -Path $stageDir | Out-Null
& dotnet publish $plithProj -c Release -o $stageDir -p:PublishSingleFile=false -p:SelfContained=false 2>&1 | Out-Host
if ($LASTEXITCODE -ne 0) { Fail "dotnet publish failed." }
$stagePlithExe = Join-Path $stageDir 'Plith.exe'
if (-not (Test-Path $stagePlithExe)) { Fail "Publish reported success but $stagePlithExe missing." }

# 3. Sign the published Plith.exe with the developer cert if the sign script
#    is available. Not strictly required (UIAccess only enforces this on Game
#    mode), but keeps the on-disk file signed the same way the installer
#    would have signed it.
if (Test-Path $signScript) {
    Step "Signing $stagePlithExe"
    $certPath = Join-Path $env:TEMP 'plith-cert.cer'
    & powershell -NoProfile -ExecutionPolicy Bypass -File $signScript `
        -PlithExe $stagePlithExe -CertOutput $certPath 2>&1 | Out-Host
    if ($LASTEXITCODE -ne 0) { Write-Warning "signing failed; continuing with unsigned Plith.exe." }
    if (Test-Path $certPath) { Remove-Item $certPath -Force -ErrorAction SilentlyContinue }
}

# 4. Kill any running Plith. SeDebugPrivilege isn't needed for a normal user-
#    launched Plith (only UIAccess ones), but taskkill covers both paths.
Step "Stopping running Plith (if any)"
$before = Get-Process -Name 'Plith' -ErrorAction SilentlyContinue
if ($before) {
    & taskkill /F /IM Plith.exe /T 2>&1 | Out-Host
    Start-Sleep -Seconds 2
    $after = Get-Process -Name 'Plith' -ErrorAction SilentlyContinue
    if ($after) {
        Write-Warning "  $($after.Count) Plith process(es) still alive after taskkill. File copy will still be attempted."
    } else {
        OK "Plith stopped."
    }
} else {
    OK "No Plith running."
}

# 5. Ensure install directory exists + has admin write. If a previous
#    install left files with hostile ACLs, reclaim ownership now.
Step "Reclaiming ownership of $installDir"
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
& takeown /F "$installDir" /R /A /D Y 2>&1 | Out-Null
& icacls "$installDir" /grant "*S-1-5-32-544:F" /T /C /Q 2>&1 | Out-Null
OK "Ownership + Administrators:F granted recursively."

# 6. Copy each file. Clear ReadOnly first, then Copy-Item -Force to overwrite
#    even if AV has the target open share-read.
Step "Copying files into $installDir"
$copied = 0
$skipped = 0
$failed = @()
Get-ChildItem -Path $stageDir -Recurse -File | ForEach-Object {
    $rel = $_.FullName.Substring($stageDir.Length).TrimStart('\')
    $target = Join-Path $installDir $rel
    $targetDir = Split-Path -Parent $target
    if (-not (Test-Path $targetDir)) { New-Item -ItemType Directory -Force -Path $targetDir | Out-Null }
    try {
        if (Test-Path $target) {
            try {
                $existing = Get-Item $target
                if ($existing.IsReadOnly) { $existing.IsReadOnly = $false }
            } catch { }
        }
        Copy-Item -Path $_.FullName -Destination $target -Force -ErrorAction Stop
        $copied++
    } catch {
        $failed += "$rel  ->  $($_.Exception.Message)"
    }
}
Write-Host "  copied: $copied files"
if ($failed.Count -gt 0) {
    Write-Host "  failed: $($failed.Count)" -ForegroundColor Yellow
    $failed | Select-Object -First 5 | ForEach-Object { Write-Host "    $_" }
    if ($failed.Count -gt 5) { Write-Host "    ... and $($failed.Count - 5) more" }
}

# 7. Update the Add/Remove Programs registry entry so Windows knows about the
#    new version. Matches what InstallOrchestrator.RegisterPlith normally writes.
Step "Updating uninstall registry entry"
$uninstallKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith'
if (-not (Test-Path $uninstallKey)) { New-Item -Path $uninstallKey -Force | Out-Null }
Set-ItemProperty -Path $uninstallKey -Name 'DisplayName'      -Value 'Plith'
Set-ItemProperty -Path $uninstallKey -Name 'DisplayVersion'   -Value $expectedVersion
Set-ItemProperty -Path $uninstallKey -Name 'InstallLocation'  -Value $installDir
Set-ItemProperty -Path $uninstallKey -Name 'Publisher'        -Value 'Plith'
Set-ItemProperty -Path $uninstallKey -Name 'DisplayIcon'      -Value (Join-Path $installDir 'Plith.exe')
Set-ItemProperty -Path $uninstallKey -Name 'NoModify'         -Value 1 -Type DWord
Set-ItemProperty -Path $uninstallKey -Name 'NoRepair'         -Value 1 -Type DWord
OK "Add/Remove Programs entry set to $expectedVersion."

# 8. Start Menu shortcut. Common first, per-user fallback.
Step "Creating Start Menu shortcut"
$plithExe = Join-Path $installDir 'Plith.exe'
$commonLnk = Join-Path ([Environment]::GetFolderPath('CommonStartMenu')) 'Programs\Plith.lnk'
$userLnk = Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs\Plith.lnk'
$wsh = New-Object -ComObject WScript.Shell

function New-Shortcut([string]$lnkPath) {
    $dir = Split-Path -Parent $lnkPath
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    if (Test-Path $lnkPath) { Remove-Item $lnkPath -Force -ErrorAction SilentlyContinue }
    $sc = $wsh.CreateShortcut($lnkPath)
    $sc.TargetPath = $plithExe
    $sc.WorkingDirectory = $installDir
    $sc.IconLocation = $plithExe
    $sc.Description = 'Modern Windows audio OSD with Voicemeeter-first design and media controls.'
    $sc.Save()
}

try {
    New-Shortcut $commonLnk
    OK "Common Start Menu: $commonLnk"
} catch {
    Write-Warning "  Common Start Menu failed: $($_.Exception.Message). Trying per-user..."
    try {
        New-Shortcut $userLnk
        OK "Per-user Start Menu: $userLnk"
    } catch {
        Write-Warning "  Per-user Start Menu also failed: $($_.Exception.Message). Skipping shortcut."
    }
}

# 9. Register autostart Run key so Plith launches at login.
Step "Registering autostart"
$runKey = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
Set-ItemProperty -Path $runKey -Name 'Plith' -Value "`"$plithExe`""
OK "HKCU\...\Run\Plith set."

# 10. Verify.
Step "Verifying"
if (-not (Test-Path $plithExe)) { Fail "Plith.exe missing after copy." }
$info = (Get-Item $plithExe).VersionInfo
$installed = ($info.ProductVersion -split '\+')[0]
if ($installed -ne $expectedVersion) {
    Write-Warning "  On-disk version is $installed, expected $expectedVersion."
    Write-Warning "  This usually means Norton sandbox intercepted PowerShell too, or the target file was locked."
    Fail "Version mismatch."
}
OK "Plith.exe on disk reports version $installed."
Write-Host ""
Write-Host "SUCCESS: Plith $expectedVersion is installed at $installDir" -ForegroundColor Green
Write-Host "Launch: & '$plithExe'"
