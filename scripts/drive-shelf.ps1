# scripts/drive-shelf.ps1 - presses things on the shelf, for real, and judges what happened.
#
# Why this exists. capture-shelf.ps1 removed the need for a person to JUDGE the shelf; this
# removes the need for a person to PRESS it. The premise that a person was required held for
# Plith's own OSD, which is a UIAccess window that synthetic input genuinely cannot reach - but
# the shelf is not that window. The shelf belongs to Plith.DropCatcher, which runs at MEDIUM
# integrity, and UIPI does not stand between one Medium process and another. Measured rather
# than assumed, on 2026-09-19 from an rdp-tcp#0 session: UI Automation reads the shelf's whole
# tree with names and screen rectangles, and SendInput drives it.
#
# THE THREE INSTRUMENT DEFECTS THIS FILE EXISTS TO STOP BEING REDISCOVERED. Each produced a
# confident, plausible, wrong number before it was caught, and two of them read as product
# defects:
#
#   1. PowerShell assigns to a COPY when the target is a field of a nested value type. Every
#      `$i.u.mi.dwFlags = ...` is silently discarded and SendInput receives an all-zero
#      structure - and reports success. So all input construction below lives in C#, where an
#      assignment is an assignment. A one-line check at startup proves the pointer really moves
#      before anything else is measured.
#
#   2. THE POINTER MUST ARRIVE BY MOVEMENT. A single absolute jump onto a window leaves WPF's
#      Mouse.DirectlyOver stale, so nothing under the pointer sees a MouseEnter. Move-ShelfPointer
#      glides, and finishes with a one-pixel there-and-back so a glide that ends where it started
#      still sends a message.
#
#   3. THE POINTER MUST NEVER LEAVE THE SHELF. ShelfWindow.LeaveGrace is 500 ms: park the pointer
#      outside and the shelf dismisses half a second later. A "rest" baseline taken off the shelf
#      and compared against a "hover" taken after it is two captures of the same desktop, and it
#      reports EXACTLY zero changed pixels - which reads like a product that ignores the mouse.
#      Baselines are taken over a neutral part of the shelf instead.
#
# WHAT THIS CANNOT ANSWER, stated here so its output is not read as more than it is. In
# --shelfprobe mode App's CatcherClient is null, so RemoveItems, ClearShelf, NewStack and Restack
# are raised and go nowhere. Plith owns the shelf model and answers those verbs with a fresh set
# of Items messages, so with no Plith the PAGE NEVER CHANGES in response to them. Every item in
# docs/SHELF-VERIFICATION.md section 3 whose expectation is "the tile disappears" or "shelf.txt
# no longer lists it" still needs the real pair of processes.

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-drive",
    [string]$Configuration = 'Debug',
    [int]$W = 384,
    [int]$H = 224
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "src\Plith.DropCatcher\bin\$Configuration\net10.0-windows10.0.22000.0\Plith.DropCatcher.exe"
if (-not (Test-Path $exe)) { throw "Build $Configuration first: $exe not found." }
$catcherLog = Join-Path $env:LOCALAPPDATA 'Plith\dropcatcher.log'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ShelfInput {
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr extra; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr extra; }
    [StructLayout(LayoutKind.Explicit)] public struct UNION {
        [FieldOffset(0)] public MOUSEINPUT mi;
        [FieldOffset(0)] public KEYBDINPUT ki;
    }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public UNION u; }
    public struct POINT { public int X, Y; }
    public struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll", SetLastError=true)] static extern uint SendInput(uint n, INPUT[] i, int cb);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);

    const uint MOVE=0x0001, LEFTDOWN=0x0002, LEFTUP=0x0004, RIGHTDOWN=0x0008, RIGHTUP=0x0010;
    const uint ABSOLUTE=0x8000, VIRTUALDESK=0x4000, KEYUP=0x0002;

    static void Send(INPUT[] a) {
        uint n = SendInput((uint)a.Length, a, Marshal.SizeOf(typeof(INPUT)));
        if (n != a.Length) throw new Exception("SendInput accepted " + n + " of " + a.Length +
            ", Win32 error " + Marshal.GetLastWin32Error());
    }
    static INPUT Mouse(uint flags, int dx, int dy) {
        INPUT i = new INPUT();
        i.type = 0; i.u.mi.dwFlags = flags; i.u.mi.dx = dx; i.u.mi.dy = dy;
        return i;
    }
    public static void Move(int x, int y) {
        int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77);
        int vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);
        int nx = (int)Math.Round((x - vx) * 65535.0 / (vw - 1));
        int ny = (int)Math.Round((y - vy) * 65535.0 / (vh - 1));
        Send(new INPUT[] { Mouse(MOVE | ABSOLUTE | VIRTUALDESK, nx, ny) });
    }
    public static void Glide(int x0, int y0, int x1, int y1, int steps) {
        for (int i = 1; i <= steps; i++) {
            Move(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
            System.Threading.Thread.Sleep(16);
        }
    }
    public static void LeftClick() {
        Send(new INPUT[] { Mouse(LEFTDOWN, 0, 0) });
        System.Threading.Thread.Sleep(70);
        Send(new INPUT[] { Mouse(LEFTUP, 0, 0) });
    }
    public static void RightClick() {
        Send(new INPUT[] { Mouse(RIGHTDOWN, 0, 0) });
        System.Threading.Thread.Sleep(70);
        Send(new INPUT[] { Mouse(RIGHTUP, 0, 0) });
    }
    public static void Key(ushort vk) {
        INPUT d = new INPUT(); d.type = 1; d.u.ki.wVk = vk;
        INPUT u = new INPUT(); u.type = 1; u.u.ki.wVk = vk; u.u.ki.dwFlags = KEYUP;
        Send(new INPUT[] { d }); System.Threading.Thread.Sleep(40); Send(new INPUT[] { u });
    }
    public static string Where() {
        POINT p; GetCursorPos(out p);
        IntPtr h = WindowFromPoint(p);
        uint owner; GetWindowThreadProcessId(h, out owner);
        return "cursor " + p.X + "," + p.Y + " over hwnd=" + h + " pid=" + owner;
    }
}
public class ShelfWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out ShelfInput.RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern IntPtr GetDesktopWindow();
    [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
    [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
    [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr d,int dx,int dy,int w,int h,IntPtr s,int sx,int sy,uint rop);
}
'@

function Find-ShelfWindow {
    $script:hits = @()
    $cb = [ShelfWin+EnumProc]{
        param($h, $l)
        $o = [uint32]0; [void][ShelfWin]::GetWindowThreadProcessId($h, [ref]$o)
        $pn = try { (Get-Process -Id $o -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -and $pn -like '*DropCatcher*' -and [ShelfWin]::IsWindowVisible($h)) {
            $r = New-Object 'ShelfInput+RECT'; [void][ShelfWin]::GetWindowRect($h, [ref]$r)
            if ([ShelfWin]::GetWindowLongW($h, -20) -band 0x80000) {
                $script:hits += [pscustomobject]@{ Hwnd=$h; Pid=$o; X=$r.Left; Y=$r.Top
                                                   W=($r.Right-$r.Left); H=($r.Bottom-$r.Top) }
            }
        }
        $true
    }
    [void][ShelfWin]::EnumWindows($cb, [IntPtr]::Zero)
    $script:hits | Select-Object -First 1
}

function Save-Shot([int]$X,[int]$Y,[int]$W,[int]$H,[string]$Path) {
    $bmp = New-Object System.Drawing.Bitmap $W, $H
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dst = $g.GetHdc()
    $desk = [ShelfWin]::GetDesktopWindow(); $src = [ShelfWin]::GetDC($desk)
    $ok = [ShelfWin]::BitBlt($dst, 0, 0, $W, $H, $src, $X, $Y, (0x00CC0020 -bor 0x40000000))
    [void][ShelfWin]::ReleaseDC($desk, $src); $g.ReleaseHdc($dst); $g.Dispose()
    if (-not $ok) {
        $bmp.Dispose()
        $state = (qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }) -replace '\s+', ' '
        throw ("BitBlt failed for ${W}x${H} at $X,$Y.`n    Current session: $state`n" +
               "    A disconnected (Disc) or locked session has no desktop to capture and this " +
               "fails for ANY window. Reconnect and re-run.")
    }
    $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp
}

function Compare-Shots($a, $b) {
    $changed = 0
    for ($y = 0; $y -lt $a.Height; $y++) {
        for ($x = 0; $x -lt $a.Width; $x++) {
            if ($a.GetPixel($x, $y).ToArgb() -ne $b.GetPixel($x, $y).ToArgb()) { $changed++ }
        }
    }
    $changed
}

function Get-Element {
    param($Window, [string]$Name, [string]$Type)
    $el = [System.Windows.Automation.AutomationElement]::FromHandle($Window.Hwnd)
    $byName = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    # BOTH name and control type: a tile, its label and its tooltip all carry the same name, and
    # a name-only search returned a 119x26 text run in place of a 64x64 tile once already.
    $ct = switch ($Type) {
        'Custom' { [System.Windows.Automation.ControlType]::Custom }
        'Button' { [System.Windows.Automation.ControlType]::Button }
        default  { $null }
    }
    $cond = if ($ct) {
        New-Object System.Windows.Automation.AndCondition($byName,
            (New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::ControlTypeProperty, $ct)))
    } else { $byName }
    $e = $el.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
    if (-not $e) { return $null }
    $r = $e.Current.BoundingRectangle
    [pscustomobject]@{ X=[int]$r.X; Y=[int]$r.Y; W=[int]$r.Width; H=[int]$r.Height
                       CX=[int]($r.X + $r.Width/2); CY=[int]($r.Y + $r.Height/2) }
}

