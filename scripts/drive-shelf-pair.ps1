# scripts/drive-shelf-pair.ps1 - drives the REAL Plith + drop catcher pair, and judges what
# happened by reading the file Plith writes.
#
# WHY THIS EXISTS, AND WHY drive-shelf.ps1 IS NOT ENOUGH. That script drives the catcher in
# --shelfprobe mode, where App's CatcherClient is null: RemoveItems, ClearShelf, NewStack and
# Restack are raised and go nowhere, so the page never changes in response to them. Plith owns
# the shelf model. Every item in docs/SHELF-VERIFICATION.md section 3 whose expectation is "the
# tile disappears" or "shelf.txt no longer lists it" therefore needs both processes, connected,
# with the shelf opened the way a person opens it: a click on the notch.
#
# THE ORACLE IS shelf.txt, NOT THE PICTURE. Plith writes %LOCALAPPDATA%\Plith\shelf.txt on every
# change - one path per line, newest first, a blank line between stacks (ShelfStore.Save). A
# pixel count says something changed and never says the right thing changed; the file says which
# paths are on the shelf and how they are grouped, which is exactly what sections 3.1 to 3.6 ask
# about. Every check below is written against it, and the screenshots are kept so a person can
# see what the file is describing.
#
# WHAT THIS SCRIPT TOUCHES, AND PUTS BACK. It replaces shelf.txt with a known fixture set and
# restores the original afterwards; it starts Plith from the build output and stops it; and it
# opens a small window of its own (see PRECONDITION 3) and closes it. It does not touch any
# window the person had open.
#
# ------------------------------------------------------------------------------------------
# THE FOUR INSTRUMENT DEFECTS. The first three are drive-shelf.ps1's and are repeated because
# they are properties of this machine rather than of that file. The fourth was found here.
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
# ------------------------------------------------------------------------------------------
#
# THREE PRECONDITIONS, each of which produced a wasted run before it was written down:
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

# Stacks as the file records them: one path per line, a blank line between stacks.
function Read-Shelf {
    if (-not (Test-Path $storePath)) { return @() }
    $stacks = @(); $current = @()
    foreach ($line in @(Get-Content $storePath)) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            if ($current.Count -gt 0) { $stacks += ,$current; $current = @() }
            continue
        }
        $current += [System.IO.Path]::GetFileName($line)
    }
    if ($current.Count -gt 0) { $stacks += ,$current }
    ,$stacks
}

