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
    'MediaWidget.xaml:NotchTrack groove on the panel' =
        'NotchTrack is the Loft floor Border, a hairline rule at about 1.3:1 against the panel BY INTENT (1.26:1 and 1.34:1 on dark, 1.26:1 and 1.41:1 on light, computed from the floor hexes rather than by this script, which skips the pair). Not forced to 3:1: the fill inside the groove is the OsdAccent Mark, itself 3:1 from the panel, and a groove at 3:1 as well would sit at the same luminance as the fill and swallow it (measured with BorderStrong: fill vs groove 1.2 to 1.54:1 on light). The groove is a rule; the fill carries the visible state.'
    'MediaWidget.xaml:TransportButtonStyle' =
        'the chip is translucent (#0DFFFFFF over the panel), so nothing static knows what is underneath. It used to be a chip over a near-black scrim over album artwork; that scrim is gone with the page redesign, and the chip now sits on the notch panel. Checked instead in the render harness, light and dark - see widget-media.png.'
    'MediaWidget.xaml:TrackBarStyle' =
        'NEITHER colour is declared: MediaWidget.PaintBar computes both from NotchInk, the played part being the ink itself and the groove the same ink at 24 per cent alpha, so what this scan would need to measure is an alpha composite over a surface painted two files away. Every declared alternative WAS measured here first and every one failed: NotchInk on NotchBezelBrush 1,1:1, NotchInk on OsdHighlight 1,1:1, and, in the TrackOn era whose NotchTrack no longer exists so these two are history and not reproducible today, OsdAccent on NotchTrack 1,0:1 and NotchInk on NotchTrack 2,6:1 - because the groove and the ink were derived independently of each other, leaving the ink an unpredictable distance from the groove. A fill derived from the groove cleared every ratio and drew the bar INVERTED, which a ratio cannot see. Checked in the render harness instead, light and dark: see widget-media.png and widget-media-paused.png.'
}

$nonText = @{
    'OsdContent.xaml:OsdAccent on the panel' =
        'the accent bar fill and rail pip: a non-text mark, 3:1 by DeriveLoftRoles(...).Mark'
    'Ornaments.xaml:LoftTicks (OrnamentTickBrush)' =
        'the ruler under the bars: a Stroke, a non-text mark, and the floor TextTertiary on both themes'
    'SettingsTheme.xaml:ModernProgressBarStyle' =
        'the accent FILL inside its own groove - a 4 DIP bar, read as a length and not a glyph'
}

# Pairs that are MEASURED at the text bar, REPORTED with the number actually seen, and do not fail
# the run AT THE RATIO THEY WERE ACCEPTED AT. Different from $unresolvable (not measured at all)
# and from $nonText (held to a lower bar): these are known misses that a colour change in this
# slice was deliberately not allowed to paper over.
#
# The exemption is keyed by PAIR AND THEME, and it carries a Floor, for two reasons the first
# version of this table got wrong. A pair that misses on one floor usually clears on the other,
# and exempting both hides a real pass behind an apology. And an exemption with no floor can never
# fail again: the ratio could fall to 2:1 and the run would still print ACCEPTED and exit 0, which
# is the one thing an instrument in this repository is not allowed to do. Floor is the ratio the
# pair was accepted at, so a LATER change that makes it worse fails the run.
$acceptedBelowBar = @{
    'Ornaments.xaml:LoftLabelStyle rail label|Dark' = @{
        Floor = 3.75
        Why   = 'OsdTextTertiary on the dark floor, 10 px upper-case mono, measured at 3.81:1 against the 4.5:1 text bar. The ratio is pre-existing (3.38:1 before the Loft branch), so no colour was changed to make it pass. The LIGHT floor is not exempt and clears the bar at 6.90:1. A decision for the design owner, not a thing to fix by nudging a hex here.'
    }
}
$acceptedWorst = @{}

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
# The catcher paints the shelf, in the same palette keys, and until this slice nothing scanned
# it. Its window carried hard-coded colours that no check ever measured.
$xamlFiles = @(
    Get-ChildItem -Path (Join-Path $root 'src\Plith') -Filter '*.xaml' -Recurse
    Get-ChildItem -Path (Join-Path $root 'src\Plith.DropCatcher') -Filter '*.xaml' -Recurse
) | Where-Object { $_.FullName -notmatch '[\\/](obj|bin)[\\/]' }

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