# Menu items, asked for BY CONTROL TYPE from the shelf window's own element. A WPF ContextMenu
# lives in a popup HWND of its own and is NOT a direct ControlView child of the desktop, so
# walking the desktop's children reports an open menu as absent - which it did, while the
# product's own log said a menu was in flight.
function Get-MenuItems {
    param($Window)
    $el = [System.Windows.Automation.AutomationElement]::FromHandle($Window.Hwnd)
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::MenuItem)
    $names = @()
    foreach ($i in $el.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)) {
        $names += $i.Current.Name
    }
    $names
}

function Move-Pointer {
    param([int]$X, [int]$Y, [int]$Settle = 350)
    $p = New-Object 'ShelfInput+POINT'; [void][ShelfInput]::GetCursorPos([ref]$p)
    [ShelfInput]::Glide($p.X, $p.Y, $X, $Y, 16)
    [ShelfInput]::Glide($X, $Y, ($X + 1), $Y, 2)
    [ShelfInput]::Glide(($X + 1), $Y, $X, $Y, 2)
    Start-Sleep -Milliseconds $Settle
}

# --- the instrument checks itself before it measures anything --------------------------------
$origin = New-Object 'ShelfInput+POINT'; [void][ShelfInput]::GetCursorPos([ref]$origin)
[ShelfInput]::Move(400, 400)
Start-Sleep -Milliseconds 250
$check = New-Object 'ShelfInput+POINT'; [void][ShelfInput]::GetCursorPos([ref]$check)
if ([Math]::Abs($check.X - 400) -gt 2 -or [Math]::Abs($check.Y - 400) -gt 2) {
    throw ("The synthetic move did not move the pointer (it is at $($check.X),$($check.Y), not " +
           "400,400). Measure nothing with a broken instrument - see defect 1 in this file's header.")
}
"instrument check passed: the pointer really moves."

