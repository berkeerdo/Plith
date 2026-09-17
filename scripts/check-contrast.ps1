# scripts/check-contrast.ps1 — measures text contrast for every colour pair the XAML declares,
# across both themes and a spread of accents.
#
# Why this exists: a user reported white text on a pink panel in the light theme and could not
# read it. The cause was ink declared as a constant sitting on a surface the user tints — measured
# afterwards at 1.2:1, where 4.5:1 is the threshold for body text. It was not a near miss and it
# was not one colour: every accent failed the same way on the light theme, and the primary button
# failed on any dark accent in either theme.
#
# None of that was visible to a build, a test or the render harness, which only ever drew the dark
# theme. So the pairs are measured instead of looked at.
#
# The pair list is NOT curated. The script reads the XAML for elements and styles that set both a
# Foreground and a Background from resources, so a pair added later is covered without anyone
# remembering to add it here. That also means it only sees pairs stated in one place — a
# foreground set on a child of a background set on a parent is invisible to it, and always will
# be. It is a floor, not a proof.
#
# Run with: pwsh -File scripts/check-contrast.ps1

[CmdletBinding()]
param(
    [string]$Configuration = 'Debug',

    # A spread rather than every preset: pale, mid, deep, one on each side of the light/dark
    # crossover where a threshold-based implementation would go wrong - and both true extremes,
    # which are the ones that were reported broken and are now the ones that move the surface.
    [string[]]$Accents = @('#A3E635', '#EC4899', '#1E3A8A', '#FACC15', '#64748B',
                           '#000000', '#FFFFFF', '#0A0A0A', '#F5F5F5'),

    # WCAG AA for body text.
    [double]$Threshold = 4.5,

    # And for things that are not text. WCAG allows 3.0 for user-interface components and
    # graphical objects, which is a real distinction rather than a discount - a 4 DIP bar and a
    # transport glyph are read by shape, not by letterform.
    [double]$NonTextThreshold = 3.0
)

# The pairs that are not text, and why. Listed rather than inferred, because nothing in the markup
# says whether a Foreground paints a letter or a triangle - and a script that guessed would either
# wave through real text or fail honest shapes.
#
# Every entry here is a decision someone made, not a pass someone wanted. Anything not listed is
# held to the text threshold.
# Pairs this script cannot resolve, and why. Different from the list below it: those are pairs
# measured against a lower bar, these are pairs it cannot measure at all.
#
# Recorded rather than quietly skipped, and rather than satisfied by inventing a constant. A gate
# that is made to pass by writing a fake colour into the product has been turned into decoration.
$unresolvable = @{
    'MediaWidget.xaml:TransportButtonStyle' =
        'both layers are painted by the page, not declared: a translucent chip over a near-black scrim over album artwork. Nothing static knows what is underneath. Checked instead in the render harness, light and dark - see widget-media.png.'
}

$nonText = @{
    'SettingsTheme.xaml:ModernProgressBarStyle' =
        'the accent FILL inside its own groove - a 4 DIP bar, read as a length and not a glyph'
}