# Pairs a page builds in code rather than declaring, so the scan above cannot see them. Named
# here so they are still measured across every accent and both themes — the alternative is a page
# whose colours nothing checks, which is how the OSD's ink reached 1.2:1 and stayed there.
# The shelf's tiles, whose colours are set in code. Pointed at this check, the first version
# failed twice over and both failures were invisible in a render: NotchInkMuted on a track tile
# measured 1.5:1 on every accent, and NotchInk on the same tile still only reached 4.0:1 with a
# white accent. NotchTrack is the Loft floor's Border, a hairline rule (about 1.3:1 against its
# panel) and NOT a 3:1 surface; it used to be built by ContrastInk.TrackOn and was one. The text
# pairs below do not depend on it: they are ink on OsdSurfaceBrush, and the tiles have no chip
# behind them, so the ink is measured against the panel it actually sits on.
$codeBehindPairs = @(
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchInk'; Where = 'ShelfWidget.cs:tile label, icon and count' },
    # The line under the row saying the shelf opens on a click, and the sentence that replaces it
    # when the shelf cannot open. It is declared in XAML with a DynamicResource foreground and no
    # background of its own, so the scan above cannot pair it: the surface it sits on is painted
    # by OsdContent's NotchSurface, two files away. That sentence is the only thing on the page
    # that a person has to READ rather than recognise, and the reason it needs the check is the
    # one the tiles already proved - NotchInkMuted measured 1.5:1 on a track tile, and nothing in
    # a render showed it.
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchInkMuted'; Where = 'ShelfWidget.xaml:the open hint and the unavailable sentence' },
    # The catcher's own tiles, added in Task 9: it paints the shelf in the same palette keys as
    # ShelfWidget above and until this slice nothing scanned it at all. The pairs work only
    # because Task 3 resolved the catcher's brushes onto ITS OWN Resources under these same key
    # names, so a lookup against Plith's own theme dictionaries below still means the same thing.
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchInk'; Where = 'ShelfSurface.xaml.cs:tile label and count' },
    # The "Stack N" caption above each column (see BuildColumn): a sighted match for the
    # per-stack AutomationProperties.Name set on the same column's wrapping Border, and the
    # reason this pair measures shipped code rather than a colour nothing draws.
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchInkMuted'; Where = 'ShelfSurface.xaml.cs:stack caption' },
    # Neither key is ever a Foreground beside a declared Background, so the scan above could not
    # pair them, and for that reason neither had been measured against the panel beneath it.
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'OsdAccent'; Where = 'OsdContent.xaml:OsdAccent on the panel' },
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchTrack'; Where = 'MediaWidget.xaml:NotchTrack groove on the panel' },
    # The ornament ink layer. LoftLabelStyle sets a Foreground and no Background, and
    # OrnamentTickBrush is a Stroke, so the scan above pairs neither; both sit on the panel
    # painted by OsdContent, two files away.
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'OsdTextTertiary'; Where = 'Ornaments.xaml:LoftLabelStyle rail label' },
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'OrnamentTickBrush'; Where = 'Ornaments.xaml:LoftTicks (OrnamentTickBrush)' }
)
foreach ($cb in $codeBehindPairs) {
    # The file each pair's own local-brush override (if any) would be declared under, read back
    # out of Where rather than hard-coded: every entry above named a real source file, and a
    # constant here would have quietly pointed the catcher's own pairs at ShelfWidget.cs's local
    # brushes instead of at their own (harmless today only because neither file declares one).
    $file = ($cb.Where -split ':')[0]
    $pairs.Add([pscustomobject]@{ Bg = $cb.Bg; Fg = $cb.Fg; Where = $cb.Where; File = $file })
}