$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$x = [int](($screen.Width - $W) / 2)
$marker = (Get-Content $catcherLog -ErrorAction SilentlyContinue | Measure-Object -Line).Lines

Get-Process -Name '*DropCatcher*' -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
$proc = Start-Process -FilePath $exe -ArgumentList '--shelfprobe', $x, 0, $W, $H -PassThru

$shots = @()
$verdicts = @()
$failed = 0
try {
    $win = $null; $deadline = [datetime]::UtcNow.AddSeconds(15)
    while ([datetime]::UtcNow -lt $deadline) { $win = Find-ShelfWindow; if ($win) { break }; Start-Sleep -Milliseconds 50 }
    if (-not $win) { throw 'No visible layered Plith.DropCatcher window appeared within 15s.' }
    Start-Sleep -Milliseconds 400
    "shelf window: hwnd=$($win.Hwnd) pid=$($win.Pid) at $($win.X),$($win.Y) $($win.W)x$($win.H)"

    # Onto the shelf AT ONCE, so the leave timer is never armed for the rest of the run.
    Move-Pointer -X ($win.X + 20) -Y ($win.Y + 8) -Settle 300

    $tile = Get-Element -Window $win -Name 'quarterly-report.pdf' -Type Custom
    if (-not $tile) { throw 'UIA could not find the probe tile "quarterly-report.pdf".' }
    "tile: @$($tile.X),$($tile.Y) $($tile.W)x$($tile.H), centre $($tile.CX),$($tile.CY)"

    function Add-Verdict([string]$Name, [bool]$Ok, [string]$Detail) {
        $script:verdicts += "  [$(if ($Ok) { 'PASS' } else { 'FAIL' })] $Name - $Detail"
        if (-not $Ok) { $script:failed++ }
    }

    # 3.1 - the hover remove affordance, probed AT THE TILE'S CENTRE, which is where a person
    # aims and which was dead until the tile was given a Transparent background.
    $base = Save-Shot $win.X $win.Y $win.W $win.H (Join-Path $OutDir '1-neutral.png'); $shots += $base
    Move-Pointer -X $tile.CX -Y $tile.CY -Settle 450
    $hover = Save-Shot $win.X $win.Y $win.W $win.H (Join-Path $OutDir '2-hover.png'); $shots += $hover
    $dHover = Compare-Shots $base $hover
    $btn = Get-Element -Window $win -Name 'Remove quarterly-report.pdf from the shelf' -Type Button
    Add-Verdict '3.1 hover reveals the remove control, at the tile CENTRE' ([bool]$btn -and $dHover -gt 0) `
        "$dHover px changed; UIA button $(if ($btn) { "@$($btn.X),$($btn.Y) $($btn.W)x$($btn.H)" } else { 'ABSENT' })"

    # 3.2 - a press at the centre selects. The selection ring is ShelfModel.DragPaths showing up
    # on screen, and a press that does not land is also a drag that can never start.
    [ShelfInput]::LeftClick(); Start-Sleep -Milliseconds 900
    $sel = Save-Shot $win.X $win.Y $win.W $win.H (Join-Path $OutDir '3-selected.png'); $shots += $sel
    $dSel = Compare-Shots $hover $sel
    Add-Verdict '3.2 a click at the tile CENTRE selects it' ($dSel -gt 0) "$dSel px changed (the selection ring)"

    # The context menu, which is the only way 3.7 (Open) and 3.8 (Show in file manager) are
    # reachable at all, and the second route into 3.1's Remove.
    [ShelfInput]::RightClick(); Start-Sleep -Milliseconds 1200
    $items = @(Get-MenuItems -Window $win)
    $expected = @('Open', 'Show in file manager', 'Remove')
    Add-Verdict 'tile context menu carries Open / Show in file manager / Remove' `
        (($items.Count -eq 3) -and -not (Compare-Object $items $expected)) "[$($items -join ', ')]"
    $shots += Save-Shot ($win.X - 30) $win.Y ($win.W + 220) ($win.H + 200) (Join-Path $OutDir '4-menu.png')

    # 3.9 - an open menu must not let the shelf close under itself. LeaveGrace is 500 ms, so
    # taking the pointer off the shelf and waiting well past it is exactly the stated condition.
    $p = New-Object 'ShelfInput+POINT'; [void][ShelfInput]::GetCursorPos([ref]$p)
    [ShelfInput]::Glide($p.X, $p.Y, ($win.X - 300), ($win.Y + 780), 20)
    Start-Sleep -Milliseconds 2500
    $alive = (-not $proc.HasExited) -and [bool](Find-ShelfWindow)
    Add-Verdict '3.9 the shelf survives its own open menu' $alive `
        'pointer off the shelf for 2.5 s against a 500 ms grace'

    if ($alive) {
        # BACK ONTO THE SHELF BEFORE CLOSING THE MENU. With the pointer still outside, the
        # deferred dismissal is CORRECT to fire the moment the menu goes, and reading that as a
        # failure is what the first version of this driver did.
        Move-Pointer -X ($win.X + 20) -Y ($win.Y + 8) -Settle 400
        [ShelfInput]::Key(0x1B); Start-Sleep -Milliseconds 800
        Add-Verdict '3.9 Esc closes the menu' ((@(Get-MenuItems -Window $win)).Count -eq 0) 'no menu items left'
        Add-Verdict '3.9 the shelf outlives its menu' (-not $proc.HasExited) 'still up after the menu closed'

        if (-not $proc.HasExited) {
            # Hold still ON the shelf for three times the grace. If it goes now it went for the
            # wrong reason, and an Esc sent afterwards would be credited with a dismissal it did
            # not cause.
            Start-Sleep -Milliseconds 1500
            Add-Verdict '3.9 the shelf holds still with the pointer on it' (-not $proc.HasExited) `
                'no dismissal in 1.5 s with the pointer over the shelf'

            if (-not $proc.HasExited) {
                [ShelfInput]::Key(0x1B)
                $gone = $proc.WaitForExit(5000)
                $reason = (Get-Content $catcherLog | Select-Object -Last 3 |
                           Where-Object { $_ -match 'Shelf closing' }) -join ' / '
                # The shelf closing is the behaviour; the REASON is a separate claim, and the log
                # credits the first deferred reason rather than the Esc that actually did it
                # (ShelfWindow.Dismiss: "_pendingDismissal ?? why"). Reported, not asserted.
                Add-Verdict '3.9 Esc still dismisses the shelf after a menu' $gone `
                    "closed=$gone; log says: $reason"
            }
        }
    }
}
finally {
    foreach ($s in $shots) { if ($s) { $s.Dispose() } }
    if ($proc -and -not $proc.HasExited) { $proc | Stop-Process -Force -ErrorAction SilentlyContinue }
    [ShelfInput]::Move($origin.X, $origin.Y)
}

''
'=== verdicts ==='
$verdicts
''
'--- catcher log for this run ---'
Get-Content $catcherLog | Select-Object -Skip $marker | ForEach-Object { "  $_" }
''
"Shots in $OutDir. LOOK AT THEM: a pixel count says something changed, never that the right " +
"thing changed."
if ($failed -gt 0) { throw "$failed shelf check(s) FAILED." }
'Done.'
