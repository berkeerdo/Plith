# scripts/install-local.ps1 — build, sign, and install Plith to %ProgramFiles%\Plith\
# so it earns the UIAccess privilege from app.manifest. Requires admin.

[CmdletBinding()]
param(
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "install-local.ps1 requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$projPath = Join-Path $repoRoot 'src\Plith\Plith.csproj'
$publishDir = Join-Path $repoRoot 'publish'
$installDir = Join-Path $env:ProgramFiles 'Plith'

# 1. Resolve signtool. Windows SDK or VS Build Tools must be installed.
$signtoolPath = $null
$cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
if ($cmd) { $signtoolPath = $cmd.Source }
if (-not $signtoolPath) {
    $candidates = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
        -Filter 'signtool.exe' -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match 'x64' } |
        Sort-Object FullName -Descending
    if ($candidates) { $signtoolPath = $candidates[0].FullName }
}
if (-not $signtoolPath) {
    throw "signtool.exe not found. Install the Windows 10/11 SDK or VS Build Tools (workload: 'Desktop development with C++') and re-run."
}

# 2. Stop Plith if running so we can overwrite files.
Get-Process -Name Plith -ErrorAction SilentlyContinue | Stop-Process -Force

# 3. Set up cert (idempotent); capture thumbprint.
$thumb = & (Join-Path $PSScriptRoot 'setup-cert.ps1') | Select-Object -Last 1
if (-not $thumb) { throw "setup-cert.ps1 did not return a thumbprint." }

# 4. Build a multi-file Release publish. PublishSingleFile=false because UIAccess
#    binaries occasionally trip up appcompat's manifest parser when the manifest
#    is embedded in a single-file bundle.
Write-Host "Publishing $Configuration build..."
& dotnet publish $projPath -c $Configuration -o $publishDir `
    -p:PublishSingleFile=false -p:SelfContained=false | Out-Host
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed." }

# 5. Sign the main executable. WPF .NET app only needs the exe signed; the DLLs
#    don't need signatures for UIAccess to be honored (only the manifest-bearing
#    assembly does).
$exePath = Join-Path $publishDir 'Plith.exe'
Write-Host "Signing $exePath..."
& $signtoolPath sign /sha1 $thumb /fd SHA256 `
    /tr 'http://timestamp.digicert.com' /td SHA256 $exePath | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed." }

# 6. Mirror to Program Files. /MIR removes stale files from a prior install.
Write-Host "Installing to $installDir..."
if (-not (Test-Path $installDir)) { New-Item -ItemType Directory -Path $installDir | Out-Null }
& robocopy $publishDir $installDir /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
# robocopy uses non-zero success codes; 0-7 are non-error.
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with code $LASTEXITCODE." }

$installedExe = Join-Path $installDir 'Plith.exe'

# 7. Create Start menu shortcut so Plith appears in Windows search + Recent Apps.
$startMenu = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs'
$shortcutPath = Join-Path $startMenu 'Plith.lnk'
Write-Host "Creating Start menu shortcut..."
$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $installedExe
$shortcut.WorkingDirectory = $installDir
$shortcut.IconLocation = $installedExe
$shortcut.Description = 'Modern Windows audio OSD with Voicemeeter-first design and media controls.'
$shortcut.Save()

# 8. Register in Add/Remove Programs (Settings -> Apps -> Installed apps).
#    UninstallString points at the uninstall script through pwsh -ExecutionPolicy Bypass
#    so the Windows uninstall button just works without ceremony.
$uninstallKey = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Plith'
$uninstallScript = Join-Path $PSScriptRoot 'uninstall-local.ps1'
Write-Host "Registering Add/Remove Programs entry..."
if (-not (Test-Path $uninstallKey)) { New-Item -Path $uninstallKey -Force | Out-Null }
$version = (Get-Item $installedExe).VersionInfo.ProductVersion
Set-ItemProperty -Path $uninstallKey -Name 'DisplayName'     -Value 'Plith'
Set-ItemProperty -Path $uninstallKey -Name 'DisplayVersion'  -Value ($version ?? '0.1.0')
Set-ItemProperty -Path $uninstallKey -Name 'Publisher'       -Value 'Plith Self-Signed'
Set-ItemProperty -Path $uninstallKey -Name 'InstallLocation' -Value $installDir
Set-ItemProperty -Path $uninstallKey -Name 'DisplayIcon'     -Value $installedExe
Set-ItemProperty -Path $uninstallKey -Name 'UninstallString' `
    -Value "pwsh.exe -NoProfile -ExecutionPolicy Bypass -File `"$uninstallScript`""
Set-ItemProperty -Path $uninstallKey -Name 'NoModify'        -Value 1 -Type DWord
Set-ItemProperty -Path $uninstallKey -Name 'NoRepair'        -Value 1 -Type DWord
$estimatedKb = [int]((Get-ChildItem $installDir -Recurse | Measure-Object Length -Sum).Sum / 1KB)
Set-ItemProperty -Path $uninstallKey -Name 'EstimatedSize'   -Value $estimatedKb -Type DWord

# 9. Launch via explorer.exe so the new process runs in the user context (not admin),
#    which is what UIAccess binaries need to register their privilege correctly.
#    Direct Start-Process from elevated PowerShell fails with "A referral was returned
#    from the server" because UIAccess + admin parent = invalid token combo.
#    Plith's AutoStartService.Apply rewrites the HKCU\Run entry to the new path on
#    every startup, so no manual registry edit is needed here.
Write-Host "Launching $installedExe..."
Start-Process -FilePath 'explorer.exe' -ArgumentList "`"$installedExe`""

Write-Host ""
Write-Host "Done. Plith is in Start menu (search 'Plith') and Add/Remove Programs."
Write-Host "Open Settings and check the Game mode badge -- it should now read 'Active'."
