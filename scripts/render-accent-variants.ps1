# scripts/render-accent-variants.ps1 — renders the light-theme volume HUD at several strengths of
# accent damping, so "still too bright" can be answered by looking rather than by another guess.
#
# The two dials are independent and it matters which one is wrong:
#
#   saturation — how loud the colour is. A fully saturated hue on a pale panel glares even when
#                its luminance is modest.
#   contrast   — how far its luminance sits from the panel. Raising this darkens the bar; it is
#                what makes the bar legible, and past a point it also makes it sombre.
#
# Each variant below moves one or both. Nothing here is wired into the product: the chosen pair
# becomes the two constants in AccentTheme.
#
# Run with: pwsh -STA -File scripts/render-accent-variants.ps1

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-accent-variants",
    [string]$Configuration = 'Debug',
    [string]$Accent = '#CAFF33'
)

$ErrorActionPreference = 'Stop'
if ([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {
    throw "Must run under -STA. Use: pwsh -STA -File $PSCommandPath"
}

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'Plith.dll'))
[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $c = Join-Path $bin "$((($e.Name -split ',')[0])).dll"
    if (Test-Path $c) { [Reflection.Assembly]::LoadFrom($c) } else { $null }
})
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if ($null -eq [Windows.Application]::Current) { $app = [Windows.Application]::new() }
else { $app = [Windows.Application]::Current }

$paletteSources = @('Resources/Theme.xaml', 'Resources/Palette.Light.xaml',
                    'Resources/OsdPalette.Light.xaml', 'Resources/PlithIcons.xaml')
foreach ($rel in $paletteSources) {
    $d = [Windows.ResourceDictionary]::new()
    $d.psbase.Source = [Uri]::new("pack://application:,,,/Plith;component/$rel", [UriKind]::Absolute)
    $app.Resources.MergedDictionaries.Add($d)
}

$base = [Plith.Services.AccentTheme]::ParseHexColor($Accent, [Windows.Media.Colors]::Gray)
$hsl = [Plith.Services.AccentTheme]::RgbToHsl($base)
$surfaces = [Plith.Services.AccentTheme]::DeriveOsdSurfaces($base, $false)

# Damp saturation, then darken until the bar clears the requested contrast against its panel.
function Get-Accent([double]$SatCap, [double]$MinContrast) {
    $h = $hsl.Item1
    $s = [Math]::Min($hsl.Item2, $SatCap)
    $l = [Math]::Min($hsl.Item3, 0.42)
    while ($l -gt 0.10) {
        $c = [Plith.Services.AccentTheme]::HslToRgb($h, $s, $l)
        if ([Plith.Services.ContrastInk]::ContrastRatio($c, $surfaces.SurfaceEnd) -ge $MinContrast) { break }
        $l -= 0.02
    }
    [Plith.Services.AccentTheme]::HslToRgb($h, $s, $l)
}

$variants = @(
    @{ Name = 'a-simdiki';     Sat = 0.80; Contrast = 3.0; Label = 'SIMDIKI  doygunluk 0.80, kontrast 3.0' }
    @{ Name = 'b-daha-mat';    Sat = 0.60; Contrast = 3.0; Label = 'daha mat  doygunluk 0.60, kontrast 3.0' }
    @{ Name = 'c-daha-koyu';   Sat = 0.80; Contrast = 4.5; Label = 'daha koyu  doygunluk 0.80, kontrast 4.5' }
    @{ Name = 'd-mat-ve-koyu'; Sat = 0.55; Contrast = 4.5; Label = 'mat + koyu  doygunluk 0.55, kontrast 4.5' }
    @{ Name = 'e-en-sakin';    Sat = 0.45; Contrast = 5.5; Label = 'en sakin  doygunluk 0.45, kontrast 5.5' }
)

$vm = [Plith.ViewModels.AudioCardViewModel]::new()
$vm.Label = 'Logitech G733'
$vm.BusLine = 'Voicemeeter | A1'
$vm.GainNormalized = 0.62
$vm.GainText = '62%'

$mediaVm = [Plith.ViewModels.MediaViewModel]::new()

foreach ($v in $variants) {
    $accent = Get-Accent $v.Sat $v.Contrast
    $ratio = [Plith.Services.ContrastInk]::ContrastRatio($accent, $surfaces.SurfaceEnd)

    # Swap the accent the view model resolves, then rebuild its brushes so the bar picks it up.
    # The REAL override the app builds - surface, ink, track, everything - with only the accent
    # swapped for the variant. The first version of this script published the accent alone, so the
    # renders came back wearing the dark theme's track and ink: a picture of something that does
    # not exist, offered as evidence. Worse than no picture.
    $b = [Windows.Media.SolidColorBrush]::new($accent); $b.Freeze()
    $over = [Plith.Services.ThemeService]::BuildAccentOverride($base, $false)
    $over['OsdAccent'] = $b
    $over['Accent'] = $b
    $app.Resources.MergedDictionaries.Add($over)
    $vm.RefreshThresholdBrushes()

    $hud = [Plith.Views.Widgets.NotchHud]::new($vm, $mediaVm, $null)
    foreach ($rel in $paletteSources) {
        $d = [Windows.ResourceDictionary]::new()
        $d.psbase.Source = [Uri]::new("pack://application:,,,/Plith;component/$rel", [UriKind]::Absolute)
        $hud.Resources.MergedDictionaries.Add($d)
    }
    $hud.Resources.MergedDictionaries.Add($over)
    $hud.Show([Plith.Views.Widgets.NotchHudKind]::Volume)

    $host_ = [Windows.Controls.Border]::new()
    $g = [Windows.Media.LinearGradientBrush]::new()
    $g.StartPoint = [Windows.Point]::new(0, 0); $g.EndPoint = [Windows.Point]::new(0, 1)
    $g.GradientStops.Add([Windows.Media.GradientStop]::new($surfaces.SurfaceStart, 0))
    $g.GradientStops.Add([Windows.Media.GradientStop]::new($surfaces.SurfaceEnd, 1))
    $host_.Background = $g
    $host_.Width = 300; $host_.Height = 46
    $host_.Child = $hud
    $host_.Measure([Windows.Size]::new(300, 46))
    $host_.Arrange([Windows.Rect]::new(0, 0, 300, 46))
    $host_.UpdateLayout()

    $rtb = [Windows.Media.Imaging.RenderTargetBitmap]::new(900, 138, 288, 288, [Windows.Media.PixelFormats]::Pbgra32)
    $rtb.Render($host_)
    $enc = [Windows.Media.Imaging.PngBitmapEncoder]::new()
    $enc.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($rtb))
    $path = Join-Path $OutDir "$($v.Name).png"
    $fs = [IO.File]::Create($path); try { $enc.Save($fs) } finally { $fs.Dispose() }

    "{0,-42} {1}  {2:N1}:1" -f $v.Label, ('#{0:X2}{1:X2}{2:X2}' -f $accent.R, $accent.G, $accent.B), $ratio

    $app.Resources.MergedDictionaries.Remove($over)
}

"Rendered to $OutDir"
