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
$signtool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $signtool) {
    $candidates = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
        -Filter 'signtool.exe' -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match 'x64' } |
        Sort-Object FullName -Descending
    if ($candidates) { $signtool = $candidates[0] }
}
if (-not $signtool) {
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
& $signtool.Source sign /sha1 $thumb /fd SHA256 `
    /tr 'http://timestamp.digicert.com' /td SHA256 $exePath | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed." }

# 6. Mirror to Program Files. /MIR removes stale files from a prior install.
Write-Host "Installing to $installDir..."
if (-not (Test-Path $installDir)) { New-Item -ItemType Directory -Path $installDir | Out-Null }
& robocopy $publishDir $installDir /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
# robocopy uses non-zero success codes; 0-7 are non-error.
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with code $LASTEXITCODE." }

# 7. Launch. Plith's AutoStartService.Apply rewrites the HKCU\Run entry to the
#    new path on every startup, so no manual registry edit is needed here.
$installedExe = Join-Path $installDir 'Plith.exe'
Write-Host "Launching $installedExe..."
Start-Process -FilePath $installedExe

Write-Host ""
Write-Host "Done. Open Settings and check the Game mode badge -- it should now read 'Active'."
