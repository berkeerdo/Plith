# scripts/drive-shelf-pair.ps1 - drives the REAL Plith + drop catcher pair, and judges what
# happened by reading the file Plith writes.
#
# WHY THIS EXISTS, AND WHY drive-shelf.ps1 IS NOT ENOUGH. That script drives the catcher in
# --shelfprobe mode, where App's CatcherClient is null: RemoveItems and ClearShelf are raised
# and go nowhere, so the page never changes in response to them. Plith owns
# the shelf model. Every item in docs/SHELF-VERIFICATION.md section 3 whose expectation is "the
# tile disappears" or "shelf.txt no longer lists it" therefore needs both processes, connected,
# with the shelf opened the way a person opens it: a click on the notch.
#
# THE ORACLE IS shelf.txt, NOT THE PICTURE. Plith writes %LOCALAPPDATA%\Plith\shelf.txt on every
# change - one path per line, newest first, with no grouping of any kind (ShelfStore.Save). A
# pixel count says something changed and never says the right thing changed; the file says which
# paths are on the shelf, which is what sections 3.1 to 3.3 ask about. Every check below is
# written against it, and the screenshots are kept so a person can see what the file describes.
#
# Sections 3.4, 3.5 and 3.6 were about stacks and are gone with them.
#
# WHAT THIS SCRIPT TOUCHES, AND PUTS BACK. It replaces shelf.txt with a known fixture set and
# restores the original afterwards; it starts Plith from the build output and stops it; and it
# opens a small window of its own (see PRECONDITION 3) and closes it. It does not touch any
# window the person had open.
#
# ------------------------------------------------------------------------------------------
# THE SIX INSTRUMENT DEFECTS. The first three are drive-shelf.ps1's and are repeated because
# they are properties of this machine rather than of that file. The last three were found here.
#
#   1. PowerShell assigns to a COPY when the target is a field of a nested value type. Every
#      `$i.u.mi.dwFlags = ...` is silently discarded and SendInput receives an all-zero
#      structure - and reports success. All input construction lives in C# for that reason.
#
#   2. THE POINTER MUST ARRIVE BY MOVEMENT. A single jump onto a window leaves WPF's
#      Mouse.DirectlyOver stale, so nothing under the pointer sees a MouseEnter. Move-Pointer
#      glides and finishes with a one-pixel there-and-back, so a glide that ends where it
#      started still sends a message.
#
#   3. THE POINTER MUST NEVER LEAVE THE SHELF while the shelf is what is being measured.
#      ShelfWindow.LeaveGrace is 500 ms.
#
#   4. AN ANTI-CHEAT DRIVER CAN FILTER INJECTED INPUT, PARTIALLY, AND THE PARTIAL IS THE TRAP.
#      Measured on 2026-09-19 with Riot Vanguard loaded (service vgc): SendInput rejects
#      MOUSEEVENTF_MOVE with ERROR_INVALID_PARAMETER (87) for every flavour - relative,
#      absolute, absolute+virtualdesk - on every attempt, and rejects LEFTDOWN INTERMITTENTLY
#      (refused, then accepted for two runs, then refused again, with the same game running
#      throughout), while LEFTUP, WHEEL and every keyboard event from the same process are
#      accepted in the same second. An instrument that only checks "did SendInput succeed" on
#      one event would send half a click - LEFTDOWN refused, LEFTUP accepted - and then measure
#      the result, with the product blameless and the number meaningless. Movement therefore
#      goes through SetCursorPos, which is not injection and is not filtered; and
#      Assert-InputWorks below refuses to measure anything until a full button round trip has
#      been accepted. It is a precondition and not a guarantee: the block can return mid-run,
#      and when it does every call throws rather than returning a wrong answer quietly.
#
#   5. A PERSON USING THE MOUSE BREAKS NO CALL AND RUINS EVERY MEASUREMENT. Measured on
#      2026-09-19: SetCursorPos lands exactly, every time, read back immediately - and a hand on
#      the mouse moves the pointer away again within 200 ms, ~4000 px of drift in six seconds
#      against 0 px on an idle machine. Nothing fails. No error is returned. The press simply
#      happens somewhere else, and whatever was under the pointer gets reported as a verdict
#      about the shelf. This is the one defect here that is silent by nature, which makes it
#      the worst of the five: defect 4 at least shouts. Assert-InputWorks samples the pointer
#      for one second before the run and refuses to start if anything else is driving it, and
#      Move-Pointer re-checks arrival before every press so the mid-run case is loud too.
#      THE USUAL CULPRIT IS NOT A HAND. Measured twice on 2026-09-19, the thing holding the
#      pointer was a GAME - an Unreal client covering the monitor at 2560x1440 - whose mouse
#      capture re-centres the cursor, which is why the pointer kept ending on exactly 1280,720,
#      the precise centre of the screen. A round-numbered resting position is the tell. That
#      case fails precondition 3 at the same time and has a different remedy (close the game,
#      not let go of the mouse), so the message names the foreground window and says which.
#
#   6. AN EVENT TAKES THE OPEN FRAME AWAY, AND THE PRODUCT IS RIGHT TO LET IT. A volume key or a
#      track change gets its own short HUD shape, and that shape replaces an open widget frame.
#      A run that pages to the shelf and then clicks can lose the frame in between: measured on
#      2026-09-20 with music playing, and the run reported "the shelf never appeared", which
#      points at the catcher and the pipe rather than at Spotify. The page is now re-checked
#      immediately before the press, and a frame that has gone is re-opened rather than blamed.
#      See the click step for the log lines.
#      It also cost this file a misdiagnosis worth keeping: the old check slept 200 ms and then
#      read the cursor once, so it blamed "the pointer did not move" - pointing at defect 1,
#      which has nothing to do with it - for a pointer that had moved perfectly and then been
#      shoved aside. Two causes, opposite remedies, one message. They are separate now.
# ------------------------------------------------------------------------------------------
#
# FOUR PRECONDITIONS, each of which produced a wasted run before it was written down:
#
#   1. THE SESSION MUST BE CONNECTED. A disconnected or locked session has no desktop, and
#      BitBlt fails for any window in it. `qwinsta` must show this session Active.
#
#   2. INPUT INJECTION MUST ACTUALLY REACH THE DESKTOP. See defect 4.
#
#   3. NOTHING MAY COVER THE MONITOR. OsdHost.OnForegroundCoversMonitorChanged hides the notch
#      outright whenever the foreground window covers the screen - that is the designed
#      fallback, not a fault. A maximised terminal is enough to trigger it, so a driver cannot
#      simply run from one. This script opens a small 320x200 window of its own and lets it take
#      the foreground. It does NOT minimise anything belonging to the person: a fullscreen game
#      that keeps reclaiming the foreground is reported as a precondition failure instead, which
#      is the honest answer.
#
#   4. THE POINTER MUST BE FREE. This script drives the real pointer and cannot share it with a
#      person. See defect 5. A run takes about a minute; hands off the mouse for that minute.

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-drive-pair",
    [string]$Configuration = 'Debug'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$plithExe = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0\Plith.exe"