function Format-Shelf {
    param($Stacks)
    if (-not $Stacks -or $Stacks.Count -eq 0) { return '(empty)' }
    (($Stacks | ForEach-Object { '[' + ($_ -join ' ') + ']' }) -join ' ')
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
    $before = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$before)
    [PairInput]::Move(400, 400)
    Start-Sleep -Milliseconds 200
    $after = New-Object 'PairInput+POINT'; [void][PairInput]::GetCursorPos([ref]$after)
    if ([Math]::Abs($after.X - 400) -gt 2 -or [Math]::Abs($after.Y - 400) -gt 2) {
        throw ("The pointer did not move (it is at $($after.X),$($after.Y), not 400,400). " +
               "Measure nothing with a broken instrument - see defect 1 in this file's header.")
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
$names = @('alpha.txt','bravo.txt','charlie.txt','delta.txt')
foreach ($n in $names) { Set-Content -Path (Join-Path $fixtureDir $n) -Value "fixture $n" -Encoding UTF8 }
# Two stacks of TWO. Two because 3.5 restacks between them and 3.6 needs a last one to drag
# past; two each because a column draws at most two tiles before folding the rest into a "+N"
# chip, and a tile inside that chip is in no UIA tree and cannot be pressed.
Set-Content -Path $storePath -Encoding UTF8 -Value @(
    (Join-Path $fixtureDir 'alpha.txt'),
    (Join-Path $fixtureDir 'bravo.txt'),
    '',
    (Join-Path $fixtureDir 'charlie.txt'),
    (Join-Path $fixtureDir 'delta.txt')
)
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
        $tail = @(Get-Content $plithLog -ErrorAction SilentlyContinue) | Select-Object -Skip $plithMark
        if ($tail -match 'Drop catcher connected') { break }
        Start-Sleep -Milliseconds 250
    }
    if (-not (@(Get-Content $plithLog) | Select-Object -Skip $plithMark | Select-String 'Drop catcher connected')) {
        throw 'The drop catcher never connected within 30 s; the shelf cannot open. See plith.log.'
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
    #    and NOT by the row's announcement. Measured on 2026-09-19: ShelfWidget sets
    #    "Shelf, N items" on `Tiles`, which is a StackPanel, and a StackPanel has no automation
    #    peer - so that name is in no UIA tree and nothing can key off it. See
    #    docs/SHELF-VERIFICATION.md section 3.11.
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
    $frame = Find-LayeredWindow -ProcessLike 'Plith'
    Move-Pointer -X ($frame.X + [int]($frame.W / 2)) -Y ($frame.Y + [int]($frame.H / 2)) -Settle 500
    [PairInput]::LeftClick()
    Start-Sleep -Seconds 2

    $shelf = Find-ShelfWindow
    Add-Verdict '2.2 a click on the shelf page opens the shelf' ([bool]$shelf) `
        "$(if ($shelf) { "$($shelf.X),$($shelf.Y) $($shelf.W)x$($shelf.H)" } else { 'no catcher window' })"
    if (-not $shelf) { throw 'The shelf never appeared; nothing below can run.' }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '3-shelf.png' | Out-Null

    # Onto the shelf AT ONCE, so LeaveGrace is never armed for the rest of the run.
    Move-Pointer -X ($shelf.X + 20) -Y ($shelf.Y + 8) -Settle 400

    $stacks = Read-Shelf
    Add-Verdict 'the shelf arrives holding the seeded stacks' `
        ($stacks.Count -eq 2 -and $stacks[0].Count -eq 2 -and $stacks[1].Count -eq 2) (Format-Shelf $stacks)

    # THE TWO-TILE RULE, which shapes every step below and cost a run to learn. A stack column
    # draws at most TWO tiles and folds the rest into one "+N" chip named "N more items in this
    # stack". A third item in a stack is therefore not in the UIA tree at all, and asking for it
    # by name reports "not found" - which reads exactly like a page that has lost its accessible
    # names. The fixture is four files in two stacks of two, and the order of the steps keeps
    # every tile a later step needs down to at most the second row of its column.
    #
    # The steps also run in an order where each one's outcome is the next one's input, so the
    # expected shelf is computable at every point rather than re-seeded - which is not possible
    # mid-run anyway, since Plith loads the store once at startup.

    # --- 3.5 restack: drag a tile onto another existing stack -----------------------------------
    # charlie is the first tile of stack 2, so it is drawn. Restacking prepends, measured.
    $charlie = Get-Element -Hwnd $shelf.Hwnd -Name 'charlie.txt'
    $stack1 = Get-Element -Hwnd $shelf.Hwnd -Name 'Stack 1, 2 items'
    if ($charlie -and $stack1) {
        Invoke-Drag -FromX $charlie.CX -FromY $charlie.CY -ToX $stack1.CX -ToY $stack1.CY
        $after = Read-Shelf
        $moved = ($after.Count -ge 1) -and ($after[0] -contains 'charlie.txt') -and
                 -not (@($after | Select-Object -Skip 1) | Where-Object { $_ -contains 'charlie.txt' })
        Add-Verdict '3.5 dragging a tile onto another stack restacks it' $moved (Format-Shelf $after)
    } else {
        Add-Verdict '3.5 dragging a tile onto another stack restacks it' $false `
            "could not locate charlie.txt ($([bool]$charlie)) or Stack 1 ($([bool]$stack1)) in the UIA tree"
    }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '4-restacked.png' | Out-Null

    # --- 3.6 drag past the last stack starts a new one -------------------------------------------
    # Stack 1 is now [charlie alpha bravo]; alpha is its second tile and still drawn.
    $shelf = Find-ShelfWindow
    $before = Read-Shelf
    $alpha = Get-Element -Hwnd $shelf.Hwnd -Name 'alpha.txt'
    if ($alpha) {
        # The empty area to the RIGHT of the last column, still inside the window so the pointer
        # never leaves the shelf and arms the leave timer.
        Invoke-Drag -FromX $alpha.CX -FromY $alpha.CY `
                    -ToX ($shelf.X + $shelf.W - 26) -ToY ($shelf.Y + [int]($shelf.H / 2))
        $after = Read-Shelf
        $grew = $after.Count -gt $before.Count -and (@($after | Select-Object -Last 1) -contains 'alpha.txt')
        Add-Verdict '3.6 dragging past the last stack starts a new one' $grew `
            "before $(Format-Shelf $before) -> after $(Format-Shelf $after)"
    } else {
        Add-Verdict '3.6 dragging past the last stack starts a new one' $false 'alpha.txt not in the UIA tree'
    }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '5-new-stack-by-drag.png' | Out-Null

    # --- 3.4 the plus control makes a stack, and a drag lands in it -------------------------------
    # bravo is now the second tile of stack 1 ([charlie bravo]), so its source stack survives the
    # move and the stack count genuinely grows.
    $shelf = Find-ShelfWindow
    $plus = Get-Element -Hwnd $shelf.Hwnd -Name 'Start a new stack' -Type Button
    if ($plus) {
        $before = Read-Shelf
        Move-Pointer -X $plus.CX -Y $plus.CY -Settle 350
        [PairInput]::LeftClick()
        Start-Sleep -Milliseconds 900

        # An empty stack writes NOTHING to the file - ShelfStore.Save skips empty stacks - so the
        # only honest evidence for the click on its own is the page announcing one.
        $shelf = Find-ShelfWindow
        $emptyName = @(Get-Names -Hwnd $shelf.Hwnd) | Where-Object { $_ -match '^Stack \d+, 0 items' } | Select-Object -First 1
        $slot = if ($emptyName) { Get-Element -Hwnd $shelf.Hwnd -Name $emptyName } else { $null }
        $bravo = Get-Element -Hwnd $shelf.Hwnd -Name 'bravo.txt'
        if ($bravo -and $slot) {
            Invoke-Drag -FromX $bravo.CX -FromY $bravo.CY -ToX $slot.CX -ToY $slot.CY
            $after = Read-Shelf
            $landed = ($after.Count -gt $before.Count) -and
                      ((@($after | Where-Object { $_ -contains 'bravo.txt' })).Count -eq 1)
            Add-Verdict '3.4 a new stack, then a drag lands in it' $landed `
                "empty stack announced as '$emptyName'; before $(Format-Shelf $before) -> after $(Format-Shelf $after)"
        } else {
            Add-Verdict '3.4 a new stack, then a drag lands in it' $false `
                "empty stack announced as '$emptyName'; bravo.txt found: $([bool]$bravo); empty column found: $([bool]$slot)"
        }
    } else {
        Add-Verdict '3.4 a new stack, then a drag lands in it' $false "'Start a new stack' is not in the UIA tree"
    }
    Save-Shot $shelf.X $shelf.Y $shelf.W $shelf.H '6-new-stack-by-button.png' | Out-Null

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
            $gone = -not (@($after | Where-Object { $_ -contains 'charlie.txt' }))
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
    # Three single-item stacks are left, so both tiles are drawn and neither is in an overflow.
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
            $bothGone = -not (@($after | Where-Object { $_ -contains 'delta.txt' })) -and
                        -not (@($after | Where-Object { $_ -contains 'alpha.txt' }))
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
}

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
if ($script:failed -gt 0) { throw "$($script:failed) shelf check(s) FAILED." }
'Done.'
