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
    [string]$Configuration = 'Debug',

    # The theme to render in. Light exists because the OSD's ink was a constant chosen for a dark
    # panel, and on the light theme the accent-tinted surface goes pale - white on pale pink,
    # measured at 1.2:1. Nothing in this harness could show that while it only rendered dark.
    [ValidateSet('Dark', 'Light')]
    [string]$Theme = 'Dark',

    # The accent to tint with, as the user would pick it. Empty renders the untinted palette.
    # Tinting matters more than it sounds: the running app paints the OSD on a gradient derived
    # from this colour, so a render on a flat ground is a render of something that never ships.
    [string]$Accent = '#A3E635'
)

$ErrorActionPreference = 'Stop'

if ([Threading.Thread]::CurrentThread.GetApartmentState() -ne 'STA') {
    throw "Must run under -STA. Use: pwsh -STA -File $PSCommandPath"
}

# The session has to be Active, and this is not a copy of drive-media-page.ps1's check for
# tidiness: it was MEASURED here on 2026-09-21. In a DISCONNECTED session this harness runs to
# completion, prints "widget-media.png  (356 x 116)" for every render, and writes PNGs that are
# 769 bytes of pure transparency. Every pixel comes back A=0.
#
# That is worse than failing, because the output looks like output. The only thing that caught it
# was the shelf's own empty-outline check throwing with "left=0, right=0, top=0, bottom=0", which
# reads as a design defect in a dashed border rather than as an empty image, and by then fifteen
# renders had already been reported as done.
#
# docs/SHELF-VERIFICATION.md section 7.5 records the related fact for screen CAPTURE: what breaks
# it is the session being disconnected or locked. This adds RenderTargetBitmap to that list, which
# is not obvious: offscreen rendering has no window and no desktop surface to capture, and it
# still produces nothing.
$session = qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }
if ($session -notmatch 'Active') {
    throw ("This session is not Active (`qwinsta` says: $($session -replace '\s+', ' ')). " +
           "RenderTargetBitmap produces FULLY TRANSPARENT images in a disconnected or locked " +
           "session while reporting every render as done, so the PNGs would be 769 bytes of " +
           "nothing and the run would look successful. Reconnect and re-run.")
}

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
$dll = Join-Path $bin 'Plith.dll'
if (-not (Test-Path $dll)) { throw "Build $Configuration first — $dll not found." }

# The catcher's own assembly, loaded the same way and for the same reason: ShelfSurface lives
# there, not in Plith, because the catcher is a second process that must not reference Plith.dll
# (see Plith.DropCatcher.csproj). The harness is the one place that is allowed to load both, since
# it is neither process: it is only looking at what each one would draw.
$dcBin = Join-Path $root "src\Plith.DropCatcher\bin\$Configuration\net10.0-windows10.0.22000.0"
$dcDll = Join-Path $dcBin 'Plith.DropCatcher.dll'
if (-not (Test-Path $dcDll)) { throw "Build $Configuration first: $dcDll not found." }

Add-Type -AssemblyName PresentationCore, PresentationFramework, WindowsBase
# Resolve Plith's own dependencies out of its output folder rather than the script's directory.
$null = [Reflection.Assembly]::LoadFrom($dll)
$dcAssembly = [Reflection.Assembly]::LoadFrom($dcDll)
[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $name = ($e.Name -split ',')[0]
    foreach ($candidateBin in @($bin, $dcBin)) {
        $candidate = Join-Path $candidateBin "$name.dll"
        if (Test-Path $candidate) { return [Reflection.Assembly]::LoadFrom($candidate) }
    }
    $null
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
    'Resources/Theme.xaml', "Resources/Palette.$Theme.xaml",
    "Resources/OsdPalette.$Theme.xaml", 'Resources/PlithIcons.xaml'
)

# The accent override, built by the SAME method the running app calls.
#
# Not recomputed here. A harness that derives its own colours drifts from the app the first time
# either changes, and this branch has already paid for that once: every colour judged from these
# renders was judged against a flat ground the product does not have.
$accentOverride = $null
if ($Accent) {
    $baseColor = [Plith.Services.AccentTheme]::ParseHexColor($Accent, [Windows.Media.Colors]::Gray)
    $accentOverride = [Plith.Services.ThemeService]::BuildAccentOverride($baseColor, ($Theme -eq 'Dark'))
}

# The ground the OSD actually sits on, so a render shows the contrast a person gets.
$osdSurfaces = if ($Accent) {
    [Plith.Services.AccentTheme]::DeriveOsdSurfaces($baseColor, ($Theme -eq 'Dark'))
} else { $null }

# The classic cards resolve StaticResource at CONSTRUCTION, which searches the application
# rather than a tree the control is not in yet. The widget pages get away with DynamicResource
# and the host element; these do not.
foreach ($rel in $paletteSources) {
    $d = [Windows.ResourceDictionary]::new()
    $d.psbase.Source = [Uri]::new("pack://application:,,,/Plith;component/$rel", [UriKind]::Absolute)
    $app.Resources.MergedDictionaries.Add($d)
}

if ($accentOverride) { $app.Resources.MergedDictionaries.Add($accentOverride) }

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

    # Last, so it wins over the palette defaults it is there to override.
    if ($accentOverride) { $Element.Resources.MergedDictionaries.Add($accentOverride) }
}

# Proven before anything is drawn: an unresolved resource renders as WPF's default, which is
# black text on a black ground - indistinguishable from a design fault.
$probe = [Windows.Controls.Border]::new()
Add-Palette $probe
foreach ($k in 'NotchInk','NotchTrack','OsdGainGreen','OsdTextPrimary','OsdUiFont') {
    $v = $probe.TryFindResource($k)
    if ($null -eq $v) { throw "Resource '$k' did not resolve - the render would be meaningless." }
}
"  palette resolved ($Theme theme, accent $(if ($Accent) { $Accent } else { 'none' }))"

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
    # The surface the OSD is actually drawn on, not a flat colour. This is the whole reason the
    # light-theme contrast bug was invisible here for a whole branch.
    if ($osdSurfaces -and -not $PSBoundParameters.ContainsKey('Background')) {
        $g = [Windows.Media.LinearGradientBrush]::new()
        $g.StartPoint = [Windows.Point]::new(0, 0); $g.EndPoint = [Windows.Point]::new(0, 1)
        $g.GradientStops.Add([Windows.Media.GradientStop]::new($osdSurfaces.SurfaceStart, 0))
        $g.GradientStops.Add([Windows.Media.GradientStop]::new($osdSurfaces.SurfaceEnd, 1))
        $host_.Background = $g
    } else {
        $host_.Background = [Windows.Media.BrushConverter]::new().ConvertFromString($Background)
    }
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

    # Detached rather than left parented to $host_, which is about to go out of scope anyway:
    # WPF refuses to give an element a second parent while it still has one, and without this a
    # caller could never pass the SAME element to Save-Visual twice - which a two-pass check
    # (render once, mutate state, render the same control again) needs to do.
    $host_.Child = $null

    "  $Name.png  ($W x $H)"
}