$catcherExe = Join-Path (Split-Path -Parent $plithExe) 'Plith.DropCatcher.exe'
$plithLog = Join-Path $env:LOCALAPPDATA 'Plith\plith.log'
$catcherLog = Join-Path $env:LOCALAPPDATA 'Plith\dropcatcher.log'
$storePath = Join-Path $env:LOCALAPPDATA 'Plith\shelf.txt'
$fixtureDir = Join-Path $env:TEMP 'plith-shelf-fixtures'

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class PairInput {
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
    [DllImport("user32.dll", SetLastError=true)] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();

    const uint LEFTDOWN=0x0002, LEFTUP=0x0004, RIGHTDOWN=0x0008, RIGHTUP=0x0010, WHEEL=0x0800;
    const uint KEYUP=0x0002;

    // The whole point of defect 4: a refusal must be loud. SendInput reports how many events it
    // accepted, and a partially accepted batch is worse than none - it leaves a button down.
    static void Send(INPUT[] a, string what) {
        uint n = SendInput((uint)a.Length, a, Marshal.SizeOf(typeof(INPUT)));
        if (n != a.Length) throw new Exception("SendInput refused " + what + ": accepted " + n +
            " of " + a.Length + ", Win32 error " + Marshal.GetLastWin32Error() +
            " (87 with an anti-cheat driver loaded means injected input is being filtered)");
    }
    static INPUT Mouse(uint flags, uint data) {
        INPUT i = new INPUT();
        i.type = 0; i.u.mi.dwFlags = flags; i.u.mi.mouseData = data;
        return i;
    }

    // Movement is SetCursorPos, not SendInput. See defect 4.
    public static void Move(int x, int y) {
        if (!SetCursorPos(x, y))
            throw new Exception("SetCursorPos(" + x + "," + y + ") failed, Win32 error " +
                                Marshal.GetLastWin32Error());
    }
    public static void Glide(int x0, int y0, int x1, int y1, int steps) {
        for (int i = 1; i <= steps; i++) {
            Move(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
            System.Threading.Thread.Sleep(16);
        }
    }
    public static void LeftDown() { Send(new INPUT[] { Mouse(LEFTDOWN, 0) }, "LEFTDOWN"); }
    public static void LeftUp()   { Send(new INPUT[] { Mouse(LEFTUP, 0) }, "LEFTUP"); }
    public static void LeftClick() { LeftDown(); System.Threading.Thread.Sleep(70); LeftUp(); }
    public static void RightClick() {
        Send(new INPUT[] { Mouse(RIGHTDOWN, 0) }, "RIGHTDOWN");
        System.Threading.Thread.Sleep(70);
        Send(new INPUT[] { Mouse(RIGHTUP, 0) }, "RIGHTUP");
    }
    public static void Wheel(int notches) {
        Send(new INPUT[] { Mouse(WHEEL, unchecked((uint)(notches * 120))) }, "WHEEL");
    }
    public static void KeyDown(ushort vk) {
        INPUT d = new INPUT(); d.type = 1; d.u.ki.wVk = vk;
        Send(new INPUT[] { d }, "key down 0x" + vk.ToString("X"));
    }
    public static void KeyUp(ushort vk) {
        INPUT u = new INPUT(); u.type = 1; u.u.ki.wVk = vk; u.u.ki.dwFlags = KEYUP;
        Send(new INPUT[] { u }, "key up 0x" + vk.ToString("X"));
    }
    public static void Key(ushort vk) { KeyDown(vk); System.Threading.Thread.Sleep(40); KeyUp(vk); }
    public static string Where() {
        POINT p; GetCursorPos(out p);
        IntPtr h = WindowFromPoint(p);
        uint owner; GetWindowThreadProcessId(h, out owner);
        return "cursor " + p.X + "," + p.Y + " over hwnd=" + h + " pid=" + owner;
    }
}
public static class PairWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out PairInput.RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern IntPtr GetDesktopWindow();
    [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
    [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
    [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr d,int dx,int dy,int w,int h,IntPtr s,int sx,int sy,uint rop);
}
'@

# --- plumbing ---------------------------------------------------------------------------------

function Find-LayeredWindows {
    param([string]$ProcessLike)
    $script:found = @()
    $cb = [PairWin+EnumProc]{
        param($h, $l)
        $o = [uint32]0; [void][PairWin]::GetWindowThreadProcessId($h, [ref]$o)
        $pn = try { (Get-Process -Id $o -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -and $pn -like $ProcessLike -and [PairWin]::IsWindowVisible($h) -and
            ([PairWin]::GetWindowLongW($h, -20) -band 0x80000)) {
            $r = New-Object 'PairInput+RECT'; [void][PairWin]::GetWindowRect($h, [ref]$r)
            if (($r.Right - $r.Left) -gt 0) {
                $script:found += [pscustomobject]@{ Hwnd=$h; Pid=[int]$o; Proc=$pn
                    X=$r.Left; Y=$r.Top; W=($r.Right-$r.Left); H=($r.Bottom-$r.Top) }
            }
        }
        $true
    }
    [void][PairWin]::EnumWindows($cb, [IntPtr]::Zero)
    $script:found
}

function Find-LayeredWindow {
    param([string]$ProcessLike)
    Find-LayeredWindows -ProcessLike $ProcessLike | Select-Object -First 1
}

# The catcher owns more than one layered window: the notch stand-in it puts in Plith's place,
# and the drag-out window. Taking "the first layered catcher window" found the wrong one the
# moment a drag had run, and every check after it reported "not in the UIA tree" - which reads
# exactly like a page that had lost its accessible names. The shelf is the one whose tree
# announces itself, so that is what this asks for.
function Find-ShelfWindow {
    foreach ($w in Find-LayeredWindows -ProcessLike '*DropCatcher*') {
        try {
            if ((Get-Names -Hwnd $w.Hwnd) -match '^Shelf, ') { return $w }
        } catch { }
    }
    $null
}

# BOTH name and control type. A tile, its label and its tooltip all carry the same name, and a
# name-only search returned a text run in place of a tile once already.
function Get-Element {
    param($Hwnd, [string]$Name, [string]$Type = 'Custom')
    # Find-ShelfWindow returns $null when the shelf is not up, and `$shelf.Hwnd` on that $null
    # arrives here as $null too. [IntPtr]$null then fails with "Cannot convert null to type
    # System.IntPtr" - a cast error that names no window, no step and no control, and which
    # cost a whole run on 2026-09-20 before this said so out loud.
    if ($null -eq $Hwnd) {
        throw ("Get-Element was asked for '$Name' with a NULL window handle. The window it " +
               "belongs to was never found - for the shelf that means Find-ShelfWindow " +
               "returned nothing, so the shelf had closed or never opened.")
    }
    $el = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$Hwnd)
    $byName = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    $ct = switch ($Type) {
        'Custom' { [System.Windows.Automation.ControlType]::Custom }
        'Button' { [System.Windows.Automation.ControlType]::Button }
        'Text'   { [System.Windows.Automation.ControlType]::Text }
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

function Get-Names {
    param($Hwnd)
    if ($null -eq $Hwnd) {
        throw 'Get-Names was given a NULL window handle: the window it belongs to was never found.'
    }
    $el = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$Hwnd)
    $all = $el.FindAll([System.Windows.Automation.TreeScope]::Descendants,
                       [System.Windows.Automation.Condition]::TrueCondition)
    @($all | ForEach-Object { $_.Current.Name } | Where-Object { $_ })
}

function Move-Pointer {
    param([int]$X, [int]$Y, [int]$Settle = 350)
    $p = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$p)
    [PairInput]::Glide($p.X, $p.Y, $X, $Y, 16)
    [PairInput]::Glide($X, $Y, ($X + 1), $Y, 2)
    [PairInput]::Glide(($X + 1), $Y, $X, $Y, 2)
    Start-Sleep -Milliseconds $Settle

    # Defect 5's mid-run half. Assert-InputWorks proves nobody else owned the pointer at the
    # START of the run; it cannot promise that for the minute that follows. Every caller below
    # presses immediately after this returns, so a pointer that has been nudged elsewhere turns
    # into a press on the wrong thing and a verdict about the wrong control - silently, with the
    # product blameless, which is the one property this file's header says an instrument must
    # never have. Checking arrival costs one call and makes that case loud instead.
    $at = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$at)
    if ([Math]::Abs($at.X - $X) -gt 2 -or [Math]::Abs($at.Y - $Y) -gt 2) {
        throw ("The pointer was sent to $X,$Y and is at $($at.X),$($at.Y) instead. Something " +
               "moved it during the $Settle ms settle - almost always a hand on the mouse. " +
               "Nothing measured after this point would be about the shelf, so the run stops " +
               "here rather than pressing whatever is under the pointer now.")
    }
}

# A press, a glide well past SystemParameters.MinimumHorizontalDragDistance, and a release.
# DoDragDrop pumps its own modal message loop inside the catcher, so the release has to arrive
# as a real message rather than as a state change - which is why the glide is stepped and the
# pauses are generous.
function Invoke-Drag {
    param([int]$FromX, [int]$FromY, [int]$ToX, [int]$ToY)
    Move-Pointer -X $FromX -Y $FromY -Settle 450
    [PairInput]::LeftDown()
    Start-Sleep -Milliseconds 140
    [PairInput]::Glide($FromX, $FromY, $ToX, $ToY, 28)
    Start-Sleep -Milliseconds 500
    [PairInput]::LeftUp()
    Start-Sleep -Milliseconds 900
}

function Save-Shot {
    param([int]$X,[int]$Y,[int]$W,[int]$H,[string]$Name)
    $path = Join-Path $OutDir $Name
    $bmp = New-Object System.Drawing.Bitmap $W, $H
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dst = $g.GetHdc()
    $desk = [PairWin]::GetDesktopWindow(); $src = [PairWin]::GetDC($desk)
    $ok = [PairWin]::BitBlt($dst, 0, 0, $W, $H, $src, $X, $Y, (0x00CC0020 -bor 0x40000000))
    [void][PairWin]::ReleaseDC($desk, $src); $g.ReleaseHdc($dst); $g.Dispose()
    if (-not $ok) {
        $bmp.Dispose()
        $state = (qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }) -replace '\s+', ' '
        throw ("BitBlt failed for ${W}x${H} at $X,$Y.`n    Current session: $state`n" +
               "    A disconnected or locked session has no desktop to capture. Reconnect and re-run.")
    }
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $path
}

