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
. (Join-Path $PSScriptRoot 'lib\signing.ps1')
$cert = Get-PlithSigningCert -Subject $CertSubject

# 2. Locate signtool.exe.
$signtoolPath = Get-PlithSigntool

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

# 4. Sign with a SHA-256 digest and a timestamp, then verify. A failed timestamp fails the script.
Invoke-PlithSign -Signtool $signtoolPath -Thumbprint $cert.Thumbprint -Path $exe

$hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
Write-Host "Signed exe:  $exe"
Write-Host "Thumbprint:  $($cert.Thumbprint)"
Write-Host "SHA-256:     $hash"
Write-Host "Install (under sudo): scripts\install-osd.ps1 -Yes -Sha256 $hash"
