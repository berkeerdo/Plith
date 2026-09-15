# scripts/render-widgets.ps1 — renders each notch widget and the classic card to PNG, offscreen.
#
# Why this exists: every design defect on this branch was found by a person looking at a
# screenshot and reporting it, one per round. The OSD renders in a layered window that ordinary
# capture misses, and the test suite is not STA so it cannot construct a UserControl at all — so
# there was no way to SEE the result without running the app and asking someone.
#
# This closes that loop. It constructs each widget at the exact size the frame gives it, renders
# it with RenderTargetBitmap, and writes a PNG. No app launch, no input, no screen capture.
#
# Run with: pwsh -STA -File scripts/render-widgets.ps1
#   (STA is mandatory — WPF cannot create a visual on an MTA thread.)

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-render",
    [string]$Configuration = 'Debug'
)

$ErrorActionPreference = 'Stop'

if ([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {
    throw "Must run under -STA. Use: pwsh -STA -File $PSCommandPath"
}

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
$dll = Join-Path $bin 'Plith.dll'
if (-not (Test-Path $dll)) { throw "Build $Configuration first — $dll not found." }

Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase
# Resolve Plith's own dependencies out of its output folder rather than the script's directory.
$null = [Reflection.Assembly]::LoadFrom($dll)
[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $name = ($e.Name -split ',')[0]
    $candidate = Join-Path $bin "$name.dll"
    if (Test-Path $candidate) { [Reflection.Assembly]::LoadFrom($candidate) } else { $null }
})

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# An Application is needed so pack:// URIs and Application.Resources resolve; the widgets reach
# their palette through DynamicResource, which finds nothing without this.
if ($null -eq [Windows.Application]::Current) { $app = [Windows.Application]::new() }
else { $app = [Windows.Application]::Current }

# The dictionaries go on the HOST ELEMENT rather than on Application.Resources.
#
# DynamicResource walks up the element tree before it reaches the application, so this is the
# shorter and more certain path - and it sidesteps the fact that a ResourceDictionary reached
# through Application.Current in PowerShell does not behave like the dictionary it wraps.
$paletteSources = @(
    'Resources/Theme.xaml', 'Resources/Palette.Dark.xaml',
    'Resources/OsdPalette.Dark.xaml', 'Resources/PlithIcons.xaml'
)

# The classic cards resolve StaticResource at CONSTRUCTION, which searches the application
# rather than a tree the control is not in yet. The widget pages get away with DynamicResource
# and the host element; these do not.
foreach ($rel in $paletteSources) {
    $d = [Windows.ResourceDictionary]::new()
    $d.psbase.Source = [Uri]::new("pack://application:,,,/Plith;component/$rel", [UriKind]::Absolute)
    $app.Resources.MergedDictionaries.Add($d)
}

function Add-Palette([Windows.FrameworkElement]$Element) {
    # Added to the element's OWN collection rather than replacing its Resources wholesale:
    # assigning the property from PowerShell hands over an object the tree does not then search.
    foreach ($rel in $paletteSources) {
        $d = [Windows.ResourceDictionary]::new()
        # psbase, not $d.Source. ResourceDictionary implements IDictionary, and PowerShell
        # resolves member access against the dictionary before the CLR property - so the plain
        # assignment silently ADDS AN ENTRY KEYED "Source" instead of loading the file, and the
        # dictionary comes back with exactly one useless key. Nothing throws; everything
        # afterwards renders in WPF's defaults, which is black text on a black ground.
        $d.psbase.Source = [Uri]::new("pack://application:,,,/Plith;component/$rel", [UriKind]::Absolute)
        $Element.Resources.MergedDictionaries.Add($d)
    }
}

# Proven before anything is drawn: an unresolved resource renders as WPF's default, which is
# black text on a black ground - indistinguishable from a design fault.
$probe = [Windows.Controls.Border]::new()
Add-Palette $probe
foreach ($k in 'NotchInk','NotchTrack','OsdGainGreen','OsdTextPrimary','OsdUiFont') {
    $v = $probe.TryFindResource($k)
    if ($null -eq $v) { throw "Resource '$k' did not resolve - the render would be meaningless." }
}
"  palette resolved"

function Save-Visual {
    # [double] on a PowerShell param is not enough: a caller passing 356.0 through a variable
    # can still arrive as something the arithmetic below turns into 0, and RenderTargetBitmap
    # rejects a zero width with an exception rather than a blank image. Bound explicitly.
    param([Windows.FrameworkElement]$Element,
          [ValidateRange(1, 4096)][double]$W,
          [ValidateRange(1, 4096)][double]$H,
          [string]$Name,
          [string]$Background = '#06070A')

    # The notch's own ground behind it, so the PNG shows what a person sees rather than the
    # element floating on transparency.
    $host_ = [Windows.Controls.Border]::new()
    Add-Palette $host_
    $host_.Background = [Windows.Media.BrushConverter]::new().ConvertFromString($Background)
    $host_.Width = $W; $host_.Height = $H
    $host_.Child = $Element

    $host_.Measure([Windows.Size]::new($W, $H))
    $host_.Arrange([Windows.Rect]::new(0, 0, $W, $H))
    $host_.UpdateLayout()

    # 2x, so stroke weights and type can be judged rather than guessed at.
    $scale = 2.0
    $rtb = [Windows.Media.Imaging.RenderTargetBitmap]::new(
        [int]($W * $scale), [int]($H * $scale), 96 * $scale, 96 * $scale,
        [Windows.Media.PixelFormats]::Pbgra32)
    $rtb.Render($host_)

    $enc = [Windows.Media.Imaging.PngBitmapEncoder]::new()
    $enc.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($rtb))
    $path = Join-Path $OutDir "$Name.png"
    $fs = [IO.File]::Create($path)
    try { $enc.Save($fs) } finally { $fs.Dispose() }
    "  $Name.png  ($W x $H)"
}