# One flat list, newest first. The file was blank-line separated groups while the shelf had
# stacks; it is one path per line now, and a blank line is skipped rather than meaningful.
function Read-Shelf {
    if (-not (Test-Path $storePath)) { return @() }
    @(Get-Content $storePath |
        Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
        ForEach-Object { [System.IO.Path]::GetFileName($_) })
}

function Format-Shelf {
    param($Items)
    if (-not $Items -or @($Items).Count -eq 0) { return '(empty)' }
    '[' + (@($Items) -join ' ') + ']'
}

$script:verdicts = @()
$script:failed = 0
function Add-Verdict {
    param([string]$Name, [bool]$Ok, [string]$Detail)
    $script:verdicts += "  [$(if ($Ok) { 'PASS' } else { 'FAIL' })] $Name - $Detail"
    if (-not $Ok) { $script:failed++ }
}

# --- preconditions ----------------------------------------------------------------------------

if (-not (Test-Path $plithExe)) { throw "Build $Configuration first: $plithExe not found." }
if (-not (Test-Path $catcherExe)) {
    throw ("$catcherExe is not beside Plith.exe. DropCatcherLauncher resolves the catcher by " +
           "that one rule, so Plith would report 'Drop catcher not found' and the shelf would " +
           "never open. Rebuild rather than copying it by hand.")
}

