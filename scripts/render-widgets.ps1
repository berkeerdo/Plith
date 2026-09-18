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
$stack0 = [string[]](@($shelfFolder) + ($fixtures[0..3] | ForEach-Object { Join-Path $shelfDir $_ }))
$stack1 = [string[]]('\nosuchhost\share\ghost.txt', (Join-Path $bin 'Plith.exe'))
$surfaceModel.SetStack(0, 2, $stack0)
$surfaceModel.SetStack(1, 2, $stack1)

# One tile selected, so the accent ring (AccentBrush, the one place this control paints the
# accent) is actually in the saved renders. Without this none of the three showed it at all, and
# the white-accent render exists specifically to catch an accent-on-surface contrast defect it
# could not catch on an empty selection. The folder tile, stack0's only real tile beside its
# overflow count, so the ring is visible next to that tile in the same picture.
$surfaceModel.Select($shelfFolder, $false)

# 384 x 224. Width was the arithmetic guess (five 64 DIP tiles, four 8 DIP gaps, two 16 DIP
# margins) and the render confirmed it exactly - a five-stack, no-overflow model fits with the
# last tile's icon and full file name clear of the rounded corner. Height was NOT: the arithmetic
# guess of 264 left roughly a quarter of the page blank below the second tile row in every render.
# Measured instead, with ShelfSurface.Measure(new Size(384, PositiveInfinity)): the control wants
# 210 DIP at this width. 224 is that measurement plus 14 DIP of margin, not a second guess - the
# same shape kept in sync here by hand as frameW/frameH are with WidgetFrame, since the harness
# and the control it renders are deliberately two separate assemblies (see the DropCatcher DLL
# load above).
$shelfSurfaceW = 384.0
$shelfSurfaceH = 224.0

$shelfSurface = [Plith.DropCatcher.Shelf.ShelfSurface]::new()
$shelfSurface.Apply($shelfPalette)
$shelfSurface.Render($surfaceModel)
Wait-ForDispatcher
Save-Visual -Element $shelfSurface -W $shelfSurfaceW -H $shelfSurfaceH -Name 'shelf-surface'

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
$pass1IconHost = $pass1Tile.Child.Children[0]
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
$pass2IconHost = $pass2Tile.Child.Children[0]

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
