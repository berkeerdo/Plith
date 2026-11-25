# scripts/lib/signing.ps1: signing helpers shared by build-osd.ps1 and build-setup.ps1.
# Dot-source it. Never creates or imports a certificate.

# Returns the newest valid certificate with a private key in CurrentUser\My, or throws.
function Get-PlithSigningCert {
    param([string]$Subject = 'CN=Plith Self-Signed')
    $cert = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object { $_.Subject -eq $Subject -and $_.NotAfter -gt (Get-Date) -and $_.HasPrivateKey } |
        Sort-Object NotAfter -Descending |
        Select-Object -First 1
    if (-not $cert) {
        throw "No valid '$Subject' certificate with a private key in CurrentUser\My. This script never creates one."
    }
    return $cert
}

# Returns the path of signtool.exe: on PATH, else the newest x64 one under Windows Kits.
function Get-PlithSigntool {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $candidates = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" `
        -Filter 'signtool.exe' -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match 'x64' } |
        Sort-Object FullName -Descending
    if ($candidates) { return $candidates[0].FullName }
    throw 'signtool.exe not found. Install the Windows 10/11 SDK.'
}

# Signs one file with SHA-256 and a timestamp, then requires a Valid signature.
function Invoke-PlithSign {
    param([string]$Signtool, [string]$Thumbprint, [string]$Path)
    Write-Host "Signing $Path with $Signtool..."
    & $Signtool sign /sha1 $Thumbprint /fd SHA256 `
        /tr 'http://timestamp.digicert.com' /td SHA256 $Path | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for $Path" }
    $sig = Get-AuthenticodeSignature $Path
    if ($sig.Status -ne 'Valid') { throw "Signature status is $($sig.Status) for $Path, expected Valid." }
}