$session = qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }
if ($session -notmatch 'Active') {
    throw ("This session is not Active (`qwinsta` says: $($session -replace '\s+', ' ')). A " +
           "disconnected or locked session has no desktop: nothing can be captured and nothing " +
           "can be pressed. Reconnect and re-run.")
}
"session: $(($session -replace '\s+', ' ').Trim())"

function Assert-InputWorks {
    # Movement first, and proven by reading the cursor back rather than by a return value.
    #
    # DEFECT 5 LIVES HERE. This check used to sleep 200 ms and then read the cursor once, which
    # folds two unrelated failures into one message. If the call did not take effect, the remedy
    # is about DPI virtualization or a driver. If something else moved the pointer afterwards,
    # the remedy is to stop touching the mouse. The old message said "the pointer did not move"
    # for both, which is actively false in the second case - the pointer moved exactly where it
    # was told and then a hand moved it away - and it points the reader at defect 1, which is
    # not involved at all. Read back IMMEDIATELY to answer the first question; a sleep here
    # measures the wrong thing, because anything else touching the pointer lands in the gap.
    $before = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$before)
    [PairInput]::Move(400, 400)
    $landed = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$landed)
    if ([Math]::Abs($landed.X - 400) -gt 2 -or [Math]::Abs($landed.Y - 400) -gt 2) {
        throw ("SetCursorPos reported success but the pointer read back at " +
               "$($landed.X),$($landed.Y) instead of 400,400. The call itself is not taking " +
               "effect - suspect DPI virtualization of a non-aware host, or a driver rejecting " +
               "it. This is NOT the anti-cheat case (defect 4 refuses SendInput and leaves " +
               "SetCursorPos alone) and NOT defect 1.")
    }

    # Then: is anything ELSE driving the pointer? A person with a hand on the mouse does not
    # break any call here, which is exactly why it has to be measured separately - every press
    # below aims at a tile by coordinate, so a pointer that will not stay put produces verdicts
    # about whatever happened to be under it. Measured on 2026-09-19: an idle machine drifts 0 px
    # across six samples, and a hand on the mouse produced ~4000 px in six seconds.
    $drift = 0
    $prev = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$prev)
    for ($s = 0; $s -lt 10; $s++) {
        Start-Sleep -Milliseconds 100
        $now = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$now)
        $drift += [Math]::Abs($now.X - $prev.X) + [Math]::Abs($now.Y - $prev.Y)
        $prev = $now
    }
    if ($drift -gt 4) {
        # Name the culprit rather than guessing at it. The first version of this message said
        # "almost always this is a person using the mouse", and on this machine the measured
        # cause was twice a GAME holding the pointer - which has a different remedy and also
        # fails precondition 3 at the same time. A pointer parked on the exact centre of the
        # screen is the tell: that is mouse capture re-centring it, not a hand.
        $h = [PairInput]::GetForegroundWindow()
        $who = [uint32]0; [void][PairWin]::GetWindowThreadProcessId($h, [ref]$who)
        $name = (Get-Process -Id $who -ErrorAction SilentlyContinue).ProcessName
        $rect = New-Object 'PairInput+RECT'; [void][PairWin]::GetWindowRect($h, [ref]$rect)
        $scr = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        $full = ($rect.Right - $rect.Left) -ge $scr.Width -and ($rect.Bottom - $rect.Top) -ge $scr.Height
        $mid = "$([int]($scr.Width / 2)),$([int]($scr.Height / 2))"
        throw ("The pointer is being moved by something else: $drift px of drift over one " +
               "second while this script asked for none, ending at $($prev.X),$($prev.Y).`n" +
               "    Foreground window: '$name'$(if ($full) { ', and it COVERS THE MONITOR' }).`n" +
               "    $(if ($full) {
                        "That is a game or a fullscreen app holding the pointer, and it fails " +
                        "precondition 3 as well: OsdHost hides the notch outright while the " +
                        "foreground covers the screen, so there would be nothing to click even " +
                        "with the pointer free. Close it and re-run. (A pointer ending on or " +
                        "near $mid, the exact centre of the screen, is mouse capture " +
                        "re-centring it - that is a game, not a hand.)"
                    } else {
                        "That is usually a person with a hand on the mouse. Take your hand off " +
                        "it for about a minute and re-run."
                    })`n" +
               "    Either way: every step below aims at a tile by coordinate and presses, so a " +
               "run started now would press whatever the pointer had wandered onto and report " +
               "the result as a verdict about the shelf. This script drives the real pointer " +
               "and cannot share it.")
    }

    # Then a full button round trip, over an empty part of the desktop. Defect 4: LEFTUP and
    # WHEEL can be accepted while LEFTDOWN is refused, so checking one says nothing about the
    # other, and half a click is worse than no click.
    try {
        [PairInput]::LeftDown()
        Start-Sleep -Milliseconds 50
        [PairInput]::LeftUp()
    }
    catch {
        $guard = @(Get-Service -ErrorAction SilentlyContinue |
                   Where-Object { $_.Name -match 'vgc|EasyAnti|BEService' -and $_.Status -eq 'Running' } |
                   ForEach-Object { $_.Name }) -join ', '
        throw ("Injected mouse input is being refused: $($_.Exception.Message)`n" +
               "    Running input-filtering services: $(if ($guard) { $guard } else { 'none found by name' })`n" +
               "    An anti-cheat driver (Riot Vanguard's vgc, EasyAntiCheat, BattlEye) filters " +
               "SendInput process-wide while it is loaded, and a protected game being open " +
               "tightens it further. Close the game - and, for Vanguard, that means a reboot, " +
               "because its driver loads at boot - then re-run.")
    }
    [PairInput]::Move($before.X, $before.Y)
}
Assert-InputWorks
'input check passed: the pointer moves and a full click round trip is accepted.'

# --- the stage window -------------------------------------------------------------------------

$stageScript = Join-Path $OutDir 'stage-window.ps1'
Set-Content -Path $stageScript -Encoding UTF8 -Value @'
# A small foreground window, and nothing else. The notch hides itself whenever the foreground
# window covers the monitor, and a maximised terminal is enough to do that.
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$f = New-Object System.Windows.Forms.Form
$f.Text = 'Plith shelf driver - stage window'
$f.StartPosition = 'Manual'
$f.Location = New-Object System.Drawing.Point 60, 760
$f.Size = New-Object System.Drawing.Size 320, 200
$f.Add_Shown({ $f.Activate() })
[System.Windows.Forms.Application]::Run($f)
'@

