# Regenerates the floor constants in src/Plith/Services/LoftPalette.cs from praxvon-ui's
# theme tokens, the sibling of that repo's own `build-tokens`.
#
# The generated values are checked in on purpose: a Plith build must not need the praxvon-ui
# checkout. This script is run by a person when praxvon-ui's theme moves, and its output is
# reviewed like any other change. LoftAccentTests.DarkFloor_MatchesTheSpec is the guard that
# fails if someone edits the constants by hand instead.

[CmdletBinding()]
param(
    [string]$PraxvonUi = 'C:\Projects\praxvon-ui',
    [string]$Target = (Join-Path $PSScriptRoot '..\src\Plith\Services\LoftPalette.cs')
)

$ErrorActionPreference = 'Stop'

$themePath = Join-Path $PraxvonUi 'tokens\themes\praxvon.json'
if (-not (Test-Path $themePath)) { throw "praxvon theme not found at $themePath" }

$theme = Get-Content $themePath -Raw | ConvertFrom-Json

function Emit([object]$mode, [string]$name) {
    $fields = 'background', 'surface', 'foreground', 'text-secondary', 'text-tertiary', 'border', 'border-strong'
    $names  = 'Background', 'Surface', 'Foreground', 'TextSecondary', 'TextTertiary', 'Border', 'BorderStrong'

    $lines = for ($i = 0; $i -lt $fields.Count; $i++) {
        $hex = $mode.($fields[$i])
        if ($hex -notmatch '^#[0-9A-Fa-f]{6}$') { throw "$name.$($fields[$i]) is not a 6 digit hex: $hex" }
        $r = "0x$($hex.Substring(1,2))"; $g = "0x$($hex.Substring(3,2))"; $b = "0x$($hex.Substring(5,2))"
        "        {0,-14} Rgb({1}, {2}, {3})," -f "$($names[$i]):", $r, $g, $b
    }
    $lines -join "`r`n"
}

$dark  = Emit $theme.dark  'dark'
$light = Emit $theme.light 'light'

Write-Host "dark floor:"  -ForegroundColor Cyan; Write-Host $dark
Write-Host "light floor:" -ForegroundColor Cyan; Write-Host $light
Write-Host ""
Write-Host "Paste these into $Target, replacing the two initialiser blocks, then run:" -ForegroundColor Yellow
Write-Host "  dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter FullyQualifiedName~LoftAccentTests"
