# scripts/install-osd.ps1 - Installs the signed plith-osd.exe under Program Files.
# Without -Yes it only prints what it would do. With -Yes, run it under sudo.
# -Uninstall -Yes removes only the installed exe and the empty osd folder.

[CmdletBinding()]
param(
    [switch]$Yes,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$source = Join-Path $repoRoot 'target\release\plith-osd.exe'
$destDir = 'C:\Program Files\Plith\osd'
$dest = 'C:\Program Files\Plith\osd\plith-osd.exe'

function Stop-InstalledOsd {
    # Only the process whose image is exactly the installed path.
    Get-Process -Name 'plith-osd' -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $dest } |
        ForEach-Object {
            Write-Host "Stopping plith-osd (pid $($_.Id))"
            Stop-Process -Id $_.Id -Force
            $_.WaitForExit(5000) | Out-Null
        }
}

if ($Uninstall) {
    if (-not $Yes) {
        Write-Host "Dry run. Would stop plith-osd running from $dest, delete $dest, and remove $destDir if empty."
        exit 0
    }
    Stop-InstalledOsd
    if (Test-Path -LiteralPath $dest) { Remove-Item -LiteralPath $dest -Force }
    if ((Test-Path -LiteralPath $destDir) -and -not (Get-ChildItem -LiteralPath $destDir -Force)) {
        Remove-Item -LiteralPath $destDir -Force
    }
    Write-Host 'Uninstalled.'
    exit 0
}

if (-not (Test-Path -LiteralPath $source)) { throw "Source not found: $source. Run scripts\build-osd.ps1 first." }

if (-not $Yes) {
    Write-Host 'Dry run. Would:'
    Write-Host "  stop plith-osd if running from $dest"
    Write-Host "  copy $source"
    Write-Host "    to $dest"
    Write-Host '  verify the Authenticode signature at the destination'
    Write-Host 'Pass -Yes (under sudo) to do it.'
    exit 0
}

$srcSig = Get-AuthenticodeSignature -LiteralPath $source
if ($srcSig.Status -ne 'Valid') { throw "Source signature is $($srcSig.Status), expected Valid." }

Stop-InstalledOsd
New-Item -ItemType Directory -Path $destDir -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $dest -Force

$sig = Get-AuthenticodeSignature -LiteralPath $dest
if ($sig.Status -ne 'Valid') { throw "Installed signature is $($sig.Status), expected Valid." }
Write-Host "Installed $dest (signature Valid, $($sig.SignerCertificate.Thumbprint))"
