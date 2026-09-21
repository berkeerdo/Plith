# scripts/check-publish-shape.ps1 - fails if the published Plith is missing a file the product
# needs at runtime.
#
# Why this exists: the shelf could not work in any installed build, and nothing noticed.
#
# Plith.csproj copies Plith.DropCatcher beside Plith.exe from a target that runs
# AfterTargets="Build", writing into $(OutDir). That is the BUILD output. `dotnet publish`
# computes its own file set and never picked those files up, so the publish output had no
# catcher in it. scripts/manual-install.ps1 copies the publish stage into C:\Program Files\Plith,
# and DropCatcherLauncher looks for the catcher beside Plith.exe and nowhere else, so an
# installed Plith reported NotFound and the shelf could never open.
#
# Measured on 2026-09-19 at the physical console: C:\Program Files\Plith\ held Plith.exe and no
# Plith.DropCatcher.exe, and a fresh `dotnet publish` produced 17 files with the catcher absent
# from all of them. A green build, 473 + 17 green tests and three green lints all passed over it,
# because every one of them looks at the build output and none of them looks at what ships.
#
# This gate publishes for real and looks at the artifact, which is the only thing that can catch
# a file that builds correctly and ships nowhere.
#
# Run with: pwsh -File scripts/check-publish-shape.ps1

[CmdletBinding()]
param(
    [string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$project = Join-Path $repoRoot 'src\Plith\Plith.csproj'

# Files the installed product cannot run without. Each entry names WHY, so a failure reads as a
# consequence rather than as a missing file.
$required = @(
    @{ Name = 'Plith.exe';              Because = 'the application itself' }
    @{ Name = 'Plith.DropCatcher.exe';  Because = 'DropCatcherLauncher looks beside Plith.exe and nowhere else; without it every shelf drop reports NotFound' }
    @{ Name = 'Plith.DropCatcher.dll';  Because = 'the catcher exe is an apphost and will not start without its assembly' }
    @{ Name = 'Plith.DropCatcher.runtimeconfig.json'; Because = 'the catcher will not start without its runtime config' }
)

$stage = Join-Path ([IO.Path]::GetTempPath()) ("plith-publish-shape-" + [Guid]::NewGuid().ToString('N'))

try {
    Write-Host "Publishing $Configuration to a temporary stage to inspect what ships..."
    & dotnet publish $project -c $Configuration -o $stage -p:PublishSingleFile=false -p:SelfContained=false --nologo 2>&1 |
        Select-Object -Last 3 | Out-Host
    if ($LASTEXITCODE -ne 0) {
        Write-Host "check-publish-shape.ps1 FAILED: dotnet publish did not succeed." -ForegroundColor Red
        exit 1
    }

    $missing = @()
    foreach ($item in $required) {
        if (-not (Test-Path (Join-Path $stage $item.Name))) { $missing += $item }
    }

    $count = (Get-ChildItem -Path $stage -File).Count
    Write-Host "Publish produced $count file(s) in the root of the stage."

    if ($missing.Count -gt 0) {
        Write-Host "check-publish-shape.ps1 FAILED: the published product is missing $($missing.Count) required file(s)." -ForegroundColor Red
        foreach ($item in $missing) {
            Write-Host "  MISSING $($item.Name) - $($item.Because)" -ForegroundColor Red
        }
        Write-Host ""
        Write-Host "A file that builds but does not publish is a file the installed product does not have."
        exit 1
    }

    Write-Host "Publish shape check passed: all $($required.Count) required file(s) reach the publish output." -ForegroundColor Green
    exit 0
}
finally {
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue }
}
