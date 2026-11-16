# scripts/install-osd.ps1: installs the signed plith-osd.exe under Program Files.
# Without -Yes it only prints what it would do (and checks the source read-only).
# -Yes -Sha256 <hex> installs; run it under sudo. The hash is the one build-osd.ps1 printed:
#   the copy inside Program Files must match it and be signed by the pinned certificate.
# -CheckAcl only checks the installed folders and exe read-only (no sudo needed).
# -Uninstall -Yes removes only the installed exe and the empty osd folder.
#
# Why the pins: the developer key sits in CurrentUser\My, so any same-user process could sign
# a swapped target\release exe with it; "signature Valid" alone would then install that exe as
# UIAccess. The hash ties the install to the build the developer just saw, the thumbprint to
# this certificate. After the copy no non-admin SID may write to the install, because UIAccess
# trusts the path, not the ACL. Limit: a same-user process can still edit this script itself,
# so read it before running it elevated. A real certificate is a ROADMAP item.

[CmdletBinding()]
param(
    [switch]$Yes,
    [switch]$Uninstall,
    [switch]$CheckAcl,
    [string]$Sha256 = ''
)

$ErrorActionPreference = 'Stop'

# CN=Plith Self-Signed, valid to 2031-05-30. Change only together with build-osd.ps1's cert.
$PinnedThumbprint = '144A7919A1FC941FDC3A9443C1DBAA7AE002AC0F'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$source = Join-Path $repoRoot 'target\release\plith-osd.exe'
$plithDir = 'C:\Program Files\Plith'
$destDir = 'C:\Program Files\Plith\osd'
$dest = 'C:\Program Files\Plith\osd\plith-osd.exe'
$temp = 'C:\Program Files\Plith\osd\plith-osd.exe.new'

# SIDs that may write: SYSTEM, Administrators, TrustedInstaller, and CREATOR OWNER (it maps to
# the owner of a new child, and owners are checked separately).
$AdminSids = @(
    'S-1-5-18',
    'S-1-5-32-544',
    'S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464'
)
$CreatorOwner = 'S-1-3-0'
# Rights that let a holder change what runs: write or append data (add file or subfolder),
# write extended attributes or attributes, delete child, delete, WRITE_DAC, WRITE_OWNER,
# GENERIC_ALL, GENERIC_WRITE.
$WriteMask = 0x2 -bor 0x4 -bor 0x10 -bor 0x40 -bor 0x100 -bor 0x10000 -bor 0x40000 `
    -bor 0x80000 -bor 0x10000000 -bor 0x40000000

function Get-AclProblems([string]$Path) {
    $problems = @()
    $acl = Get-Acl -LiteralPath $Path
    $sidType = [System.Security.Principal.SecurityIdentifier]
    $owner = $acl.GetOwner($sidType).Value
    if ($AdminSids -notcontains $owner) { $problems += "$Path is owned by $owner" }
    foreach ($rule in $acl.GetAccessRules($true, $true, $sidType)) {
        if ($rule.AccessControlType -ne 'Allow') { continue }
        $sid = $rule.IdentityReference.Value
        if ($AdminSids -contains $sid -or $sid -eq $CreatorOwner) { continue }
        $rights = [int64]$rule.FileSystemRights -band 0xFFFFFFFFL
        if (($rights -band $WriteMask) -ne 0) {
            $problems += ('{0}: {1} has rights 0x{2:X8}' -f $Path, $sid, $rights)
        }
    }
    return $problems
}

function Assert-AdminOnly([string[]]$Paths) {
    $problems = @()
    foreach ($path in $Paths) {
        if (Test-Path -LiteralPath $path) { $problems += Get-AclProblems $path }
    }
    if ($problems) { throw ("Non-admin write access on the install:`n  " + ($problems -join "`n  ")) }
}

function Assert-Signer([string]$Path) {
    $sig = Get-AuthenticodeSignature -LiteralPath $Path
    if ($sig.Status -ne 'Valid') { throw "$Path signature is $($sig.Status), expected Valid." }
    $thumb = $sig.SignerCertificate.Thumbprint
    if ($thumb -ne $PinnedThumbprint) { throw "$Path is signed by $thumb, expected $PinnedThumbprint." }
}

function Assert-Hash([string]$Path, [string]$Expected) {
    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    if ($actual -ne $Expected) { throw "$Path has SHA-256 $actual, expected $Expected." }
}

if ($CheckAcl) {
    Assert-AdminOnly @($plithDir, $destDir, $dest)
    if (Test-Path -LiteralPath $dest) { Assert-Signer $dest }
    Write-Host "ACL ok: only SYSTEM, Administrators and TrustedInstaller can change $plithDir, $destDir and the exe."
    exit 0
}

if ($Yes) {
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'install-osd.ps1 -Yes needs an elevated process: run under sudo.'
    }
}

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
    $hash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    Assert-Signer $source
    Write-Host "Source: $source"
    Write-Host "  SHA-256 $hash, signed by the pinned certificate $PinnedThumbprint"
    Write-Host 'Dry run. Would:'
    Write-Host "  stop plith-osd if running from $dest"
    Write-Host "  check that only admins can change $plithDir and $destDir"
    Write-Host "  copy the source to $temp, check its SHA-256 against -Sha256 and its signer"
    Write-Host "  rename it to $dest and check the ACL of the folders and the exe again"
    Write-Host 'Compare the hash with the one build-osd.ps1 printed, then run under sudo:'
    Write-Host "  scripts\install-osd.ps1 -Yes -Sha256 <the hash build-osd.ps1 printed>"
    exit 0
}

if ($Sha256 -notmatch '^[0-9A-Fa-f]{64}$') {
    throw 'install-osd.ps1 -Yes needs -Sha256 <64 hex digits>, the hash build-osd.ps1 printed.'
}
$expectedHash = $Sha256.ToUpperInvariant()

# Fail fast on the source; the decisive checks run on the copy inside Program Files, where a
# medium process can no longer swap it.
Assert-Signer $source
Assert-Hash $source $expectedHash

Stop-InstalledOsd
New-Item -ItemType Directory -Path $destDir -Force | Out-Null
Assert-AdminOnly @($plithDir, $destDir)

try {
    Copy-Item -LiteralPath $source -Destination $temp -Force
    Assert-Hash $temp $expectedHash
    Assert-Signer $temp
    Move-Item -LiteralPath $temp -Destination $dest -Force
} catch {
    if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Force }
    throw
}

try {
    Assert-AdminOnly @($plithDir, $destDir, $dest)
    Assert-Signer $dest
} catch {
    # Fail closed: an exe others can replace must not stay installed as UIAccess.
    Remove-Item -LiteralPath $dest -Force
    throw
}
Write-Host "Installed $dest (SHA-256 $expectedHash, signer $PinnedThumbprint, admin-only ACL)"