$ErrorActionPreference = 'Stop'
if ([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {
    throw "Must run under -STA. Use: pwsh -STA -File $PSCommandPath"
}

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
$dll = Join-Path $bin 'Plith.dll'
if (-not (Test-Path $dll)) { throw "Build $Configuration first — $dll not found." }

Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase
$null = [Reflection.Assembly]::LoadFrom($dll)
if ($null -eq [Windows.Application]::Current) { $null = [Windows.Application]::new() }

# --- the pairs, read from the XAML ---------------------------------------------------------
$pairs = [System.Collections.Generic.List[object]]::new()
$xamlFiles = Get-ChildItem -Path (Join-Path $root 'src\Plith') -Filter '*.xaml' -Recurse |
    Where-Object { $_.FullName -notmatch '[\\/](obj|bin)[\\/]' }

# A file that defines a brush key for itself wins over the theme for anything inside it.
#
# Without this the gate measures pairs that do not occur: the media page declares its own ink,
# because it paints its own near-black scrim over the artwork and is not sitting on the themed
# surface at all. Resolving its foreground from the theme produced four failures for colours that
# are never drawn together. A gate that reports pairs the product does not have is a gate people
# learn to argue with.
$localBrushes = @{}
foreach ($file in $xamlFiles) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    foreach ($m in [regex]::Matches($text, '<SolidColorBrush\s+x:Key="([^"]+)"\s+Color="(#[0-9A-Fa-f]{6,8})"')) {
        $localBrushes["$($file.Name)|$($m.Groups[1].Value)"] = $m.Groups[2].Value
    }
}

foreach ($file in $xamlFiles) {
    $text = Get-Content -LiteralPath $file.FullName -Raw

    foreach ($m in [regex]::Matches($text, '(?s)<Style[^>]*x:Key="([^"]+)"[^>]*>(.*?)</Style>')) {
        $body = $m.Groups[2].Value
        $fg = [regex]::Match($body, 'Property="Foreground"\s+Value="\{(?:Dynamic|Static)Resource\s+([^}]+)\}"')
        $bg = [regex]::Match($body, 'Property="Background"\s+Value="\{(?:Dynamic|Static)Resource\s+([^}]+)\}"')
        if ($fg.Success -and $bg.Success) {
            $pairs.Add([pscustomobject]@{
                Bg = $bg.Groups[1].Value.Trim(); Fg = $fg.Groups[1].Value.Trim()
                Where = "$($file.Name):$($m.Groups[1].Value)"; File = $file.Name
            })
        }
    }

    foreach ($m in [regex]::Matches($text, '(?s)<[A-Za-z]+\b[^>]*?>')) {
        $el = $m.Value
        $fg = [regex]::Match($el, 'Foreground="\{(?:Dynamic|Static)Resource\s+([^}]+)\}"')
        $bg = [regex]::Match($el, 'Background="\{(?:Dynamic|Static)Resource\s+([^}]+)\}"')
        if ($fg.Success -and $bg.Success) {
            $pairs.Add([pscustomobject]@{
                Bg = $bg.Groups[1].Value.Trim(); Fg = $fg.Groups[1].Value.Trim()
                Where = $file.Name; File = $file.Name
            })
        }
    }
}

$pairs = $pairs | Sort-Object Bg, Fg, Where, File -Unique
Write-Host "Pairs declared in XAML: $($pairs.Count)"

# --- resolving a key to a colour ------------------------------------------------------------
function New-Scope([string]$Theme, $Override) {
    $probe = [Windows.Controls.Border]::new()
    foreach ($rel in @('Resources/Theme.xaml', "Resources/Palette.$Theme.xaml",
                       "Resources/OsdPalette.$Theme.xaml", 'Resources/SettingsTheme.xaml')) {
        $d = [Windows.ResourceDictionary]::new()
        $d.psbase.Source = [Uri]::new("pack://application:,,,/Plith;component/$rel", [UriKind]::Absolute)
        $probe.Resources.MergedDictionaries.Add($d)
    }
    if ($Override) { $probe.Resources.MergedDictionaries.Add($Override) }
    return $probe
}

# A translucent background is not a colour on its own — it composites over whatever is behind it.
# Flattened against the theme's own window background, which is what these overlays sit on.
function Resolve-Color($Scope, [string]$Key, $Under) {
    $b = $Scope.TryFindResource($Key)
    if ($null -eq $b) { return $null }
    if ($b -is [Windows.Media.LinearGradientBrush]) {
        # The harder end: text has to clear the threshold across the whole surface.
        $stops = $b.GradientStops | Sort-Object Offset
        return $stops[$stops.Count - 1].Color
    }
    if ($b -isnot [Windows.Media.SolidColorBrush]) { return $null }
    $c = $b.Color
    if ($c.A -eq 255 -or $null -eq $Under) { return $c }
    $a = $c.A / 255.0
    return [Windows.Media.Color]::FromRgb(
        [byte][math]::Round($c.R * $a + $Under.R * (1 - $a)),
        [byte][math]::Round($c.G * $a + $Under.G * (1 - $a)),
        [byte][math]::Round($c.B * $a + $Under.B * (1 - $a)))
}

$failures = [System.Collections.Generic.List[string]]::new()
$measured = 0

foreach ($theme in @('Dark', 'Light')) {
    foreach ($accentHex in $Accents) {
        $base = [Plith.Services.AccentTheme]::ParseHexColor($accentHex, [Windows.Media.Colors]::Gray)
        $override = [Plith.Services.ThemeService]::BuildAccentOverride($base, ($theme -eq 'Dark'))
        $scope = New-Scope $theme $override

        $under = Resolve-Color $scope 'WindowBg' $null
        if ($null -eq $under) { $under = Resolve-Color $scope 'OsdSurfaceBrush' $null }

        foreach ($p in $pairs) {
            $localBg = $localBrushes["$($p.File)|$($p.Bg)"]
            $localFg = $localBrushes["$($p.File)|$($p.Fg)"]

            $bg = if ($localBg) { [Windows.Media.ColorConverter]::ConvertFromString($localBg) }
                  else { Resolve-Color $scope $p.Bg $under }
            $fg = if ($localFg) { [Windows.Media.ColorConverter]::ConvertFromString($localFg) }
                  else { Resolve-Color $scope $p.Fg $bg }
            if ($null -eq $bg -or $null -eq $fg) { continue }

            if ($unresolvable.ContainsKey($p.Where)) { continue }

            $measured++
            $ratio = [Plith.Services.ContrastInk]::ContrastRatio($bg, $fg)
            $limit = if ($nonText.ContainsKey($p.Where)) { $NonTextThreshold } else { $Threshold }

            if ($ratio -lt $limit) {
                $kind = if ($limit -eq $NonTextThreshold) { 'non-text' } else { 'text' }
                $failures.Add(("  {0,-34} {1} on {2}  accent {3}  {4}  {5:N1}:1  (needs {6}, {7})" -f `
                    $p.Where, $p.Fg, $p.Bg, $accentHex, $theme.PadRight(5), $ratio, $limit, $kind))
            }
        }
    }
}

Write-Host "Measurements: $measured across $(($Accents).Count) accent(s) x 2 themes"

if ($failures.Count -gt 0) {
    Write-Host ""
    Write-Host "Contrast check failed — text below ${Threshold}:1" -ForegroundColor Red
    Write-Host ""
    $failures | Sort-Object -Unique | ForEach-Object { Write-Host $_ }
    Write-Host ""
    Write-Host "Derive the foreground from the colour behind it (Services/ContrastInk.cs) rather"
    Write-Host "than declaring it, or give the surface its own ink where it paints its own ground."
    exit 1
}

Write-Host "Contrast check passed: text pairs clear ${Threshold}:1, the $($nonText.Count) listed non-text pair(s) clear ${NonTextThreshold}:1, on both themes."
foreach ($k in $nonText.Keys | Sort-Object) { Write-Host "  non-text: $k - $($nonText[$k])" }
foreach ($k in $unresolvable.Keys | Sort-Object) { Write-Host "  NOT MEASURED: $k - $($unresolvable[$k])" }
exit 0
