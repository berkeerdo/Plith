# scripts/capture-shelf.ps1 - opens the shelf probe and captures it from the screen, for real.
#
# Why this exists: the shelf renders in a LAYERED window in a second process, and every other
# instrument this repository has looks at source or at an offscreen render. None of them sees
# what a person sees. docs/SHELF-VERIFICATION.md exists because that gap kept shipping design
# defects that a green build, green tests and a green lint all missed.
#
# The one flag that makes it work: BitBlt must be called with SRCCOPY -bor CAPTUREBLT. Without
# CAPTUREBLT a layered window is simply ABSENT from the result, and .NET's
# Graphics.CopyFromScreen cannot pass it - its CopyPixelOperation enum has no member for the
# combination and rejects the OR as an invalid value. So the convenient API returns a screenshot
# with the shelf missing, which is indistinguishable from a shelf that never opened. That is
# almost certainly how "no screenshot tool reaches it" became this repository's premise.
#
# That premise also carried a SECOND claim, that the capture cannot work over Remote Desktop.
# Measured on 2026-09-19 from an rdp-tcp#0 session (qwinsta, not $env:SESSIONNAME, which is
# stamped at process start and was stale on this machine): it captures correctly over RDP too.
# The claim had been inherited from the OSD's notes in docs/PHASE5-VERIFICATION.md and applied to
# the shelf without ever being measured against the shelf.
#
# What this does NOT remove is the need for a person at the physical console to PRESS things:
# synthetic input from a Medium process cannot reach Plith's UIAccess window, which is this
# feature's whole subject. It removes the need for a person to JUDGE things.

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-capture",
    [string]$Configuration = 'Debug',

    # Physical screen pixels, the same four numbers the OpenShelf wire message carries. Default
    # is centred against the top edge of the primary display, computed below rather than fixed,
    # because a hardcoded 708 is only centred on one particular width.
    [int]$X = -1,
    [int]$Y = 0,
    [int]$W = 384,
    [int]$H = 224,

    # Capture an already-running shelf instead of launching the probe.
    [switch]$NoLaunch,

    # Also write a nearest-neighbour blow-up, which is how the extension tag's overflow out of
    # its glyph was caught rather than by a person squinting at a 22 DIP icon.
    [int]$MagnifyScale = 3
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, System.Windows.Forms

