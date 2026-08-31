# scripts/sign-plith.ps1 - Signs Plith.exe with the developer's self-signed cert and
# exports the certificate's public key next to the installer resources so the installer
# can install it into user trust stores without needing signtool at run time.
#
# Called from Plith.Installer.csproj's PublishPlithAndEmbed target after publish and
# before the ZipDirectory step. This shifts UIAccess signing from install time (which
# required every end user's machine to have the Windows SDK) to build time (where only
# the developer needs signtool + a code-signing cert).

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)] [string]$PlithExe,
    [Parameter(Mandatory = $true)] [string]$CertOutput,
    [string]$CertSubject = 'CN=Plith Self-Signed'
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $PlithExe)) { throw "PlithExe not found at $PlithExe" }

# 1. Find (or create) the dev signing cert.
$cert = Get-ChildItem Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $CertSubject -and $_.NotAfter -gt (Get-Date) -and $_.HasPrivateKey } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1

if (-not $cert) {
    Write-Host "No usable '$CertSubject' cert in CurrentUser\My. Creating a new 5-year cert..."
    $cert = New-SelfSignedCertificate `
        -Subject $CertSubject `
        -Type CodeSigningCert `
        -KeyAlgorithm RSA `
        -KeyLength 2048 `
        -HashAlgorithm SHA256 `
        -NotAfter (Get-Date).AddYears(5) `
        -CertStoreLocation Cert:\CurrentUser\My `
        -KeyExportPolicy Exportable
    Write-Host "Created cert with thumbprint $($cert.Thumbprint)"
}

# 2. Locate signtool.exe (same logic as build-release.ps1's fallback).
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
    throw "signtool.exe not found on this build machine. Install the Windows 10/11 SDK."
}

# 3. Sign Plith.exe. SHA-256 file digest + timestamp so the signature stays valid past
#    the cert's expiry (timestamped signatures are anchored to signing time, not verify time).
Write-Host "Signing $PlithExe with $signtoolPath..."
& $signtoolPath sign /sha1 $cert.Thumbprint /fd SHA256 `
    /tr 'http://timestamp.digicert.com' /td SHA256 $PlithExe | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool failed for $PlithExe" }

# 4. Export the cert's public key next to the installer resources. Public-only .cer -
#    the private key stays on the build machine. This is the file the installer will
#    read at install time and add to LocalMachine\Root + TrustedPublisher so that the
#    pre-signed Plith.exe validates on the end user's box.
$certDir = Split-Path -Parent $CertOutput
if ($certDir -and -not (Test-Path $certDir)) { New-Item -ItemType Directory -Path $certDir -Force | Out-Null }
Export-Certificate -Cert $cert -FilePath $CertOutput -Force | Out-Null
Write-Host "Exported public cert to $CertOutput"