# --- the frame's own size, and stand-in data for each page --------------------------------
$frameW = 356.0
$frameH = 116.0

$audioVm = [Plith.ViewModels.AudioCardViewModel]::new()
$audioVm.Label = 'Logitech G733'
$audioVm.BusLine = 'Voicemeeter | A1'
$audioVm.GainNormalized = 0.62
$audioVm.GainText = '62%'

$mediaVm = [Plith.ViewModels.MediaViewModel]::new()
$mediaVm.Title = 'You Feel Be Love'
$mediaVm.Artist = 'Denis Phenomen'
$mediaVm.IsPlaying = $true
$mediaVm.HasSession = $true

# A stand-in cover, so the backdrop has something to blur. Drawn rather than loaded: the harness
# must not depend on a file that happens to be on this machine.
$coverVisual = [Windows.Media.DrawingVisual]::new()
$dc = $coverVisual.RenderOpen()
$cg = [Windows.Media.LinearGradientBrush]::new()
$cg.StartPoint = [Windows.Point]::new(0,0); $cg.EndPoint = [Windows.Point]::new(1,1)
$cg.GradientStops.Add([Windows.Media.GradientStop]::new([Windows.Media.Colors]::DarkOrange, 0))
$cg.GradientStops.Add([Windows.Media.GradientStop]::new([Windows.Media.Colors]::MediumVioletRed, 0.55))
$cg.GradientStops.Add([Windows.Media.GradientStop]::new([Windows.Media.Colors]::MidnightBlue, 1))
$dc.DrawRectangle($cg, $null, [Windows.Rect]::new(0,0,300,300))
$dc.DrawEllipse([Windows.Media.Brushes]::White, $null, [Windows.Point]::new(150,150), 54, 54)
$dc.Close()
$cover = [Windows.Media.Imaging.RenderTargetBitmap]::new(300,300,96,96,[Windows.Media.PixelFormats]::Pbgra32)
$cover.Render($coverVisual)
$mediaVm.AlbumArt = $cover

$reader = [Func[Nullable[Plith.Services.WeatherSnapshot]]] {
    [Plith.Services.WeatherSnapshot]::new(19.0, 1, [DateTimeOffset]::Now)
}
$readDate = [Func[Nullable[DateOnly]]] { [DateOnly]::FromDateTime([DateTime]::Now) }  # not a first look
$writeDate = [Action[DateOnly]] { param($d) }

# A muted microphone, so the mark is in shot. The real client needs a capture endpoint; this
# harness only needs the shape of the answer.
$micReader = [Func[Plith.Services.MicrophoneSnapshot]] {
    [Plith.Services.MicrophoneSnapshot]::new('Headset Microphone', $true, 0.8)
}

"Rendering to $OutDir"

$clock = [Plith.Views.Widgets.ClockWidget]::new($mediaVm, $reader, $micReader)
Save-Visual -Element $clock -W $frameW -H $frameH -Name 'widget-clock'

$media = [Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null)
Save-Visual -Element $media -W $frameW -H $frameH -Name 'widget-media'

$writer = [Func[double, bool]] { param($v) $true }
$audio = [Plith.Views.Widgets.AudioWidget]::new($audioVm, $writer)
Save-Visual -Element $audio -W $frameW -H $frameH -Name 'widget-audio'

$weather = [Plith.Views.Widgets.WeatherWidget]::new($reader, $readDate, $writeDate, $null)
Save-Visual -Element $weather -W $frameW -H $frameH -Name 'widget-weather'

