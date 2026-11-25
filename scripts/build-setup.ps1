# scripts/build-setup.ps1: builds the signed release installer release\Plith-Setup-<version>.exe.
# plith.exe and plith-osd.exe (UIAccess) are built and signed first, copied to a fresh
# target\setup-payload-<stamp>\ folder that plith-setup embeds, then plith-setup is built and
# signed. Never creates or imports a certificate, never runs the installer.

[CmdletBinding()]
param(
    [string]$CertSubject = 'CN=Plith Self-Signed'
)

$ErrorActionPreference = 'Stop'

# The thumbprint of the developer's certificate; every output must be signed by exactly it.
$pinnedThumbprint = '144A7919A1FC941FDC3A9443C1DBAA7AE002AC0F'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$targetDir = Join-Path $repoRoot 'target\setup-release'
. (Join-Path $PSScriptRoot 'lib\signing.ps1')
. (Join-Path $PSScriptRoot 'lib\payload-check.ps1')

# Runs one cargo release build of a package through the machine-wide lock.
function Invoke-ReleaseBuild {
    param([string]$Package)
    & node (Join-Path $HOME '.claude\bin\heavy-run.mjs') -- cargo build --release --target-dir $targetDir --package $Package
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed for $Package." }
}

Push-Location $repoRoot
try {
    # 1. Fail early: certificate, signtool, version, and a free output name.
    $cert = Get-PlithSigningCert -Subject $CertSubject
    if ($cert.Thumbprint -ne $pinnedThumbprint) {
        throw "Certificate thumbprint $($cert.Thumbprint) is not the pinned $pinnedThumbprint."
    }
    $signtool = Get-PlithSigntool

    $metadata = (& cargo metadata --format-version 1 --no-deps) | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
    $version = ($metadata.packages | Where-Object { $_.name -eq 'plith-setup' } | Select-Object -First 1).version
    if (-not $version) { throw 'Could not read the plith-setup version from cargo metadata.' }

    $releaseDir = Join-Path $repoRoot 'release'
    $output = Join-Path $releaseDir "Plith-Setup-$version.exe"
    if (Test-Path -LiteralPath $output) {
        throw "$output already exists. Move it away first; this script never overwrites or deletes it."
    }

    # 2. plith.exe and plith-osd.exe (UIAccess requested), signed.
    Invoke-ReleaseBuild 'plith'
    $env:PLITH_OSD_UIACCESS = '1'
    try {
        Invoke-ReleaseBuild 'plith-osd'
    } finally {
        Remove-Item Env:\PLITH_OSD_UIACCESS -ErrorAction SilentlyContinue
    }
    $targetRelease = Join-Path $targetDir 'release'
    $app = Join-Path $targetRelease 'plith.exe'
    $osd = Join-Path $targetRelease 'plith-osd.exe'
    foreach ($exe in @($app, $osd)) {
        if (-not (Test-Path -LiteralPath $exe)) { throw "Built exe not found at $exe" }
        Invoke-PlithSign -Signtool $signtool -Thumbprint $cert.Thumbprint -Path $exe
    }

    # 3. A fresh payload folder, named by timestamp, so no old folder is ever cleared.
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $payload = Join-Path $repoRoot "target\setup-payload-$stamp"
    if (Test-Path -LiteralPath $payload) { throw "$payload already exists." }
    New-Item -ItemType Directory -Path $payload | Out-Null
    Copy-Item -LiteralPath $app -Destination (Join-Path $payload 'plith.exe')
    Copy-Item -LiteralPath $osd -Destination (Join-Path $payload 'plith-osd.exe')
    $payloadBytes = (Get-Item (Join-Path $payload 'plith.exe')).Length +
        (Get-Item (Join-Path $payload 'plith-osd.exe')).Length

    # 4. plith-setup with the payload embedded (build.rs re-runs on the env var and the files).
    $env:PLITH_SETUP_PAYLOAD = $payload
    try {
        Invoke-ReleaseBuild 'plith-setup'
    } finally {
        Remove-Item Env:\PLITH_SETUP_PAYLOAD -ErrorAction SilentlyContinue
    }
    $setup = Join-Path $targetRelease 'plith-setup.exe'
    if (-not (Test-Path -LiteralPath $setup)) { throw "Built exe not found at $setup" }
    $unsignedSize = (Get-Item -LiteralPath $setup).Length
    Assert-PlithPayloadEmbedded -SetupExe $setup -PayloadDir $payload `
        -Originals @{ 'plith.exe' = $app; 'plith-osd.exe' = $osd } -PinnedThumbprint $pinnedThumbprint
    Invoke-PlithSign -Signtool $signtool -Thumbprint $cert.Thumbprint -Path $setup

    # 5. Verify all three: Valid and signed by the pinned certificate.
    foreach ($exe in @($app, $osd, $setup)) {
        $sig = Get-AuthenticodeSignature -LiteralPath $exe
        if ($sig.Status -ne 'Valid') { throw "$exe signature is $($sig.Status), expected Valid." }
        if ($sig.SignerCertificate.Thumbprint -ne $pinnedThumbprint) {
            throw "$exe is signed by $($sig.SignerCertificate.Thumbprint), not the pinned certificate."
        }
        Write-Host "$($sig.Status)  $exe"
    }

    # 6. Publish to release\ (create the folder, never overwrite).
    New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
    Copy-Item -LiteralPath $setup -Destination $output

    Write-Host ''
    Write-Host "Payload folder: $payload"
    Write-Host "Release:        $output"
    foreach ($pair in @(@('plith.exe', $app), @('plith-osd.exe', $osd), @('Setup', $output))) {
        $file = Get-Item -LiteralPath $pair[1]
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        Write-Host ("{0,-14} {1,12} bytes  SHA-256 {2}" -f $pair[0], $file.Length, $hash)
    }
    Write-Host "Unsigned setup size $unsignedSize, payload $payloadBytes."
} finally {
    Pop-Location
}