function Stop-Stage {
    Get-Process -Name 'powershell' -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowTitle -like '*Plith shelf driver*' } |
        Stop-Process -Force -ErrorAction SilentlyContinue
}

Get-Process -Name 'Plith*' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Stop-Stage
Start-Sleep -Milliseconds 800

# VERIFY THE KILL, do not assume it. The line above silences its own errors, so a Plith that
# refuses to die leaves no trace - and Plith takes a per-user single-instance Mutex in
# Program.cs, so the instance this script starts next would exit IMMEDIATELY and silently.
# What the script then reported was "The drop catcher never connected within 30 s", which sends
# the reader to the pipe, the catcher and the ACL, none of which is at fault. Measured on
# 2026-09-20: an instance from three hours earlier was still alive, and produced exactly that.
$survivors = @(Get-Process -Name 'Plith*' -ErrorAction SilentlyContinue)
for ($i = 0; $i -lt 10 -and $survivors.Count -gt 0; $i++) {
    Start-Sleep -Milliseconds 300
    $survivors = @(Get-Process -Name 'Plith*' -ErrorAction SilentlyContinue)
}
if ($survivors.Count -gt 0) {
    throw ("A Plith process survived being stopped: " +
           "$(($survivors | ForEach-Object { "$($_.ProcessName) (pid $($_.Id), started $($_.StartTime))" }) -join ', ').`n" +
           "    Program.cs takes a per-user single-instance mutex, so the Plith this script " +
           "starts would exit at once and every check below would measure the OLD instance, or " +
           "nothing at all. Stop it by hand and re-run.")
}