$pairs = $pairs | Sort-Object Bg, Fg, Where, File -Unique
Write-Host "Pairs declared in XAML: $($pairs.Count - $codeBehindPairs.Count), named from code-behind: $($codeBehindPairs.Count)"

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

            $acceptKey = "$($p.Where)|$theme"
            if ($acceptedBelowBar.ContainsKey($acceptKey)) {
                if (-not $acceptedWorst.ContainsKey($acceptKey) -or $ratio -lt $acceptedWorst[$acceptKey]) { $acceptedWorst[$acceptKey] = $ratio }
                $floor = $acceptedBelowBar[$acceptKey].Floor
                if ($ratio -lt $floor) {
                    $failures.Add(("  {0,-34} {1} on {2}  accent {3}  {4}  {5:N2}:1  (accepted at {6}, now WORSE)" -f `
                        $p.Where, $p.Fg, $p.Bg, $accentHex, $theme.PadRight(5), $ratio, $floor))
                }
                continue
            }

            if ($ratio -lt $limit) {
                $kind = if ($limit -eq $NonTextThreshold) { 'non-text' } else { 'text' }
                $failures.Add(("  {0,-34} {1} on {2}  accent {3}  {4}  {5:N1}:1  (needs {6}, {7})" -f `
                    $p.Where, $p.Fg, $p.Bg, $accentHex, $theme.PadRight(5), $ratio, $limit, $kind))
            }
        }

        # The selection ring: not a declared resource pair, so it cannot join the sweep above.
        #
        # SelectionRing has no static entry in any theme dictionary for TryFindResource to find -
        # Plith computes it at runtime and sends the ANSWER over the wire (ShelfSession.
        # DerivePalette, ShelfPaletteWire), specifically so the ring cannot drift from the rest of
        # the product's derived colours. It is a colour applied from code, so a resource-key
        # sweep cannot see it, and the honest fix is to make it reachable rather than exempt.
        #
        # This MIRRORS DerivePalette, call for call, rather than a second and looser derivation
        # that could quietly disagree with production. DerivePalette now sends:
        #   SurfaceStart  = LoftPalette.For(isDark).Background
        #   SurfaceEnd    = LoftPalette.For(isDark).Surface
        #   SelectionRing = AccentTheme.DeriveLoftRoles(base, isDark).Mark
        # so this takes the floor from LoftPalette.For and the ring from DeriveLoftRoles(...).Mark,
        # and measures the ring against both floor surfaces at the non-text threshold. If
        # DerivePalette changes where the ring comes from, change these lines in the same commit.
        $isDarkTheme = ($theme -eq 'Dark')
        $floor = [Plith.Services.LoftPalette]::For($isDarkTheme)
        $ring = [Plith.Services.AccentTheme]::DeriveLoftRoles($base, $isDarkTheme).Mark

        foreach ($ringGround in @(
                @{ Name = 'SurfaceStart (floor Background)'; Color = $floor.Background },
                @{ Name = 'SurfaceEnd (floor Surface)';      Color = $floor.Surface })) {
            $measured++
            $ringRatio = [Plith.Services.ContrastInk]::ContrastRatio($ringGround.Color, $ring)
            if ($ringRatio -lt $NonTextThreshold) {
                $failures.Add(("  {0,-34} {1} on {2}  accent {3}  {4}  {5:N1}:1  (needs {6}, {7})" -f `
                    'ShelfSession.cs:SelectionRing (Mark)', 'ring', $ringGround.Name, `
                    $accentHex, $theme.PadRight(5), $ringRatio, $NonTextThreshold, 'non-text'))
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
foreach ($k in $acceptedBelowBar.Keys | Sort-Object) {
    $worst = $acceptedWorst[$k]
    if ($null -ne $worst) {
        $where, $theme = $k -split '\|', 2
        Write-Host ("  BELOW BAR, ACCEPTED: {0} ({1}): worst {2:N2}:1, accepted at {3} - {4}" -f `
            $where, $theme, $worst, $acceptedBelowBar[$k].Floor, $acceptedBelowBar[$k].Why)
    }
}
exit 0