# ShelfSurface fetches a real shell icon off the UI thread and marshals it back with
# Dispatcher.BeginInvoke, on purpose (see ShellIcons.cs): a slow network path must not stall the
# surface. Nothing in this script runs a Dispatcher.Run() message loop, so a BeginInvoke callback
# just sits queued and unrun unless something pumps it - a capture taken right after Render()
# would show only the drawn fallback and never prove the extraction path exists at all. This
# drains the queue in short bursts, sleeping briefly between each so the background Task.Run has
# a chance to finish and enqueue its callback before the next drain.
function Wait-ForDispatcher {
    param([int]$Bursts = 20, [int]$DelayMs = 25)
    for ($i = 0; $i -lt $Bursts; $i++) {
        Start-Sleep -Milliseconds $DelayMs
        $frame = [Windows.Threading.DispatcherFrame]::new()
        $exit = [Action[Windows.Threading.DispatcherFrame]] { param($f) $f.Continue = $false }
        [Windows.Threading.Dispatcher]::CurrentDispatcher.BeginInvoke(
            [Windows.Threading.DispatcherPriority]::Background, $exit, $frame) | Out-Null
        [Windows.Threading.Dispatcher]::PushFrame($frame)
    }
}

# Walks the visual tree under $Root (VisualTreeHelper, not the logical tree: everything this
# harness builds is added through a Children collection, which is visual children straight
# away) and returns every descendant for which $Predicate returns true. Used to reach into
# ShelfSurface's generated tiles from outside the assembly, without needing a private field:
# the tree itself is the only contract this checks against.
function Find-VisualDescendants {
    param([Windows.DependencyObject]$Root, [scriptblock]$Predicate)
    $results = [System.Collections.Generic.List[Windows.DependencyObject]]::new()
    $stack = [System.Collections.Generic.Stack[Windows.DependencyObject]]::new()
    $stack.Push($Root)
    while ($stack.Count -gt 0) {
        $node = $stack.Pop()
        $count = [Windows.Media.VisualTreeHelper]::GetChildrenCount($node)
        for ($i = 0; $i -lt $count; $i++) {
            $child = [Windows.Media.VisualTreeHelper]::GetChild($node, $i)
            if (& $Predicate $child) { $results.Add($child) }
            $stack.Push($child)
        }
    }
    return $results
}

# --- the frame's own size, and stand-in data for each page --------------------------------
# FROM the product, not typed here. These were literal 356 and 116, and when the frame grew to
# 164 this harness kept rendering every page inside the old box: the PNGs came back 356x116 and
# would have had the new layout judged against a frame the product no longer has. The one number
# a render harness must not own is the size of the thing it is rendering.
$frameW = [double][Plith.Views.Presentation.NotchGeometry]::OpenFrameDip.Width
$frameH = [double][Plith.Views.Presentation.NotchGeometry]::OpenFrameDip.Height
"  frame from the product: $frameW x $frameH"

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

# A stand-in cover, so the tile has something to draw. Drawn rather than loaded: the harness must
# not depend on a file that happens to be on this machine. It was described as something for the
# backdrop to blur, and that backdrop is gone: the page no longer paints its own ground.
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

# 2:27 into 4:27, stamped now, so the bar and both clocks are in shot. Seeded on the view model
# rather than through a snapshot: the harness has no SMTC session and does not need one.
$mediaVm.Timeline = [Plith.Services.MediaTimeline]::new(
    [TimeSpan]::FromSeconds(147), [TimeSpan]::FromSeconds(267), [DateTimeOffset]::Now)

# A source that accepts a position write, which is what Spotify reports here. The paused and
# long-title states below leave it false on purpose, so both halves are drawn: an enabled track
# and one that reports a position while offering no gesture. The thumb only appears under the
# pointer, so no still frame can show it either way.
$mediaVm.CanSeek = $true

# Two days AFTER today, which is what the page draws: Open-Meteo's own first day is today and
# the page drops it, because today is already the big number on the left.
$days = [System.Collections.Generic.List[Plith.Services.WeatherDay]]::new()
$days.Add([Plith.Services.WeatherDay]::new([DateOnly]::FromDateTime([DateTime]::Now), 1, 21.0, 14.0))
$days.Add([Plith.Services.WeatherDay]::new([DateOnly]::FromDateTime([DateTime]::Now.AddDays(1)), 61, 18.0, 12.0))
$days.Add([Plith.Services.WeatherDay]::new([DateOnly]::FromDateTime([DateTime]::Now.AddDays(2)), 3, 23.0, 15.0))

