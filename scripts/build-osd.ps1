# scripts/build-osd.ps1: builds plith-osd.exe with uiAccess='true' and signs it with the
# developer's self-signed certificate. Never creates or imports a certificate.

[CmdletBinding()]
param(
    [string]$CertSubject = 'CN=Plith Self-Signed'
)

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$exe = Join-Path $repoRoot 'target\release\plith-osd.exe'

# 1. Find the signing certificate first, so a missing cert fails before a long build.
$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $CertSubject -and $_.NotAfter -gt (Get-Date) -and $_.HasPrivateKey } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1
if (-not $cert) {
    throw "No valid '$CertSubject' certificate with a private key in CurrentUser\My. This script never creates one."
}

# 2. Locate signtool.exe (same logic as sign-plith.ps1).
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
if (-not $signtoolPath) { throw 'signtool.exe not found. Install the Windows 10/11 SDK.' }

# 3. Build with UIAccess requested, through the machine-wide lock.
$env:PLITH_OSD_UIACCESS = '1'
Push-Location $repoRoot
try {
    & node (Join-Path $HOME '.claude\bin\heavy-run.mjs') -- cargo build --release -p plith-osd
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed.' }
} finally {
    Pop-Location
    Remove-Item Env:\PLITH_OSD_UIACCESS -ErrorAction SilentlyContinue
}
if (-not (Test-Path $exe)) { throw "Built exe not found at $exe" }

# 4. Sign with a SHA-256 digest and a timestamp. A failed timestamp fails the script.
Write-Host "Signing $exe with $signtoolPath..."
& $signtoolPath sign /sha1 $cert.Thumbprint /fd SHA256 `
    /tr 'http://timestamp.digicert.com' /td SHA256 $exe | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed for $exe" }

# 5. Verify.
$sig = Get-AuthenticodeSignature $exe
if ($sig.Status -ne 'Valid') { throw "Signature status is $($sig.Status), expected Valid." }

Write-Host "Signed exe:  $exe"
Write-Host "Thumbprint:  $($cert.Thumbprint)"