# --- the whole frame, so the page dots are actually in shot --------------------------------
# Rendering a page alone shows the page and nothing of the chrome around it, which is how a
# clipped dots lane went unnoticed: the pages looked fine on their own.
$frame = [Plith.Views.Widgets.WidgetFrame]::new()
$pager = [Plith.Services.NotchPager]::new(3)
$pages = [System.Collections.Generic.List[Windows.FrameworkElement]]::new()
$pages.Add([Plith.Views.Widgets.ClockWidget]::new($mediaVm, $reader, $micReader))
$pages.Add([Plith.Views.Widgets.WeatherWidget]::new($reader, $readDate, $writeDate, $null))
$pages.Add([Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null))
$frame.SetPages($pager, $pages)
$frame.SyncToPager(0)
Save-Visual -Element $frame -W $frameW -H $frameH -Name 'frame-page1'

$frame2 = [Plith.Views.Widgets.WidgetFrame]::new()
$pager2 = [Plith.Services.NotchPager]::new(3)
$pages2 = [System.Collections.Generic.List[Windows.FrameworkElement]]::new()
$pages2.Add([Plith.Views.Widgets.ClockWidget]::new($mediaVm, $reader, $micReader))
$pages2.Add([Plith.Views.Widgets.WeatherWidget]::new($reader, $readDate, $writeDate, $null))
$pages2.Add([Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null))
$frame2.SetPages($pager2, $pages2)
$pager2.GoTo(1)
$frame2.SyncToPager(0)
Save-Visual -Element $frame2 -W $frameW -H $frameH -Name 'frame-weather'

# --- the event HUDs, which are a different shape family ------------------------------------
$hud = [Plith.Views.Widgets.NotchHud]::new($audioVm, $mediaVm, $null)

$hud.Show([Plith.Views.Widgets.NotchHudKind]::Volume)
Save-Visual -Element $hud -W 300.0 -H 46.0 -Name 'hud-volume'

# Re-hosted rather than reused in place: Save-Visual parents the element to a fresh Border, and
# an element cannot have two parents.
$hud2 = [Plith.Views.Widgets.NotchHud]::new($audioVm, $mediaVm, $null)
$hud2.Show([Plith.Views.Widgets.NotchHudKind]::Media)
Save-Visual -Element $hud2 -W 372.0 -H 54.0 -Name 'hud-media'

# --- the classic card ----------------------------------------------------------------------
# Rendered on its own ground rather than the notch's bezel: it is a floating card, and judging
# it against black would flatter a border that has to work over a desktop.
$audioCard = [Plith.Views.AudioCardView]::new()
$audioCard.DataContext = $audioVm
Save-Visual -Element $audioCard -W 382.0 -H 56.0 -Name 'card-audio' -Background '#151A21'

$mediaCard = [Plith.Views.MediaCardView]::new()
$mediaCard.DataContext = $mediaVm
Save-Visual -Element $mediaCard -W 382.0 -H 56.0 -Name 'card-media' -Background '#151A21'

# --- the cloud silhouette on its own ------------------------------------------------------
# The sky builds its clouds only once the page is visible, which an offscreen render never is -
# so the gradient shows and the weather does not. Drawing the shape directly is the only way to
# judge the thing that was actually wrong with it.
$cloudHost = [Windows.Controls.Canvas]::new()
foreach ($spec in @(@{w=170; h=44; t=6; a=40}, @{w=104; h=30; t=30; a=70})) {
    $path = [Windows.Shapes.Path]::new()
    $path.Data = $probe.TryFindResource('ShapeCloud')
    $path.Stretch = 'Fill'
    $path.Width = $spec.w
    $path.Height = $spec.h
    $g = [Windows.Media.LinearGradientBrush]::new()
    $g.StartPoint = [Windows.Point]::new(0.35, 0)
    $g.EndPoint = [Windows.Point]::new(0.6, 1)
    $g.GradientStops.Add([Windows.Media.GradientStop]::new([Windows.Media.Color]::FromArgb($spec.a, 255,255,255), 0))
    $g.GradientStops.Add([Windows.Media.GradientStop]::new([Windows.Media.Color]::FromArgb([byte]($spec.a*0.72), 255,255,255), 0.55))
    $g.GradientStops.Add([Windows.Media.GradientStop]::new([Windows.Media.Color]::FromArgb([byte]($spec.a*0.18), 255,255,255), 1))
    $path.Fill = $g
    [Windows.Controls.Canvas]::SetLeft($path, 40)
    [Windows.Controls.Canvas]::SetTop($path, $spec.t)
    $cloudHost.Children.Add($path)
}
Save-Visual -Element $cloudHost -W $frameW -H $frameH -Name 'cloud-shape' -Background '#FF3E7BA8'

"Done."