$reader = [Func[Nullable[Plith.Services.WeatherSnapshot]]] {
    [Plith.Services.WeatherSnapshot]::new(19.0, 1, [DateTimeOffset]::Now, $days)
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

# The clock page with RAIN, which no render ever drew: its fixture has always been a clear sky, so
# the one case where its 26 DIP mark has to distinguish a cloud from rain was never looked at. It
# turned out to be the same defect the forecast columns had, found there first only because those
# were new.
$rainReader = [Func[Nullable[Plith.Services.WeatherSnapshot]]] {
    [Plith.Services.WeatherSnapshot]::new(14.0, 61, [DateTimeOffset]::Now, $days)
}
# EVERY sky kind, side by side, at the size the pages actually draw the mark.
#
# Asked directly: are all of the conditions' icons and animations right? Nothing could answer
# that, because the only kind ever rendered was Clear. Rain turned out to be a blob at this size
# and had been one for as long as the mark has existed; the others had never been looked at at
# all. One strip, five marks, 26 DIP each, on the notch's own ground.
#
# The ANIMATIONS are not in this: a still frame cannot show motion, and a mark's motion only
# starts when it is visible. What this proves is the shapes.
$kindStrip = [Windows.Controls.StackPanel]::new()
$kindStrip.Orientation = 'Horizontal'
$kindStrip.HorizontalAlignment = 'Center'
$kindStrip.VerticalAlignment = 'Center'
Add-Palette $kindStrip
foreach ($kindName in 'Clear', 'Overcast', 'Rain', 'Snow', 'Night') {
    $cell = [Windows.Controls.StackPanel]::new()
    $cell.Margin = [Windows.Thickness]::new(14, 0, 14, 0)

    $label = [Windows.Controls.TextBlock]::new()
    $label.Text = $kindName
    $label.FontSize = 10
    $label.HorizontalAlignment = 'Center'
    $label.Foreground = $kindStrip.TryFindResource('NotchInkMuted')
    $cell.Children.Add($label) | Out-Null

    $m = [Plith.Views.Widgets.WeatherMark]::new()
    $m.Width = 26; $m.Height = 26
    $m.Margin = [Windows.Thickness]::new(0, 4, 0, 0)
    $m.HorizontalAlignment = 'Center'
    $m.Show([Plith.Services.SkyKind]::$kindName)
    $cell.Children.Add($m) | Out-Null

    $kindStrip.Children.Add($cell) | Out-Null
}
Wait-ForDispatcher
Save-Visual -Element $kindStrip -W $frameW -H 70 -Name 'weather-marks'

$clockRain = [Plith.Views.Widgets.ClockWidget]::new($mediaVm, $rainReader, $micReader)
Save-Visual -Element $clockRain -W $frameW -H $frameH -Name 'widget-clock-rain'

$media = [Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null)
Save-Visual -Element $media -W $frameW -H $frameH -Name 'widget-media'

# Three more states, because the happy case is not where this layout's risk is. A title long
# enough to scroll against the 116 DIP text column, a paused track, and the page with no session
# at all: the last is the one nobody looks at until it is wrong.
$longVm = [Plith.ViewModels.MediaViewModel]::new()
$longVm.Title = 'Everything In Its Right Place (Remastered 2026 Edition)'
$longVm.Artist = 'A Band With A Fairly Long Name Too'
$longVm.IsPlaying = $true
$longVm.HasSession = $true
$longVm.AlbumArt = $cover
# No timeline on purpose: this is the live-stream case, where the progress row collapses and the
# title has the full band to scroll in.
$mediaLong = [Plith.Views.Widgets.MediaWidget]::new($longVm, $null)
Save-Visual -Element $mediaLong -W $frameW -H $frameH -Name 'widget-media-long-title'

$pausedVm = [Plith.ViewModels.MediaViewModel]::new()
$pausedVm.Title = 'You Feel Be Love'
$pausedVm.Artist = 'Denis Phenomen'
$pausedVm.IsPlaying = $false
$pausedVm.HasSession = $true
$pausedVm.AlbumArt = $cover
$pausedVm.Timeline = [Plith.Services.MediaTimeline]::new(
    [TimeSpan]::FromSeconds(12), [TimeSpan]::FromSeconds(267), [DateTimeOffset]::Now)
$mediaPaused = [Plith.Views.Widgets.MediaWidget]::new($pausedVm, $null)
Save-Visual -Element $mediaPaused -W $frameW -H $frameH -Name 'widget-media-paused'

$emptyVm = [Plith.ViewModels.MediaViewModel]::new()
$mediaEmpty = [Plith.Views.Widgets.MediaWidget]::new($emptyVm, $null)
Save-Visual -Element $mediaEmpty -W $frameW -H $frameH -Name 'widget-media-empty'

# The output picker, in the mode the page turns into. Two fixtures: this machine's own five-device
# shape, and a seven-device one so the overflow door is in shot. Injected rather than read from the
# machine, because a harness that draws whatever is plugged in today draws something different
# tomorrow. The device names are the ones measured here on 2026-09-21, including the two Steam
# entries that AudioLabel.Shorten used to collapse into one string.
$five = [System.Collections.Generic.List[Plith.Services.WindowsAudioEndpointInfo]]::new()
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('g733', 'Hoparlor (Logitech G733 Gaming Headset)', 'Logitech G733 Gaming Headset'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('realtek', 'Hoparlor (Realtek(R) Audio)', 'Realtek(R) Audio'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('nvidia', 'PG27AQDM (NVIDIA High Definition Audio)', 'NVIDIA High Definition Audio'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('steam-spk', 'Hoparlor (Steam Streaming Speakers)', 'Steam Streaming Speakers'))
$five.Add([Plith.Services.WindowsAudioEndpointInfo]::new('steam-mic', 'Hoparlor (Steam Streaming Microphone)', 'Steam Streaming Microphone'))

$pickerFive = [Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null, $five, 'g733')
$pickerFive.OpenPicker()
Save-Visual -Element $pickerFive -W $frameW -H $frameH -Name 'widget-media-picker'

$seven = [System.Collections.Generic.List[Plith.Services.WindowsAudioEndpointInfo]]::new($five)
$seven.Add([Plith.Services.WindowsAudioEndpointInfo]::new('sixth', 'Speakers (Sixth Device)', 'Sixth Device'))
$seven.Add([Plith.Services.WindowsAudioEndpointInfo]::new('seventh', 'Speakers (Seventh Device)', 'Seventh Device'))
$pickerSeven = [Plith.Views.Widgets.MediaWidget]::new($mediaVm, $null, $seven, 'g733')
$pickerSeven.OpenPicker()
Save-Visual -Element $pickerSeven -W $frameW -H $frameH -Name 'widget-media-picker-overflow'

$writer = [Func[double, bool]] { param($v) $true }
$audio = [Plith.Views.Widgets.AudioWidget]::new($audioVm, $writer)
Save-Visual -Element $audio -W $frameW -H $frameH -Name 'widget-audio'

# A typed city, so the new place line is in shot. Empty would hide it, which is the other state
# and the one the unavailable render already covers.
$place = [Func[string]] { 'Istanbul' }
$weather = [Plith.Views.Widgets.WeatherWidget]::new($reader, $readDate, $writeDate, $null, $place)
Save-Visual -Element $weather -W $frameW -H $frameH -Name 'widget-weather'

# The shelf, against a store in a temp directory with real files in it. Real files rather than a
# stub, because ShelfStore refuses to hold a path it cannot stat - that check is the point of it,
# and a fake store would render a page the product cannot produce.
$shelfDir = Join-Path $OutDir 'shelf-fixture'
New-Item -ItemType Directory -Force -Path $shelfDir | Out-Null
$fixtures = @(
    'Quarterly report FINAL v3.pdf',
    'screenshot.png',
    'notes.md',
    'invoice-2026-09.xlsx',
    'archive.zip',
    'one-more.txt',
    'and-another.txt'
)
foreach ($f in $fixtures) { Set-Content -LiteralPath (Join-Path $shelfDir $f) -Value 'x' }
$shelfFolder = Join-Path $shelfDir 'Project assets'
New-Item -ItemType Directory -Force -Path $shelfFolder | Out-Null

$storeFile = Join-Path $OutDir 'shelf-store.txt'
Remove-Item -LiteralPath $storeFile -ErrorAction SilentlyContinue
$store = [Plith.Services.Shelf.ShelfStore]::new($storeFile)
$paths = [string[]](@($shelfFolder) + ($fixtures | ForEach-Object { Join-Path $shelfDir $_ }))
$store.Add($paths)
$shelf = [Plith.Views.Widgets.ShelfWidget]::new($store)
Save-Visual -Element $shelf -W $frameW -H $frameH -Name 'widget-shelf'

# The EMPTY state, which nothing had ever rendered.
#
# It is not the same page with fewer tiles: it swaps the row for a sentence and hides the hint
# line under it, so it has its own layout and its own way of overflowing an 82 DIP content box.
# A second store on a path with no file behind it, because ShelfStore loads from disk in its
# constructor and reusing the one above would come back full.
$emptyStore = [Plith.Services.Shelf.ShelfStore]::new((Join-Path $OutDir 'shelf-store-empty.txt'))
$shelfEmpty = [Plith.Views.Widgets.ShelfWidget]::new($emptyStore)
Save-Visual -Element $shelfEmpty -W $frameW -H $frameH -Name 'widget-shelf-empty'

# And the sentence the page shows when the shelf will not open, which replaces the hint rather
# than the row. Rendered against the FULL store, because that is the only case where the line is
# showing at all, and because a sentence longer than the hint it replaces is exactly the thing
# that would push the page out of shape without anyone noticing.
$shelfUnavailable = [Plith.Views.Widgets.ShelfWidget]::new($store)
$shelfUnavailable.ShowUnavailable('The shelf helper is missing from this install.')
Save-Visual -Element $shelfUnavailable -W $frameW -H $frameH -Name 'widget-shelf-unavailable'


# --- the shelf SURFACE (the catcher's own page), at the same three call sites the widgets use --
#
# ShelfSurface lives in Plith.DropCatcher, not Plith, and takes its palette as a ShelfPalette
# rather than through the resource dictionaries this harness merges for the widgets above. Built
# from the SAME resolved colours those widgets render with, read back off the probe, so this
# render is judged against exactly what the running app would send across the wire, not a second,
# hand-picked palette that could drift from it.
$surfaceBrush = $probe.TryFindResource('OsdSurfaceBrush')
$inkBrush = $probe.TryFindResource('NotchInk')
$inkMutedBrush = $probe.TryFindResource('NotchInkMuted')
$trackBrush = $probe.TryFindResource('NotchTrack')
$accentBrush = $probe.TryFindResource('Accent')

# The selection ring keeps the accent when the accent already works, and only moves when it does
# not: ContrastInk.RingOn returns the accent unchanged once it clears 3:1 against the surface, and
# only walks its lightness (hue and saturation held fixed) when it does not. This replaced an
# earlier attempt at ContrastInk.TrackOn, which is a function of the surface alone and threw the
# accent's hue away every time - it fixed a near-white accent's measured 1.25:1 but also flattened
# a lime accent's measured 8.87:1 down to 3.03:1, a dull grey-olive, for a threshold that colour
# had already cleared. Plith sends the ANSWER across the wire rather than the accent alone, so the
# catcher never repeats the derivation. There is no sender for this yet (that is Task 6's work),
# so the harness derives it here the same way Task 6 will, standing in for what the real message
# will eventually carry.
$selectionRingColor = [Plith.Services.ContrastInk]::RingOn($accentBrush.Color, $surfaceBrush.GradientStops[1].Color)

# ShelfPalette is not resolved as [Plith.Services.Shelf.ShelfPalette]: that name is compiled into
# BOTH Plith.dll and Plith.DropCatcher.dll (ShelfPaletteWire.cs is LINKED into the catcher project
# rather than referenced, precisely so the catcher never depends on Plith's assembly - see that
# project's own comment on it). Two separate compiles of the same source make two separate CLR
# types that merely share a name, and PowerShell's bare type literal binds to whichever assembly
# it saw first - which is Plith's, not the catcher's. ShelfSurface.Apply demands the catcher's own
# type, so the value has to be built through THAT assembly's Type object.
$shelfPaletteType = $dcAssembly.GetType('Plith.Services.Shelf.ShelfPalette')
$shelfPalette = [Activator]::CreateInstance($shelfPaletteType, @(
    $surfaceBrush.GradientStops[0].Color, $surfaceBrush.GradientStops[1].Color,
    $inkBrush.Color, $inkMutedBrush.Color, $trackBrush.Color, $accentBrush.Color,
    $selectionRingColor, [bool]($Theme -eq 'Dark')))

# Seven entries across two stacks: one stack deliberately OVER the visible-rows cap (a folder
# plus four files, so the overflow tile and a folder icon both appear in the same render) and one
# exactly AT it (two files, so the ordinary two-tile column appears too). Reuses the same fixture
# files as widget-shelf above rather than a second stub set - ShelfModel stats nothing itself,
# but the tiles it paints should still carry real file names of a plausible length.
#
# The fixture files above are real (Set-Content wrote them), so ShellIcons can already extract
# something for them, but their extension-based icons look enough like the drawn document
# fallback that a render alone cannot prove a real shell icon is in the picture, or that a
# fallback sits at the same size and baseline beside one when the two are mixed. Stack1 carries
# both halves of that proof instead of two more ordinary fixtures.
#
# The built Plith.exe is the real-icon half: its icon is unmistakably not the drawn geometry.
#
# The fallback half turned out to be harder to earn than expected. SHGFI_USEFILEATTRIBUTES is
# built to answer from the extension alone, and measured directly it does that for nearly
# anything handed to it - an empty string, a 5000-character name, a reserved device name, even
# a well-formed but unreachable \\host\share\file.txt all came back a real icon, because
# Windows still knows what a ".txt" is without ever touching the network. The one path that
# reliably made SHGetFileInfo return nothing at all, checked five times over, is a single
# leading backslash rather than the two a UNC path needs: Windows reads it as a path rooted on
# the current drive (something like "C:\nosuchhost\share\ghost.txt"), and that specific
# malformed shape is what the fallback in this render actually exercises.
$surfaceModel = [Plith.DropCatcher.Shelf.ShelfModel]::new()
$shelfItems = [string[]](@($shelfFolder) +
    ($fixtures[0..3] | ForEach-Object { Join-Path $shelfDir $_ }) +
    @('\nosuchhost\share\ghost.txt', (Join-Path $bin 'Plith.exe')))
$surfaceModel.SetItems($shelfItems)

# One tile selected, so the accent ring (AccentBrush, the one place this control paints the
# accent) is actually in the saved renders. Without this none of the three showed it at all, and
# the white-accent render exists specifically to catch an accent-on-surface contrast defect it
# could not catch on an empty selection. The folder tile, stack0's only real tile beside its
# overflow count, so the ring is visible next to that tile in the same picture.
$surfaceModel.Select($shelfFolder, $false)

# READ from NotchGeometry rather than kept in sync here by hand.
#
# It used to be two literals, 384 x 224, with a comment explaining that they were "kept in sync
# here by hand" because the harness and the control are two separate assemblies. The hand-sync
# broke the moment the shelf went from two rows to three: the frame grew to 283 and this file
# still arranged the control in 224, which does not fail loudly. It renders a picture of a
# surface with its last row cut off, and a cropped render is indistinguishable from a layout
# defect.
#
# The harness already loads Plith.dll, so it can just ask. NotchGeometry.ShelfFrameDip is the
# one place the frame is defined, and its height is now an expression over the row count, so
# this follows a row being added without anyone remembering to come here.
$shelfFrame = [Plith.Views.Presentation.NotchGeometry]::ShelfFrameDip
$shelfSurfaceW = $shelfFrame.Width
$shelfSurfaceH = $shelfFrame.Height

$shelfSurface = [Plith.DropCatcher.Shelf.ShelfSurface]::new()
$shelfSurface.Apply($shelfPalette)
$shelfSurface.Render($surfaceModel)
Wait-ForDispatcher
Save-Visual -Element $shelfSurface -W $shelfSurfaceW -H $shelfSurfaceH -Name 'shelf-surface'

# --- the EMPTY shelf, which nobody had ever rendered -----------------------------------------
#
# The notch page's empty state was rendered from the first day (widget-shelf-empty above); the
# shelf surface's own was not, and it showed. Reported from the running build on 2026-09-20 as
# simply bad. A state nobody looks at is a state nobody designs, so it gets its own render here
# at the size the real window gives it: an empty shelf opens ONE row tall, not three.
$emptyModel = [Plith.DropCatcher.Shelf.ShelfModel]::new()
$emptyModel.SetItems([string[]]@())
$emptySurface = [Plith.DropCatcher.Shelf.ShelfSurface]::new()
$emptySurface.Apply($shelfPalette)
$emptySurface.Render($emptyModel)
Wait-ForDispatcher
$emptyFrame = [Plith.Views.Presentation.NotchGeometry]::ShelfFrameFor(0)
$emptySurface.Measure([Windows.Size]::new($emptyFrame.Width, [double]::PositiveInfinity))
"  empty shelf wants $([Math]::Round($emptySurface.DesiredSize.Height,1)) DIP at $($emptyFrame.Width) wide; the frame gives $($emptyFrame.Height)"
Save-Visual -Element $emptySurface -W $emptyFrame.Width -H $emptyFrame.Height -Name 'shelf-surface-empty'

# The same empty surface at the size a FULL shelf opens at, which is the state reported broken
# from a real session: the frame is chosen once at open by design, so clearing a three-row shelf
# while it is up leaves the window at three rows with an empty page in it. The dashed box used to
# be a fixed two-row height and hung in the top of that window. Rendered at both sizes now,
# because the bug lived in the difference between them.
$clearedFrame = [Plith.Views.Presentation.NotchGeometry]::ShelfFrameDip
$clearedSurface = [Plith.DropCatcher.Shelf.ShelfSurface]::new()
$clearedSurface.Apply($shelfPalette)
$clearedSurface.Render($emptyModel)
Wait-ForDispatcher
Save-Visual -Element $clearedSurface -W $clearedFrame.Width -H $clearedFrame.Height -Name 'shelf-surface-cleared'

# What the empty card's parts actually measure to. A dashed outline that draws its top and bottom
# but not its sides is a thing pixels can suggest and only the tree can settle.
$emptyHost = [Windows.Controls.Border]::new()
$emptyHost.Width = $emptyFrame.Width; $emptyHost.Height = $emptyFrame.Height
$emptyHost.Child = $emptySurface
$emptyHost.Measure([Windows.Size]::new($emptyFrame.Width, $emptyFrame.Height))
$emptyHost.Arrange([Windows.Rect]::new(0, 0, $emptyFrame.Width, $emptyFrame.Height))
$emptyHost.UpdateLayout()
$rects = @(Find-VisualDescendants -Root $emptySurface -Predicate { param($n) $n -is [Windows.Shapes.Rectangle] })
foreach ($r in $rects) {
    $pt = $r.TranslatePoint([Windows.Point]::new(0,0), $emptyHost)
    "  empty outline: $([Math]::Round($r.ActualWidth,1)) x $([Math]::Round($r.ActualHeight,1)) at $([Math]::Round($pt.X,1)),$([Math]::Round($pt.Y,1))  dash=$($r.StrokeDashArray -join ',')  thickness=$($r.StrokeThickness)"
}

# --- the empty card's outline must have ALL FOUR sides -----------------------------------------
#
# It had two. A Rectangle whose stroke straddles its own layout bounds loses the halves that fall
# outside when the parent clips, and on this card the vertical pair went while the horizontal pair
# survived. The element measured 352 x 136 the whole time, so nothing in the tree was wrong to
# read; only the pixels were. Found by counting them, after the render was looked at and the
# missing sides were noticed by eye and then twice mis-measured by probes that landed in the dash
# gaps. Fixed with a 1 DIP inset so the stroke is wholly inside.
#
# This check counts lit pixels in a band over each side, which is the one way the defect is
# visible at all: no build, no test and no other lint could see it.
$emptyPng = Join-Path $OutDir 'shelf-surface-empty.png'
$emptyBmp = [System.Drawing.Bitmap]::FromFile($emptyPng)
try {
    $sc = $emptyBmp.Width / $emptyFrame.Width
    function Measure-Band([double]$x0, [double]$x1, [double]$y0, [double]$y1) {
        $lit = 0
        for ($x = [int]($x0 * $sc); $x -le [int]($x1 * $sc); $x++) {
            for ($y = [int]($y0 * $sc); $y -le [int]($y1 * $sc); $y++) {
                $c = $emptyBmp.GetPixel([Math]::Min($x, $emptyBmp.Width - 1), [Math]::Min($y, $emptyBmp.Height - 1))
                if ($c.R + $c.G + $c.B -gt 130) { $lit++ }
            }
        }
        $lit
    }
    $sides = [ordered]@{
        left   = Measure-Band 14 22 70 180
        right  = Measure-Band 362 370 70 180
        top    = Measure-Band 20 360 55 62
        bottom = Measure-Band 20 360 188 196
    }
    $dark = @($sides.Keys | Where-Object { $sides[$_] -lt 40 })
    if ($dark.Count -gt 0) {
        throw ("empty-outline check FAILED: the dashed box is missing its $($dark -join ' and ') " +
               "side(s). Lit pixels per side: " +
               (($sides.Keys | ForEach-Object { "$_=$($sides[$_])" }) -join ', ') +
               ". A Rectangle stroke that straddles its own bounds loses the clipped halves; " +
               "inset it so the stroke is wholly inside.")
    }
    "  empty-outline check passed: all four sides drawn (" +
        (($sides.Keys | ForEach-Object { "$_=$($sides[$_])" }) -join ', ') + ")"
}
finally { $emptyBmp.Dispose() }


# --- second pass, same instance, proving the cache-hit fast path rather than reading it ------
#
# Review found that BuildTile now probes ShellIcons' cache synchronously and, on a hit, builds
# the real Image directly instead of drawing the fallback first, but nothing had ever rendered
# that path: the render above is always a cache MISS (a fresh process, an empty cache), so it
# only ever exercises the fallback-then-swap route Task 5 originally shipped. Rendering the same
# $shelfSurface a second time, after the first pass's background extraction has populated the
# cache, is what actually reaches the fast path: Render() rebuilds every tile from scratch, so
# BuildTile runs again and this time finds Plith.exe's icon already cached.
#
# Two things are asserted, not one, because the more interesting failure is not the flicker:
#   1. the icon host on the second pass never contains the drawn fallback element at all
#      (the flicker claim - if this fails, the cache-hit branch stopped being taken)
#   2. the second pass's image IS the same object the first pass extracted (the claim that
#      actually catches a broken fast path: a wrong or stale icon would still pass check 1)
$findPlithTile = { param($n) $n -is [Windows.Controls.Border] -and
    [Windows.Automation.AutomationProperties]::GetName($n) -eq 'Plith.exe' }

$pass1Tile = Find-VisualDescendants -Root $shelfSurface -Predicate $findPlithTile | Select-Object -First 1
if (-not $pass1Tile) { throw "second-pass check: could not find the Plith.exe tile after the first render." }
# Border.Child is the overlay Grid Task 7 added for the hover remove button (content at
# Children[0], the remove button at Children[1]), not the icon/label StackPanel directly any
# more - one more Children[0] than before reaches the icon host again.
$pass1IconHost = $pass1Tile.Child.Children[0].Children[0]
$pass1Image = $pass1IconHost.Children | Where-Object { $_ -is [Windows.Controls.Image] } | Select-Object -First 1
if (-not $pass1Image) {
    throw "second-pass check: the first render never resolved a real icon for Plith.exe (still " +
          "the fallback after Wait-ForDispatcher) - the cache has nothing for the second pass to hit."
}
$pass1Icon = $pass1Image.Source

$shelfSurface.Render($surfaceModel)
Save-Visual -Element $shelfSurface -W $shelfSurfaceW -H $shelfSurfaceH -Name 'shelf-surface-pass2'

$pass2Tile = Find-VisualDescendants -Root $shelfSurface -Predicate $findPlithTile | Select-Object -First 1
if (-not $pass2Tile) { throw "second-pass check: could not find the Plith.exe tile after the second render." }
$pass2IconHost = $pass2Tile.Child.Children[0].Children[0]

$pass2Fallback = $pass2IconHost.Children | Where-Object { $_ -is [Windows.Shapes.Path] }
if ($pass2Fallback) {
    throw "second-pass check FAILED (assertion 1): the icon host still drew the fallback " +
          "geometry on a cache hit. The synchronous cache probe in BuildTile did not take."
}

$pass2Image = $pass2IconHost.Children | Where-Object { $_ -is [Windows.Controls.Image] } | Select-Object -First 1
if (-not $pass2Image) {
    throw "second-pass check FAILED (assertion 1): no Image at all in the icon host on the " +
          "cache-hit pass - not even a late one, since this pass never calls Wait-ForDispatcher."
}
if (-not [object]::ReferenceEquals($pass2Image.Source, $pass1Icon)) {
    throw "second-pass check FAILED (assertion 2): the second render's icon is not the same " +
          "object the first render extracted - the fast path produced A icon, not the RIGHT one."
}

"  second-pass check passed: cache hit on pass two painted the real icon directly (no fallback " +
"element in the icon host), and it is reference-equal to what pass one actually extracted."

# --- a menu open on a tile must not survive that tile's own destruction --------------------
#
# Review of Task 7 found this the hard way: SetItems calls Render on every Items message, and
# Plith re-sends the whole Items set after every one of the four mutating verbs, so a right-click
# on tile A followed by ANY refresh (even one triggered by a totally different tile) tears tile A
# out of the tree while its menu is still open. The first version derived ShelfWindow's _menuOpen
# from ContextMenuOpening/Closing, routed events that bubble from the tile that opened them - and
# a tile Render has already destroyed has nowhere left for that bubble to go, so Closing never
# reached anyone and the shelf could never be dismissed again. The fix tracks the menu Render
# itself is responsible for (ShelfSurface._openMenu) and force-closes it before a single child is
# torn down. This drives that for real: a genuine ContextMenu, opened on a genuine tile, must
# still be open when asked, and must be closed - not orphaned on screen - the moment a render
# that did not know about it runs anyway.
$menuEvents = [System.Collections.Generic.List[bool]]::new()
$shelfSurface.add_MenuOpenChanged([Action[bool]]{ param($open) $menuEvents.Add($open) })

$menuTile = $pass2Tile
if (-not $menuTile.ContextMenu) { throw "menu-survives-render check: the Plith.exe tile has no ContextMenu." }

$menuTile.ContextMenu.IsOpen = $true
if (-not $menuTile.ContextMenu.IsOpen) { throw "menu-survives-render check: the context menu did not actually open." }
if ($menuEvents.Count -ne 1 -or -not $menuEvents[0]) {
    throw "menu-survives-render check: MenuOpenChanged did not report true when the menu opened (events: $($menuEvents -join ','))."
}

# The render this whole check exists for: something else on the shelf changed, Plith answered
# with a fresh Items set, and SetItems calls this while tile A's menu is still open.
$shelfSurface.Render($surfaceModel)

# Closed is measured NOT to fire synchronously with Render's IsOpen = $false (the default
# ContextMenu style animates its close), so this pumps the dispatcher the same way the shell-icon
# cache hit above needs to, rather than asserting on the tick right after Render returns.
Wait-ForDispatcher

if ($menuTile.ContextMenu.IsOpen) {
    throw "menu-survives-render check FAILED: the menu opened on a tile Render just destroyed is STILL OPEN, orphaned on screen with nothing left able to dismiss the shelf under it."
}
if ($menuEvents.Count -ne 2 -or $menuEvents[1]) {
    throw "menu-survives-render check FAILED: MenuOpenChanged never reported false after Render tore the tile down (events: $($menuEvents -join ',')). This is exactly the bug review found: ShelfWindow's _menuOpen would stay true forever."
}

"  menu-survives-render check passed: a context menu opened on a tile is force-closed before " +
"Render rebuilds the tiles under it, and MenuOpenChanged reports both the open and the close."


# --- a press must survive the render that press itself triggers ----------------------------
#
# The whole-branch review found the headline feature of this slice could not execute, and no
# gate on the branch could see it, because it lived in a SEAM: one task taught a press to
# re-render (ShelfWindow answers EntryPressed with Page.Render), another taught a press to grow
# into a drag, and each was correct on its own. Render clears Columns.Children and rebuilds
# every tile, so the element that took the press was out of the tree before the button came up;
# with the press state kept in a local captured by that element's own handlers, the replacement
# tile came up with nothing to continue and DragOutRequested could never be raised. Dragging a
# file out to another application, and dragging a tile between stacks, were both dead on the
# first gesture and on every one after it.
#
# This drives that seam for real: a genuine PreviewMouseLeftButtonDown raised on a genuine tile,
# EntryPressed wired exactly the way ShelfWindow wires it (select, then re-render), and then the
# move that should still become a drag afterwards.
#
# WHAT IS SUBSTITUTED, and why, so this check is not read as more than it is. The move's
# position and button state come from the MOUSE DEVICE, and neither can be synthesized offscreen:
# MouseDevice.GetPosition returns (0,0) for an element in no PresentationSource, and
# MouseEventArgs.LeftButton reports the physical button, which no script can hold down. So the
# move arrives through ShelfSurface.ContinuePress - the same method, with the same arguments,
# that the tile's own PreviewMouseMove handler calls, and the ONLY thing the handler adds is the
# e.LeftButton test. The press, the render and the tile teardown are all real.
$dragEvents = [System.Collections.Generic.List[object]]::new()
$shelfSurface.add_DragOutRequested([Action[Windows.DependencyObject, System.Collections.Generic.IReadOnlyList[string]]]{
    param($src, $paths) $dragEvents.Add([pscustomobject]@{ Source = $src; Paths = @($paths) })
})

# Exactly what ShelfWindow's constructor does with EntryPressed, including the unconditional
# Render that is the whole point of this check.
$pressRenders = 0
$shelfSurface.add_EntryPressed([Action[string, bool]]{
    param($path, $additive)
    $script:pressRenders++
    if ($additive) { $surfaceModel.Select($path, $true) } else { $surfaceModel.DragPaths($path) | Out-Null }
    $shelfSurface.Render($surfaceModel)
})

$pressedPath = Join-Path $bin 'Plith.exe'
$pressedTile = Find-VisualDescendants -Root $shelfSurface -Predicate $findPlithTile | Select-Object -First 1
if (-not $pressedTile) { throw "press-to-drag check: no Plith.exe tile to press." }
if ($dragEvents.Count -ne 0) { throw "press-to-drag check: a drag was raised before anything was pressed." }

# The press start, read the same way the down handler reads it (the same device, the same
# relative element), so the threshold arithmetic below is measured against what was recorded
# rather than against an assumption about it.
$pressOrigin = [Windows.Input.Mouse]::GetPosition($shelfSurface)

$down = [Windows.Input.MouseButtonEventArgs]::new([Windows.Input.Mouse]::PrimaryDevice, 0, [Windows.Input.MouseButton]::Left)
$down.RoutedEvent = [Windows.UIElement]::PreviewMouseLeftButtonDownEvent
$pressedTile.RaiseEvent($down)

# PRECONDITIONS, asserted before the thing this check is about, so it cannot pass vacuously.
if ($pressRenders -ne 1) {
    throw "press-to-drag check: the press did not reach EntryPressed (renders: $pressRenders). " +
          "Nothing below would prove anything - the defect being checked is a press SURVIVING a render."
}
$rebuiltTile = Find-VisualDescendants -Root $shelfSurface -Predicate $findPlithTile | Select-Object -First 1
if (-not $rebuiltTile) { throw "press-to-drag check: the tile is gone entirely after the render." }
if ([object]::ReferenceEquals($rebuiltTile, $pressedTile)) {
    throw "press-to-drag check: Render did NOT replace the pressed tile, so the seam this check " +
          "exists for is not being exercised. Either Render stopped rebuilding tiles or the " +
          "press stopped triggering one, and either way this check has become decoration."
}

# Below the threshold it is still a click: this must NOT raise, and must NOT consume the press.
$hMin = [Windows.SystemParameters]::MinimumHorizontalDragDistance
$vMin = [Windows.SystemParameters]::MinimumVerticalDragDistance
$near = [Windows.Point]::new($pressOrigin.X + $hMin - 1, $pressOrigin.Y + $vMin - 1)
if ($shelfSurface.ContinuePress($rebuiltTile, $pressedPath, $near)) {
    throw "press-to-drag check FAILED: a move of ($($hMin - 1), $($vMin - 1)) started a drag, " +
          "under the system threshold of ($hMin, $vMin). A click would become a drag."
}
if ($dragEvents.Count -ne 0) { throw "press-to-drag check FAILED: DragOutRequested fired below the drag threshold." }

# And past it, it is a drag. THIS is the assertion the review asked for.
$far = [Windows.Point]::new($pressOrigin.X + $hMin + 1, $pressOrigin.Y + $vMin + 1)
if (-not $shelfSurface.ContinuePress($rebuiltTile, $pressedPath, $far)) {
    throw "press-to-drag check FAILED: a press that triggered a re-render can no longer become " +
          "a drag. The rebuilt tile has no press behind it, so DragOutRequested is never raised - " +
          "dragging a file OUT and dragging a tile between stacks are both dead. This is the " +
          "Critical the whole-branch review found."
}
if ($dragEvents.Count -ne 1) {
    throw "press-to-drag check FAILED: expected exactly one DragOutRequested, got $($dragEvents.Count)."
}
if (-not [object]::ReferenceEquals($dragEvents[0].Source, $rebuiltTile)) {
    throw "press-to-drag check FAILED: the drag was raised for an element that is not the tile " +
          "under the pointer. ShelfWindow.StartDrag refuses a source that is not in its own " +
          "window, so a detached tile here would be refused at the next step instead of dragging."
}
if ($dragEvents[0].Paths -notcontains $pressedPath) {
    throw "press-to-drag check FAILED: the drag carries $($dragEvents[0].Paths -join ', '), not the pressed path $pressedPath."
}

# --- and a press nothing on this control ever saw released must NOT become a drag ----------
#
# No mouse capture is taken, so a release OUTSIDE the window raises no event here at all, and a
# drag out ends outside the window by definition. So a press survives every successful drag out
# unless something else clears it, and the next down on anything that is not a tile, followed by
# a move onto one, would start a drag from a press that never landed on a tile. ShelfSurface's
# constructor clears on button-DOWN at the root for exactly this; what follows drives it.
#
# The control for this half is the half above: the SAME far point, on the same surface, through
# the same method, raised a drag a moment ago. The only difference below is the non-tile down
# between the press and the move. So a false here is the guard working rather than the check
# having gone quiet - if the root clear were removed, this would raise and the assertion would
# fail, and if the press mechanism itself broke, the half above would fail first.
$pressedTile2 = Find-VisualDescendants -Root $shelfSurface -Predicate $findPlithTile | Select-Object -First 1
if (-not $pressedTile2) { throw "press-to-drag check: no tile to press a second time." }

$down2 = [Windows.Input.MouseButtonEventArgs]::new([Windows.Input.Mouse]::PrimaryDevice, 0, [Windows.Input.MouseButton]::Left)
$down2.RoutedEvent = [Windows.UIElement]::PreviewMouseLeftButtonDownEvent
$pressedTile2.RaiseEvent($down2)
if ($pressRenders -ne 2) {
    throw "press-to-drag check: the second press did not reach EntryPressed (renders: $pressRenders), " +
          "so there is no live press for the non-tile down below to clear."
}
$rebuiltTile2 = Find-VisualDescendants -Root $shelfSurface -Predicate $findPlithTile | Select-Object -First 1
if (-not $rebuiltTile2) { throw "press-to-drag check: the tile is gone after the second render." }

# A down that lands on this control but on no tile: the header, the Clear or new-stack button,
# or the gap between two tiles. Raised on the root itself, which is the one element guaranteed
# to be in the tunnelling route of every one of those and to run no BeginPress of its own.
$nonTileDown = [Windows.Input.MouseButtonEventArgs]::new([Windows.Input.Mouse]::PrimaryDevice, 0, [Windows.Input.MouseButton]::Left)
$nonTileDown.RoutedEvent = [Windows.UIElement]::PreviewMouseLeftButtonDownEvent
$shelfSurface.RaiseEvent($nonTileDown)
if ($pressRenders -ne 2) {
    throw "press-to-drag check: a down on the surface root reached EntryPressed, which it must " +
          "not - it is the stand-in for a press that lands on no tile at all."
}

if ($shelfSurface.ContinuePress($rebuiltTile2, $pressedPath, $far)) {
    throw "press-to-drag check FAILED: a move started a drag from a press this control never " +
          "saw released. A drag out ends outside this window by definition, so that stale press " +
          "is the ordinary aftermath of every successful one, and a drag from a press that did " +
          "not land is the exact failure this branch exists because of."
}
if ($dragEvents.Count -ne 1) {
    throw "press-to-drag check FAILED: DragOutRequested fired for a press that had been cleared " +
          "(events: $($dragEvents.Count), expected 1)."
}

"  press-to-drag check passed: a press that re-rendered the shelf under itself still became a " +
"drag on the rebuilt tile (threshold $hMin x $vMin honoured on both sides), carrying the pressed " +
"path; and a press cleared by a down on no tile did NOT, at the same far point."

# --- the within-surface drop target is GONE, deliberately ------------------------------------
#
# There used to be a drop-target check here, proving that TargetStackIndex resolved a release
# to the right column after the row was centred. The flat shelf has no columns to resolve to
# and no within-surface drop at all: a tile drags OUT and nothing else, so TargetStackIndex,
# OnColumnsDrop and the new-stack zone were deleted rather than adapted. A check kept here
# would be testing a gesture the product no longer offers.
#
# THE LAYOUT PASS BELOW CAME FROM THAT DELETED SECTION AND HAD TO COME BACK. It reads like part
# of the drop-target check and is not: the tile-hit check below depends on it just as much, and
# removing it made that check report "a tile was never arranged (0 x 0)" for a surface whose
# tiles were perfectly fine.

# A REAL layout pass, in a host, because the press-to-drag check above ended with a Render and
# nothing has arranged the tree since. A Border that was built but never arranged reports 0 x 0,
# and the tile-hit check below asks each tile for its ActualWidth to know where to aim.
#
# Measure/Arrange/UpdateLayout called directly on the surface do NOT fix it. Save-Visual detaches
# the element when it is done (see its own comment for why), and a detached element with no
# PresentationSource does not run a layout pass on request. Parenting it the same way Save-Visual
# does is what actually arranges the children.
$layoutHost = [Windows.Controls.Border]::new()
$layoutHost.Width = $shelfSurfaceW
$layoutHost.Height = $shelfSurfaceH
$layoutHost.Child = $shelfSurface
$layoutHost.Measure([Windows.Size]::new($shelfSurfaceW, $shelfSurfaceH))
$layoutHost.Arrange([Windows.Rect]::new(0, 0, $shelfSurfaceW, $shelfSurfaceH))
$layoutHost.UpdateLayout()

# --- a tile answers a pointer ANYWHERE inside it ---------------------------------------------
#
# Why this exists, and why nothing above could have caught it. Every other check in this file
# hands an event straight to the element it means: the press-to-drag check calls
# $pressedTile.RaiseEvent($down), which is a press the tile receives BY CONSTRUCTION. Real input
# arrives at a POINT, and the window decides which element that point belongs to. Those are
# different questions, and the second one had never been asked here.
#
# Measured on hardware before this check was written (docs/SHELF-VERIFICATION.md 3.10): a click
# at a tile's exact centre did nothing at all - no hover affordance, no selection, no press and
# therefore no drag - while the same click on the tile's icon did all three. The tile Border
# carried no Background, and WPF hit-tests a Transparent brush but NOT a null one, so only the
# painted icon and label answered the pointer and the gaps between them fell through to whatever
# was behind. ShelfWindow.xaml's own comment states that exact rule for the window's background;
# the tile did not follow it.
#
# So this check asks the tree the question real input asks: given a point, which element is it?
$findAnyTile = { param($n) $n -is [Windows.Controls.Border] -and
    $n.Width -eq 64 -and $n.Height -eq 64 -and $null -ne $n.Child }
$hitTiles = @(Find-VisualDescendants -Root $shelfSurface -Predicate $findAnyTile)
if ($hitTiles.Count -eq 0) { throw "tile-hit check: no 64x64 tile Borders in the rendered surface." }

# --- the selection ring must have room at BOTH edges of a row --------------------------------
#
# Reported from the running build on 2026-09-20: the ring on the leftmost and rightmost tiles
# "goes outside the area and is not visible". The ring is a Border thickness drawn INSIDE the
# tile, so it cannot leave the tile; what it can do is sit hard against the card's own edge with
# nothing between the two. This measures the gap rather than squinting at it.
$rowTop = ($hitTiles | ForEach-Object { $_.TranslatePoint([Windows.Point]::new(0,0), $layoutHost).Y } |
           Sort-Object | Select-Object -First 1)
$firstRow = @($hitTiles | Where-Object {
    [Math]::Abs($_.TranslatePoint([Windows.Point]::new(0,0), $layoutHost).Y - $rowTop) -lt 1 })
$lefts  = @($firstRow | ForEach-Object { $_.TranslatePoint([Windows.Point]::new(0,0), $layoutHost).X })
$rights = @($firstRow | ForEach-Object { $_.TranslatePoint([Windows.Point]::new(0,0), $layoutHost).X + $_.ActualWidth })
$leftGap  = ($lefts | Sort-Object | Select-Object -First 1)
$rightGap = $shelfSurfaceW - ($rights | Sort-Object -Descending | Select-Object -First 1)
"  edge check: first row has $($firstRow.Count) tile(s); left gap $([Math]::Round($leftGap,1)) DIP, right gap $([Math]::Round($rightGap,1)) DIP"

function Test-HitReaches {
    param([Windows.DependencyObject]$Hit, [Windows.DependencyObject]$Target)
    $node = $Hit
    while ($node) {
        if ([object]::ReferenceEquals($node, $Target)) { return $true }
        $node = try { [Windows.Media.VisualTreeHelper]::GetParent($node) } catch { $null }
    }
    $false
}

$hitFailures = @()
foreach ($hitTile in $hitTiles) {
    $w = $hitTile.ActualWidth; $h = $hitTile.ActualHeight
    if ($w -le 0 -or $h -le 0) { throw "tile-hit check: a tile was never arranged ($w x $h)." }

    # The centre, and the four points a quarter of the way in from each corner. The centre is the
    # one a person aims at and was the measured failure; the quarter points cover the padding and
    # the space either side of the label, which fail the same way for the same reason.
    $hitPoints = @(
        @{ Name = 'centre';       P = [Windows.Point]::new($w / 2, $h / 2) }
        @{ Name = 'upper left';   P = [Windows.Point]::new($w / 4, $h / 4) }
        @{ Name = 'upper right';  P = [Windows.Point]::new($w * 3 / 4, $h / 4) }
        @{ Name = 'lower left';   P = [Windows.Point]::new($w / 4, $h * 3 / 4) }
        @{ Name = 'lower right';  P = [Windows.Point]::new($w * 3 / 4, $h * 3 / 4) }
    )
    foreach ($hitPoint in $hitPoints) {
        $inSurface = $hitTile.TranslatePoint($hitPoint.P, $shelfSurface)
        $result = [Windows.Media.VisualTreeHelper]::HitTest($shelfSurface, $inSurface)
        $name = [Windows.Automation.AutomationProperties]::GetName($hitTile)
        if (-not $result) {
            $hitFailures += "      '$name' $($hitPoint.Name): the point hit NOTHING at all."
            continue
        }
        if (-not (Test-HitReaches -Hit $result.VisualHit -Target $hitTile)) {
            $hitFailures += ("      '$name' $($hitPoint.Name): resolved to " +
                "$($result.VisualHit.GetType().Name), which is not inside this tile.")
        }
    }
}
if ($hitFailures.Count -gt 0) {
    throw ("tile-hit check FAILED: a pointer inside a tile does not reach that tile, so a hover, " +
           "a click and the press a drag starts from are all dead there.`n" +
           ($hitFailures -join "`n"))
}

"  tile-hit check passed: all five probe points inside each of the $($hitTiles.Count) tiles " +
"resolve to that tile, so a hover, a press and a drag can start anywhere on one rather than only " +
"where its icon or label happens to paint."


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
#
# The stand-in desktop follows the theme. A light-theme card judged against a near-black ground
# is a card judged against a desktop nobody using the light theme has.
$desktop = if ($Theme -eq 'Light') { '#E9EAEC' } else { '#151A21' }
$audioCard = [Plith.Views.AudioCardView]::new()
$audioCard.DataContext = $audioVm
Save-Visual -Element $audioCard -W 382.0 -H 56.0 -Name 'card-audio' -Background $desktop

$mediaCard = [Plith.Views.MediaCardView]::new()
$mediaCard.DataContext = $mediaVm
Save-Visual -Element $mediaCard -W 382.0 -H 56.0 -Name 'card-media' -Background $desktop

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
