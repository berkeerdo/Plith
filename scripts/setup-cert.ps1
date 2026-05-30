# scripts/setup-cert.ps1 — idempotent self-signed code-signing cert setup for Plith.
# Requires admin (TrustedPublisher store is HKLM-scoped).
# Emits the cert thumbprint on the last stdout line so install-local.ps1 can capture it.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$current = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $current.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "setup-cert.ps1 requires administrator privileges. Right-click PowerShell -> Run as administrator."
}

$subject = 'CN=Plith Self-Signed'
$thumbFile = Join-Path $PSScriptRoot '.cert-thumbprint'

# 1. Find or create the cert in CurrentUser\My.
$cert = Get-ChildItem -Path Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $subject -and $_.NotAfter -gt (Get-Date) } |
    Select-Object -First 1

if (-not $cert) {
    Write-Host "Generating new self-signed CodeSigning cert (5-year validity)..."
    $cert = New-SelfSignedCertificate `
        -Subject $subject `
        -Type CodeSigningCert `
        -KeyUsage DigitalSignature `
        -FriendlyName 'Plith Code Signing' `
        -CertStoreLocation Cert:\CurrentUser\My `
        -NotAfter (Get-Date).AddYears(5)
} else {
    Write-Host "Reusing existing Plith cert (thumbprint $($cert.Thumbprint))."
}

# 2. Persist thumbprint for install-local.ps1.
Set-Content -Path $thumbFile -Value $cert.Thumbprint -NoNewline

# 3. Ensure public cert is in BOTH stores so Windows honors UIAccess.
#    TrustedPublisher alone is not enough for a self-signed cert — uiAccess validation
#    walks the chain and rejects the binary if the signing cert is not also a trusted
#    root. Without Root the binary silently fails to launch from Program Files.
$tempCer = [IO.Path]::Combine($env:TEMP, "Plith.cer")
$exported = $false

foreach ($storeName in 'TrustedPublisher', 'Root') {
    $store = "Cert:\LocalMachine\$storeName"
    $present = Get-ChildItem -Path $store | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
    if ($present) {
        Write-Host "Public cert already in LocalMachine\$storeName."
        continue
    }
    if (-not $exported) {
        Export-Certificate -Cert $cert -FilePath $tempCer | Out-Null
        $exported = $true
    }
    Write-Host "Importing public cert into LocalMachine\$storeName..."
    Import-Certificate -FilePath $tempCer -CertStoreLocation $store | Out-Null
}

if ($exported) { Remove-Item -Path $tempCer -ErrorAction SilentlyContinue }

# Final stdout: thumbprint, so caller can `$thumb = & setup-cert.ps1 | Select-Object -Last 1`.
$cert.Thumbprint