$origin = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$origin)
$stage = Start-Process -FilePath 'powershell' `
    -ArgumentList '-NoProfile','-WindowStyle','Hidden','-File',$stageScript -PassThru
Start-Sleep -Seconds 3

$fg = [PairInput]::GetForegroundWindow()
$fgPid = [uint32]0; [void][PairWin]::GetWindowThreadProcessId($fg, [ref]$fgPid)
$fgRect = New-Object 'PairInput+RECT'; [void][PairWin]::GetWindowRect($fg, [ref]$fgRect)
$fgName = (Get-Process -Id $fgPid -ErrorAction SilentlyContinue).ProcessName
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$covers = ($fgRect.Right - $fgRect.Left) -ge $screen.Width -and ($fgRect.Bottom - $fgRect.Top) -ge $screen.Height
"foreground: $fgName ($fgPid) at $($fgRect.Left),$($fgRect.Top) $($fgRect.Right-$fgRect.Left)x$($fgRect.Bottom-$fgRect.Top)"
if ($covers) {
    Stop-Stage
    throw ("'$fgName' covers the monitor and took the foreground back from this script's own " +
           "window. OsdHost hides the notch outright while that is true, so there is nothing " +
           "to click. A fullscreen game does this within seconds and nothing here will win " +
           "against it. Close or minimise it yourself and re-run; this script deliberately " +
           "does not touch windows it did not open.")
}

# --- the run ------------------------------------------------------------------------------------

# Back up whatever the person actually had on the shelf, and put it back in the finally.
$savedShelf = if (Test-Path $storePath) { Get-Content $storePath -Raw } else { $null }

Remove-Item $fixtureDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null
# A FULL shelf, on purpose, and that is a change the flat design earned.
#
# The stack fixture was four files in two stacks of two, shaped entirely around what the surface
# would fold away: a column drew at most two tiles and anything past that was in no UIA tree, so
# a bigger fixture produced false alarms rather than more coverage. There is no fold now, so the
# fixture is the one case worth testing: the shelf at exactly its capacity, where every file must
# still be reachable. See the tree check below, which is the whole point of the change.
# NOT $names: the paging loop below reassigns that for the notch page's UIA names, and
# the capacity check further down would then measure the wrong list.
$fixtureNames = @('alpha.txt','bravo.txt','charlie.txt','delta.txt','echo.txt',
                  'foxtrot.txt','golf.txt','hotel.txt','india.txt','juliett.txt',
                  'kilo.txt','lima.txt','mike.txt','november.txt','oscar.txt')
foreach ($n in $fixtureNames) { Set-Content -Path (Join-Path $fixtureDir $n) -Value "fixture $n" -Encoding UTF8 }
Set-Content -Path $storePath -Encoding UTF8 -Value @($fixtureNames | ForEach-Object { Join-Path $fixtureDir $_ })
"seeded: $(Format-Shelf (Read-Shelf))"

$plithMark = @(Get-Content $plithLog -ErrorAction SilentlyContinue).Count
$catcherMark = @(Get-Content $catcherLog -ErrorAction SilentlyContinue).Count

try {
    [PairInput]::Move(600, 700)
    $proc = Start-Process -FilePath $plithExe -PassThru

    # Wait for the pair rather than sleeping a guessed amount: Plith's own log says when the
    # catcher has connected, and nothing below can work before it has.
    $deadline = [datetime]::UtcNow.AddSeconds(30)
    while ([datetime]::UtcNow -lt $deadline) {
        # Did the Plith we just started die on us? The single-instance mutex makes a second
        # instance exit at once, and waiting 30 s for a log line from a process that is already
        # gone reports the CATCHER as the failure. Ask the process, not the log.
        if ($proc.HasExited) {
            throw ("The Plith this script started exited immediately (exit code " +
                   "$($proc.ExitCode)). That is what a second instance does: Program.cs takes " +
                   "a per-user single-instance mutex. Another Plith is running that the kill " +
                   "above did not remove. Nothing here is about the drop catcher or the pipe.")
        }
        $tail = @(Get-Content $plithLog -ErrorAction SilentlyContinue) | Select-Object -Skip $plithMark
        if ($tail -match 'Drop catcher connected') { break }
        Start-Sleep -Milliseconds 250
    }
    if (-not (@(Get-Content $plithLog) | Select-Object -Skip $plithMark | Select-String 'Drop catcher connected')) {
        throw ("The drop catcher never connected within 30 s; the shelf cannot open. " +
               "Plith itself is $(if ($proc.HasExited) { 'NOT running - it exited' } else { 'still running' }). " +
               'See plith.log.')
    }
    'the pair is up and connected.'

    $notch = Find-LayeredWindow -ProcessLike 'Plith'
    if (-not $notch) { throw 'No visible layered Plith window. Something is covering the monitor.' }
    "notch window: $($notch.X),$($notch.Y) $($notch.W)x$($notch.H)"

    # 1. Click the notch open. OsdHost.OnNotchClicked turns a click into the widget frame; a
    #    HUD showing a volume or track change counts as closed for this and is clicked through.
    Move-Pointer -X ($notch.X + [int]($notch.W / 2)) -Y 4 -Settle 900
    [PairInput]::LeftClick()
    Start-Sleep -Milliseconds 1500
    Save-Shot 0 0 $screen.Width 300 '1-frame.png' | Out-Null

    # 2. Page to the shelf widget. One wheel notch is exactly NotchPager.CommitThreshold (120)
    #    and IdleRearmMs is 150, so a notch every 700 ms is one page with the accumulator rested
    #    in between. A plain vertical wheel pages (WheelDecoder negates it) and DOWN is "next".
    #    The shelf page is installed last, so at most one full lap is needed.
    #
    #    The page is recognised by ShelfWidget's own OpenHint text, "Click to open the shelf",
    #    and not by the row's announcement. That announcement used to reach nothing at all -
    #    "Shelf, N items" was set on `Tiles`, a StackPanel, which WPF gives no automation peer -
    #    and it has since been moved onto the control root, so it IS in the tree now. The hint is
    #    still the better key: it names the page's purpose rather than its contents, so it does
    #    not change as files come and go. See docs/SHELF-VERIFICATION.md section 5.4.
    $onShelf = $false
    for ($i = 1; $i -le 6; $i++) {
        $names = Get-Names -Hwnd $notch.Hwnd
        if ($names -contains 'Click to open the shelf') { $onShelf = $true; break }
        [PairInput]::Wheel(-1)
        Start-Sleep -Milliseconds 700
    }
    Add-Verdict 'the notch pages to the shelf widget' $onShelf `
        "names: $((Get-Names -Hwnd $notch.Hwnd) -join ' | ')"
    Save-Shot 0 0 $screen.Width 300 '2-shelf-page.png' | Out-Null
    if (-not $onShelf) { throw 'Never reached the shelf page; nothing below can run.' }

    # 3. Click the page, which is what ShelfWidget turns into OpenRequested.
    #
    # INSTRUMENT DEFECT 6: AN EVENT CAN TAKE THE FRAME AWAY BETWEEN PAGING AND CLICKING, and the
    # product is right to let it. A volume key or a track change does not open the widget frame
    # any more, it gets its own short HUD shape - and that shape REPLACES an open frame. Measured
    # on 2026-09-20, from Plith's own log, with the click landing in the gap:
    #
    #     12:57:35.489  Widget page committed: index=3/4     <- the shelf page, reached
    #     12:57:36.451  Reposition: content=400x130          <- the HUD took the frame
    #     12:57:36.684  Reposition: content=400x68           <- back to the parked notch
    #
    # The click then hit nothing, and the run reported "the shelf never appeared", which points
    # at the catcher and the pipe. Neither was involved. Spotify was.
    #
    # So the page is re-checked IMMEDIATELY before the press, and a frame that has gone is
    # re-opened and re-paged rather than reported as a product failure. The press is still one
    # press: this retries reaching the page, never the verdict.
    $shelf = $null
    for ($attempt = 1; $attempt -le 3 -and -not $shelf; $attempt++) {
        $frame = Find-LayeredWindow -ProcessLike 'Plith'
        $stillThere = $frame -and (@(Get-Names -Hwnd $frame.Hwnd) -contains 'Click to open the shelf')

        if (-not $stillThere) {
            "  attempt ${attempt}: an event replaced the frame before the click; re-opening"
            Move-Pointer -X ($notch.X + [int]($notch.W / 2)) -Y 4 -Settle 900
            [PairInput]::LeftClick()
            Start-Sleep -Milliseconds 1500
            for ($i = 1; $i -le 6; $i++) {
                $frame = Find-LayeredWindow -ProcessLike 'Plith'
                if ($frame -and (@(Get-Names -Hwnd $frame.Hwnd) -contains 'Click to open the shelf')) { break }
                [PairInput]::Wheel(-1)
                Start-Sleep -Milliseconds 700
            }
            $stillThere = $frame -and (@(Get-Names -Hwnd $frame.Hwnd) -contains 'Click to open the shelf')
            if (-not $stillThere) { continue }
        }

        Move-Pointer -X ($frame.X + [int]($frame.W / 2)) -Y ($frame.Y + [int]($frame.H / 2)) -Settle 500
        [PairInput]::LeftClick()
        Start-Sleep -Seconds 2
        $shelf = Find-ShelfWindow
    }

    Add-Verdict '2.2 a click on the shelf page opens the shelf' ([bool]$shelf) `
        "$(if ($shelf) { "$($shelf.X),$($shelf.Y) $($shelf.W)x$($shelf.H)" } else { 'no catcher window after 3 attempts' })"
    if (-not $shelf) { throw 'The shelf never appeared; nothing below can run.' }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '3-shelf.png' | Out-Null

    # Onto the shelf AT ONCE, so LeaveGrace is never armed for the rest of the run.
    Move-Pointer -X ($shelf.X + 20) -Y ($shelf.Y + 8) -Settle 400

    $items = Read-Shelf
    Add-Verdict 'the shelf arrives holding everything that was seeded' `
        (@($items).Count -eq $fixtureNames.Count) (Format-Shelf $items)

    # NO FOLD, so no rule about one. A stack column used to draw at most two tiles and hide the
    # rest behind a count chip, and the arithmetic for that shaped every step in this file. It
    # also cost two runs to state correctly: "at most two" was wrong, because the chip cost a
    # SLOT, so a stack of three drew exactly ONE. The flat shelf draws everything it holds, so
    # any tile the steps below need is drawn by construction.

    # --- THE GUARANTEE THE FLAT SHELF EXISTS FOR ---------------------------------------------
    #
    # Every file on the shelf is on the screen, in the UIA tree, and reachable by a key.
    #
    # This is the one check that would have FAILED on the stack build, and it is why that build
    # is gone. ShelfStore capped the shelf at 20 while the surface could draw 10: five columns of
    # two, with everything past that folded into a "+N" chip. A folded tile is in no UIA tree, so
    # it is invisible to a screen reader and reachable by no key. Half of a full shelf was
    # unreachable, and nothing in the build, the tests or the lints could see it.
    #
    # The cap is now defined as exactly what the grid draws (NotchGeometry.ShelfCapacity), so the
    # two cannot drift apart. This check is what would notice if a fold ever came back.
    $shelf = Find-ShelfWindow
    $drawn = @(Get-Names -Hwnd $shelf.Hwnd)
    $missing = @($fixtureNames | Where-Object { $drawn -notcontains $_ })
    Add-Verdict 'every file on a FULL shelf is in the UIA tree' ($missing.Count -eq 0) `
        "$($fixtureNames.Count) seeded, missing: $(if ($missing.Count) { $missing -join ', ' } else { 'none' })"
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '4-full-shelf.png' | Out-Null

    # --- 3.1 second half: the hover remove control takes the row out of shelf.txt -----------------
    $shelf = Find-ShelfWindow
    $target = Get-Element -Hwnd $shelf.Hwnd -Name 'charlie.txt'
    if ($target) {
        # At the tile CENTRE, which is where a person aims and which was dead until section 3.10.
        Move-Pointer -X $target.CX -Y $target.CY -Settle 600
        $remove = Get-Element -Hwnd $shelf.Hwnd -Name 'Remove charlie.txt from the shelf' -Type Button
        if ($remove) {
            $before = Read-Shelf
            Move-Pointer -X $remove.CX -Y $remove.CY -Settle 400
            [PairInput]::LeftClick()
            Start-Sleep -Milliseconds 1400
            $after = Read-Shelf
            $gone = @($after) -notcontains 'charlie.txt'
            $stillDrawn = @(Get-Names -Hwnd (Find-ShelfWindow).Hwnd) -contains 'charlie.txt'
            Add-Verdict '3.1 remove takes the tile off the page AND out of shelf.txt' `
                ($gone -and -not $stillDrawn) `
                "before $(Format-Shelf $before) -> after $(Format-Shelf $after); still drawn: $stillDrawn"
        } else {
            Add-Verdict '3.1 remove takes the tile off the page AND out of shelf.txt' $false `
                'the hover remove control never appeared at the tile centre'
        }
    } else {
        Add-Verdict '3.1 remove takes the tile off the page AND out of shelf.txt' $false 'charlie.txt not in the UIA tree'
    }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '7-removed.png' | Out-Null

    # --- 3.2 remove acts on the SELECTION, not on the one tile the control sits on -----------------
    # Two tiles in one flat list. The written item asks for two in the SAME stack; there are no
    # stacks, so "the same stack" has no meaning and the rule under test (ShelfModel.DragPaths)
    # never depended on columns anyway. The item's second half, removing a tile that is NOT part
    # of any selection, is still not driven here.
    $shelf = Find-ShelfWindow
    $first = Get-Element -Hwnd $shelf.Hwnd -Name 'delta.txt'
    $second = Get-Element -Hwnd $shelf.Hwnd -Name 'alpha.txt'
    if ($first -and $second) {
        Move-Pointer -X $first.CX -Y $first.CY -Settle 350
        [PairInput]::LeftClick()
        Start-Sleep -Milliseconds 400
        [PairInput]::KeyDown(0x11)                      # Ctrl
        Move-Pointer -X $second.CX -Y $second.CY -Settle 350
        [PairInput]::LeftClick()
        [PairInput]::KeyUp(0x11)
        Start-Sleep -Milliseconds 700

        $before = Read-Shelf
        $shelf = Find-ShelfWindow
        $remove = Get-Element -Hwnd $shelf.Hwnd -Name 'Remove alpha.txt from the shelf' -Type Button
        if ($remove) {
            Move-Pointer -X $remove.CX -Y $remove.CY -Settle 400
            [PairInput]::LeftClick()
            Start-Sleep -Milliseconds 1400
            $after = Read-Shelf
            $bothGone = (@($after) -notcontains 'delta.txt') -and (@($after) -notcontains 'alpha.txt')
            Add-Verdict '3.2 remove acts on the whole selection' $bothGone `
                "before $(Format-Shelf $before) -> after $(Format-Shelf $after)"
        } else {
            Add-Verdict '3.2 remove acts on the whole selection' $false `
                'the remove control did not appear on the second selected tile'
        }
    } else {
        Add-Verdict '3.2 remove acts on the whole selection' $false `
            "delta.txt found: $([bool]$first); alpha.txt found: $([bool]$second)"
    }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '8-selection-removed.png' | Out-Null

    # --- 3.13 emptying the shelf ONE BY ONE never puts a file back on it --------------------------
    #
    # Reported from a real session on 2026-09-21: emptying the shelf tile by tile flickers. The
    # empty state appears, files come back into it, and then it goes empty again.
    #
    # The count the surface reports to UI Automation is the count it drew, so polling that name
    # between the click and the settled state records what was on screen in between. A remove can
    # only ever make the shelf smaller, so the sequence must be NON-INCREASING: one number going
    # up is a file that came back, which is the report.
    #
    # The polling is corroboration, not the primary evidence. A render is synchronous with the
    # Items message that causes it, so a flash is two renders close together and a 60 ms poll can
    # miss one; both processes now log every Items delivery with its count, and that record cannot
    # miss a message. Read the two together at the bottom of this report.
    $shelf = Find-ShelfWindow
    $rounds = @()
    $wentUp = @()
    $remaining = @(Read-Shelf)
    $guard = 0
    while ($remaining.Count -gt 0 -and $guard -lt 20) {
        $guard++
        $name = $remaining[0]
        $shelf = Find-ShelfWindow
        if (-not $shelf) { $rounds += "$name -> the shelf window disappeared"; break }

        $tile = Get-Element -Hwnd $shelf.Hwnd -Name $name
        if (-not $tile) { $rounds += "$name -> not in the UIA tree"; break }
        Move-Pointer -X $tile.CX -Y $tile.CY -Settle 450
        $remove = Get-Element -Hwnd $shelf.Hwnd -Name "Remove $name from the shelf" -Type Button
        if (-not $remove) { $rounds += "$name -> no remove control on hover"; break }

        Move-Pointer -X $remove.CX -Y $remove.CY -Settle 300
        [PairInput]::LeftClick()

        # A BURST OF CAPTURES, not only a poll of the tree, and the burst is the half that can
        # answer the report. The tree says what the page was asked to draw; the screen says what
        # was on it. Those differ exactly when something OTHER than this page is what flickers -
        # Plith's notch coming back, a HUD taking the frame, the catcher's stand-in - and every
        # one of those is a window this UIA walk never looks at.
        #
        # Taken first and fast, because the flash is reported as instant: twelve frames of the
        # whole top strip, roughly one every 70 ms, which covers the second after the press.
        $burstDir = Join-Path $OutDir "burst-$guard-$($name -replace '[^a-zA-Z0-9]', '')"
        New-Item -ItemType Directory -Force -Path $burstDir | Out-Null
        for ($f = 0; $f -lt 12; $f++) {
            $shot = Join-Path $burstDir ("f{0:d2}.png" -f $f)
            $bmp = New-Object System.Drawing.Bitmap $screen.Width, 340
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            $dst = $g.GetHdc()
            $desk = [PairWin]::GetDesktopWindow(); $src = [PairWin]::GetDC($desk)
            [void][PairWin]::BitBlt($dst, 0, 0, $screen.Width, 340, $src, 0, 0, (0x00CC0020 -bor 0x40000000))
            [void][PairWin]::ReleaseDC($desk, $src); $g.ReleaseHdc($dst); $g.Dispose()
            $bmp.Save($shot, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
            Start-Sleep -Milliseconds 70
        }

        # The handle is captured ONCE and reused for every poll. Re-finding the window each time
        # costs a window enumeration plus a UIA walk, which is far slower than the flash being
        # looked for, so the search itself would hide it.
        $hwnd = $shelf.Hwnd
        $seen = @()
        for ($t = 0; $t -lt 20; $t++) {
            $label = try {
                @(Get-Names -Hwnd $hwnd) | Where-Object { $_ -match '^Shelf, ' } | Select-Object -First 1
            } catch { $null }
            if (-not $label) { $label = '(gone)' }
            if ($seen.Count -eq 0 -or $seen[-1] -ne $label) { $seen += $label }
            Start-Sleep -Milliseconds 60
        }

        $counts = @($seen | ForEach-Object {
            if ($_ -match '^Shelf, (\d+) item') { [int]$matches[1] } else { -1 }
        })
        for ($i = 1; $i -lt $counts.Count; $i++) {
            if ($counts[$i] -ge 0 -and $counts[$i - 1] -ge 0 -and $counts[$i] -gt $counts[$i - 1]) {
                $wentUp += "removing $name : $($counts[$i - 1]) -> $($counts[$i])"
            }
        }
        $rounds += "$name -> $($seen -join ' | ')"
        $remaining = @(Read-Shelf)
    }

    Add-Verdict '3.13 emptying one by one never puts a file back on the shelf' ($wentUp.Count -eq 0) `
        "$(if ($wentUp.Count) { $wentUp -join ' ; ' } else { "$guard removal(s), every count non-increasing" })"
    foreach ($r in $rounds) { "    $r" }
    Add-Verdict '3.13 one-by-one removal does empty the shelf' (@(Read-Shelf).Count -eq 0) `
        "store now $(Format-Shelf (Read-Shelf))"
    Save-Shot 0 0 $screen.Width 400 '10-emptied-one-by-one.png' | Out-Null

    # --- 3.3 clear empties the shelf, and asks nothing ---------------------------------------------
    $shelf = Find-ShelfWindow
    $clear = Get-Element -Hwnd $shelf.Hwnd -Name 'Clear the shelf' -Type Button
    if ($clear) {
        $before = Read-Shelf
        Move-Pointer -X $clear.CX -Y $clear.CY -Settle 400
        [PairInput]::LeftClick()
        Start-Sleep -Milliseconds 1400
        $after = Read-Shelf
        # No dialog: a confirmation would be a window of its own, so this looks for one rather
        # than trusting the file to imply its absence.
        $dialog = @(Find-LayeredWindows -ProcessLike '*DropCatcher*') | Where-Object { $_.W -lt 200 }
        Add-Verdict '3.3 clear empties the shelf without asking' `
            (($after.Count -eq 0) -and -not $dialog) `
            "before $(Format-Shelf $before) -> after $(Format-Shelf $after); confirmation window: $([bool]$dialog)"
    } else {
        Add-Verdict '3.3 clear empties the shelf without asking' $false "'Clear the shelf' is not in the UIA tree"
    }
    Save-Shot 0 0 $screen.Width 400 '9-cleared.png' | Out-Null
}
finally {
    Get-Process -Name 'Plith*' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Stop-Stage
    if ($savedShelf -ne $null) { Set-Content -Path $storePath -Value $savedShelf -NoNewline -Encoding UTF8 }
    else { Remove-Item $storePath -Force -ErrorAction SilentlyContinue }
    [PairInput]::Move($origin.X, $origin.Y)

    # THE REPORT LIVES INSIDE THE FINALLY, and that is the whole point. It used to sit after it,
    # so any exception raised in the try - one null window handle was enough - propagated
    # straight past every accumulated verdict, both process logs and the path to the shots, and
    # printed none of them. A run that got most of the way through reported one line about an
    # IntPtr cast and nothing about what had already passed, which is indistinguishable from a
    # run that never started. Measured on 2026-09-20. The evidence existed; the script threw it
    # away. Now the report prints on every path and the exception surfaces after it.
    ''
    '=== verdicts ==='
    $script:verdicts
    ''
    '--- plith log for this run ---'
    @(Get-Content $plithLog -ErrorAction SilentlyContinue) | Select-Object -Skip $plithMark |
        Where-Object { $_ -match 'Shelf|Widget page|Notch' } | ForEach-Object { "  $_" }
    '--- catcher log for this run ---'
    @(Get-Content $catcherLog -ErrorAction SilentlyContinue) | Select-Object -Skip $catcherMark |
        ForEach-Object { "  $_" }
    ''
    "Shots in $OutDir. LOOK AT THEM: shelf.txt says which paths are where, and never says the " +
    "surface looked right while saying it."
}

if ($script:failed -gt 0) { throw "$($script:failed) shelf check(s) FAILED." }
'Done.'