$root = Split-Path -Parent $PSScriptRoot

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ShelfCapture {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern IntPtr GetDesktopWindow();
    [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
    [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
    [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr d, int dx, int dy, int w, int h,
                                                              IntPtr s, int sx, int sy, uint rop);
    public struct RECT { public int Left, Top, Right, Bottom; }
    public const uint SRCCOPY    = 0x00CC0020;
    public const uint CAPTUREBLT = 0x40000000;
    public const int  GWL_EXSTYLE   = -20;
    public const int  WS_EX_LAYERED = 0x80000;
}
'@

function Get-ShelfWindow {
    # Script scope, not the function's: the EnumWindows callback runs in its own scope and can
    # only append to a variable it can name. A function-local $hits leaves $script:hits null,
    # and += against null fails inside the callback rather than at the call site.
    $script:hits = @()
    $cb = [ShelfCapture+EnumProc]{
        param($h, $l)
        $owner = [uint32]0
        [void][ShelfCapture]::GetWindowThreadProcessId($h, [ref]$owner)
        $pn = try { (Get-Process -Id $owner -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -and $pn -like '*DropCatcher*' -and [ShelfCapture]::IsWindowVisible($h)) {
            $r = New-Object 'ShelfCapture+RECT'
            [void][ShelfCapture]::GetWindowRect($h, [ref]$r)
            $ex = [ShelfCapture]::GetWindowLongW($h, [ShelfCapture]::GWL_EXSTYLE)
            if ($ex -band [ShelfCapture]::WS_EX_LAYERED) {
                $script:hits += [pscustomobject]@{
                    Hwnd = $h; Pid = $owner
                    X = $r.Left; Y = $r.Top
                    W = ($r.Right - $r.Left); H = ($r.Bottom - $r.Top)
                }
            }
        }
        $true
    }
    [void][ShelfCapture]::EnumWindows($cb, [IntPtr]::Zero)
    $script:hits | Select-Object -First 1
}

function Save-Capture {
    param([int]$X, [int]$Y, [int]$W, [int]$H, [string]$Path)

    $bmp = New-Object System.Drawing.Bitmap $W, $H
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dst = $g.GetHdc()
    $desktop = [ShelfCapture]::GetDesktopWindow()
    $src = [ShelfCapture]::GetDC($desktop)
    # CAPTUREBLT is KEPT although it proved unnecessary on the path this was measured on (see the
    # header): it costs nothing, and it is genuinely required wherever the layered surface is not
    # already composited into the desktop DC. Removing a flag because one configuration tolerates
    # its absence is how this repository's capture premise got written backwards in the first
    # place. Falsified rather than assumed: with the -bor removed, the shelf was still captured
    # here, so this flag is NOT what makes the capture work in this session.
    $rop = [ShelfCapture]::SRCCOPY -bor [ShelfCapture]::CAPTUREBLT
    $ok = [ShelfCapture]::BitBlt($dst, 0, 0, $W, $H, $src, $X, $Y, $rop)
    [void][ShelfCapture]::ReleaseDC($desktop, $src)
    $g.ReleaseHdc($dst)
    $g.Dispose()
    if (-not $ok) {
        $bmp.Dispose()
        # The message names the overwhelmingly likely cause, because the bare failure is exactly
        # the observation that would get generalised into "layered windows cannot be captured".
        # Measured on 2026-09-19: the same capture succeeded repeatedly while the session read
        # Active, and began failing the moment the RDP client disconnected and qwinsta turned the
        # session to Disc. A disconnected or locked session has no composed desktop to blit from,
        # and that is a property of the SESSION, not of the window being layered.
        $state = (qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }) -replace '\s+', ' '
        throw ("BitBlt failed for ${W}x${H} at $X,$Y.`n" +
               "    Current session: $state`n" +
               "    If that session is Disc (disconnected) or the workstation is locked, there is " +
               "no desktop to capture and this will fail for ANY window. Reconnect and re-run.")
    }

    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)

    # Two numbers, and NEITHER is a proof - which is the point of reporting both rather than
    # gating on one.
    #
    # distinctColors catches the degenerate miss, where the capture comes back uniform. It was
    # nearly trusted as more than that: with CAPTUREBLT removed on purpose, the count did not
    # move at all (339 either way), because the window sitting BEHIND the shelf was a terminal
    # full of text and a capture of that is just as colourful as a capture of the shelf. A count
    # taken over an unknown background cannot tell the two apart.
    #
    # darkFraction is ONE-SIDED, and was measured before being described as anything more. The
    # shelf paints its surface at alpha 0xF0 over near-black (ShelfWindow's SurfaceStart/End), so
    # a captured shelf is ~0.96 dark whatever is behind it. But the background measured here was
    # a dark terminal at 0.913, so on this desktop the two are nearly the same number. It can
    # catch a miss against a LIGHT desktop and it cannot catch one against a dark desktop.
    #
    # So the honest instruction is the one in the header: LOOK AT THE PNG. Neither number is a
    # substitute, and the pair is printed so that nobody has to take one on trust.
    $distinct = @{}
    $dark = 0; $total = 0
    for ($py = 0; $py -lt $H; $py += 2) {
        for ($px = 0; $px -lt $W; $px += 2) {
            $c = $bmp.GetPixel($px, $py)
            $distinct[$c.ToArgb()] = 1
            $total++
            if ((0.299 * $c.R + 0.587 * $c.G + 0.114 * $c.B) -lt 48) { $dark++ }
        }
    }
    $bmp.Dispose()
    [pscustomobject]@{
        DistinctColors = $distinct.Count
        DarkFraction   = [Math]::Round($dark / $total, 3)
    }
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if ($X -lt 0) {
    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $X = [int](($screen.Width - $W) / 2)
}

$probe = $null
if (-not $NoLaunch) {
    $exe = Join-Path $root "src\Plith.DropCatcher\bin\$Configuration\net10.0-windows10.0.22000.0\Plith.DropCatcher.exe"
    if (-not (Test-Path $exe)) { throw "Build $Configuration first: $exe not found." }
    $probe = Start-Process -FilePath $exe -ArgumentList '--shelfprobe', $X, $Y, $W, $H -PassThru
}

try {
    # Polled rather than slept at: the shelf grows out of the notch, so a fixed wait either
    # captures a half-grown shape or wastes time. The window existing and being layered is the
    # signal; the short settle after it is for the growth animation to finish.
    $window = $null
    $deadline = [datetime]::UtcNow.AddSeconds(15)
    while ([datetime]::UtcNow -lt $deadline) {
        $window = Get-ShelfWindow
        if ($window) { break }
        Start-Sleep -Milliseconds 150
    }
    if (-not $window) { throw "No visible layered Plith.DropCatcher window appeared within 15s." }
    Start-Sleep -Milliseconds 700

    "  shelf window: hwnd=$($window.Hwnd) pid=$($window.Pid) at $($window.X),$($window.Y) $($window.W)x$($window.H)"
    if ($window.W -ne $W -or $window.H -ne $H) {
        "  NOTE: the window is $($window.W)x$($window.H), not the ${W}x${H} asked for - capturing what is there."
    }

    $full = Join-Path $OutDir 'shelf-live.png'
    $stats = Save-Capture -X $window.X -Y $window.Y -W $window.W -H $window.H -Path $full
    "  shelf-live.png  $($window.W)x$($window.H)  distinctColors=$($stats.DistinctColors) darkFraction=$($stats.DarkFraction)"
    if ($stats.DistinctColors -lt 8) {
        throw ("The capture came back with $($stats.DistinctColors) distinct colours, i.e. " +
               "uniform, so the layered window was NOT in it.")
    }
    if ($stats.DarkFraction -lt 0.5) {
        "  WARNING: only $($stats.DarkFraction) of the frame is dark, where a captured shelf measures" +
        " ~0.96. That suggests the capture shows the desktop instead. Look at the PNG."
    }
    "  Neither number PROVES the shelf is in the frame (see Save-Capture). Look at $full."

    if ($MagnifyScale -gt 1) {
        $src = [System.Drawing.Bitmap]::FromFile($full)
        $big = New-Object System.Drawing.Bitmap ($src.Width * $MagnifyScale), ($src.Height * $MagnifyScale)
        $g = [System.Drawing.Graphics]::FromImage($big)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
        $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
        $g.DrawImage($src, 0, 0, $big.Width, $big.Height)
        $g.Dispose()
        $magnified = Join-Path $OutDir 'shelf-live-magnified.png'
        $big.Save($magnified, [System.Drawing.Imaging.ImageFormat]::Png)
        "  shelf-live-magnified.png  $($big.Width)x$($big.Height)  (nearest-neighbour ${MagnifyScale}x)"
        $src.Dispose(); $big.Dispose()
    }
}
finally {
    if ($probe -and -not $probe.HasExited) { $probe | Stop-Process -Force -ErrorAction SilentlyContinue }
}

"Done."
