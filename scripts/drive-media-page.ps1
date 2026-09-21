#requires -Version 7
<#
.SYNOPSIS
  Drives the real notch and answers one question: does it open on the media page while something
  is playing?

.DESCRIPTION
  Every gate this feature has is static. The build is green, 526 tests are green, both lints are
  green, and none of them presses anything: the suite is not STA and cannot construct a
  UserControl, and the renders are still frames of a page nobody clicked. This is the instrument
  that clicks it.

  It can, and the reason is recorded in CLAUDE.md: app.manifest sets uiAccess="false" and only
  Release swaps in the signed one, so a DEBUG Plith runs at MEDIUM integrity. UIPI blocks
  nothing, UI Automation reads the whole tree and SendInput reaches the window. Over Remote
  Desktop as well, provided the session is Active.

  It refuses to run rather than guessing when a precondition is not met. Four of them:

  1. The build exists.

  2. The session is Active. A locked or disconnected session has no desktop, so nothing can be
     pressed and nothing can be captured.

  3. The notch is the live presentation and its widgets are on. Classic OSD has no widget frame
     to open, so every verdict below would be about a surface that is not running.

  4. SOMETHING IS ACTUALLY PLAYING, asked of SMTC through Plith's own MediaSessionClient rather
     than assumed. A run against a silent machine would pass the "not playing opens on the clock"
     half and silently skip the half worth measuring, which is how a green verdict gets produced
     for a feature nobody exercised.

     Precondition 4 distinguishes three answers, not two: playing, nothing playing, and SMTC
     unreadable from this process. The third must not be reported as the second - an instrument
     that blames the user for its own failure is worse than no instrument.

  Run with: pwsh -File scripts/drive-media-page.ps1
#>

[CmdletBinding()]
param(
    [string]$OutDir = "$env:TEMP\plith-drive-media",
    [string]$Configuration = 'Debug'
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
$plithExe = Join-Path $bin 'Plith.exe'
$plithDll = Join-Path $bin 'Plith.dll'
$configPath = Join-Path $env:LOCALAPPDATA 'Plith\config.ini'

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$script:verdicts = @()
$script:failed = 0

function Add-Verdict {
    param([string]$Name, [bool]$Ok, [string]$Detail)
    $script:verdicts += "  [$(if ($Ok) { 'PASS' } else { 'FAIL' })] $Name - $Detail"
    if (-not $Ok) { $script:failed++ }
}

# --- input, copied from drive-shelf-pair.ps1 ---------------------------------------------------
#
# Copied rather than imported, which is what drive-shelf.ps1 and drive-shelf-pair.ps1 already do
# with each other. Movement is SetCursorPos and NOT SendInput, and a partially accepted batch
# throws: both are that script's defect 4, measured with an anti-cheat driver loaded, where
# LEFTDOWN was refused intermittently while LEFTUP and every keyboard event were accepted in the
# same second. Half a click then measures a meaningless result with the product blameless.

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MediaInput {
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

    const uint LEFTDOWN=0x0002, LEFTUP=0x0004, WHEEL=0x0800;

    static void Send(INPUT[] a, string what) {
        uint n = SendInput((uint)a.Length, a, Marshal.SizeOf(typeof(INPUT)));
        if (n != a.Length) throw new Exception("SendInput refused " + what + ": accepted " + n +
            " of " + a.Length + ", Win32 error " + Marshal.GetLastWin32Error() +
            " (87 with an anti-cheat driver loaded means injected input is being filtered)");
    }
    static INPUT Mouse(uint flags) { return Mouse(flags, 0); }
    static INPUT Mouse(uint flags, uint data) {
        INPUT i = new INPUT();
        i.type = 0; i.u.mi.dwFlags = flags; i.u.mi.mouseData = data;
        return i;
    }
    public static void Wheel(int notches) {
        Send(new INPUT[] { Mouse(WHEEL, unchecked((uint)(notches * 120))) }, "WHEEL");
    }
    public static void Move(int x, int y) {
        if (!SetCursorPos(x, y))
            throw new Exception("SetCursorPos(" + x + "," + y + ") failed, Win32 error " +
                                Marshal.GetLastWin32Error());
    }
    public static void LeftClick() {
        Send(new INPUT[] { Mouse(LEFTDOWN) }, "LEFTDOWN");
        System.Threading.Thread.Sleep(70);
        Send(new INPUT[] { Mouse(LEFTUP) }, "LEFTUP");
    }
    public static void Glide(int x0, int y0, int x1, int y1, int steps) {
        for (int i = 1; i <= steps; i++) {
            Move(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
            System.Threading.Thread.Sleep(16);
        }
    }
    public static void Drag(int x0, int y0, int x1, int y1) {
        Move(x0, y0);
        System.Threading.Thread.Sleep(120);
        Send(new INPUT[] { Mouse(LEFTDOWN) }, "LEFTDOWN");
        System.Threading.Thread.Sleep(140);
        Glide(x0, y0, x1, y1, 24);
        System.Threading.Thread.Sleep(200);
        Send(new INPUT[] { Mouse(LEFTUP) }, "LEFTUP");
    }
    public static string Where() {
        POINT p; GetCursorPos(out p);
        IntPtr h = WindowFromPoint(p);
        uint owner; GetWindowThreadProcessId(h, out owner);
        return "cursor " + p.X + "," + p.Y + " over hwnd=" + h + " pid=" + owner;
    }
}
public static class MediaWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out MediaInput.RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
}
'@

function Find-LayeredWindows {
    param([string]$ProcessLike)
    $script:found = @()
    $cb = [MediaWin+EnumProc]{
        param($h, $l)
        $o = [uint32]0; [void][MediaWin]::GetWindowThreadProcessId($h, [ref]$o)
        $pn = try { (Get-Process -Id $o -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -and $pn -like $ProcessLike -and [MediaWin]::IsWindowVisible($h) -and
            ([MediaWin]::GetWindowLongW($h, -20) -band 0x80000)) {
            $r = New-Object 'MediaInput+RECT'; [void][MediaWin]::GetWindowRect($h, [ref]$r)
            if (($r.Right - $r.Left) -gt 0) {
                $script:found += [pscustomobject]@{ Hwnd=$h; Pid=[int]$o; Proc=$pn
                    X=$r.Left; Y=$r.Top; W=($r.Right-$r.Left); H=($r.Bottom-$r.Top) }
            }
        }
        $true
    }
    [void][MediaWin]::EnumWindows($cb, [IntPtr]::Zero)
    $script:found
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

function Get-Element {
    # BOTH name and control type. A label, its tooltip and the control itself can all carry the
    # same name, and a name-only search returned a text run in place of a control once already on
    # this branch.
    param($Hwnd, [string]$Name, [string]$Type = 'Custom')
    if ($null -eq $Hwnd) {
        throw "Get-Element was asked for '$Name' with a NULL window handle."
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

function Move-Pointer {
    param([int]$X, [int]$Y, [int]$Settle = 300)
    [MediaInput]::Move($X, $Y)
    Start-Sleep -Milliseconds $Settle
}

function Assert-InputWorks {
    # Read back IMMEDIATELY rather than after a sleep. drive-shelf-pair.ps1's defect 5: a sleep
    # here folds two unrelated failures into one message, and the old wording blamed "the pointer
    # did not move" when the call had worked perfectly and a hand had moved it afterwards.
    [MediaInput]::Move(400, 400)
    $landed = New-Object 'MediaInput+POINT'; [void][MediaInput]::GetCursorPos([ref]$landed)
    if ([Math]::Abs($landed.X - 400) -gt 2 -or [Math]::Abs($landed.Y - 400) -gt 2) {
        throw ("SetCursorPos reported success but the pointer read back at " +
               "$($landed.X),$($landed.Y) instead of 400,400. The call is not taking effect: " +
               "suspect DPI virtualization of a non-aware host, or a driver rejecting it.")
    }

    # Is anything ELSE driving the pointer? A hand on the mouse breaks no call here, which is why
    # it is measured separately: the press below aims by coordinate, so a pointer that will not
    # stay put produces a verdict about whatever happened to be under it. An idle machine drifts
    # 0 px across these samples; a fullscreen game holding the pointer was the measured cause
    # twice on this machine, and it re-centres the cursor rather than merely moving it.
    $drift = 0
    $prev = New-Object 'MediaInput+POINT'; [void][MediaInput]::GetCursorPos([ref]$prev)
    for ($s = 0; $s -lt 10; $s++) {
        Start-Sleep -Milliseconds 100
        $now = New-Object 'MediaInput+POINT'; [void][MediaInput]::GetCursorPos([ref]$now)
        $drift += [Math]::Abs($now.X - $prev.X) + [Math]::Abs($now.Y - $prev.Y)
        $prev = $now
    }
    if ($drift -gt 4) {
        $h = [MediaInput]::GetForegroundWindow()
        $who = [uint32]0; [void][MediaWin]::GetWindowThreadProcessId($h, [ref]$who)
        $name = (Get-Process -Id $who -ErrorAction SilentlyContinue).ProcessName
        throw ("Something else is driving the pointer: $drift px of drift across one second " +
               "while nothing here touched it. Foreground window belongs to '$name'. A game " +
               "holding the pointer and a hand on the mouse both land here; either way every " +
               "press below would aim at whatever happens to be under the cursor.")
    }

    # A full button round trip, over an empty part of the desktop, before anything is measured.
    # LEFTUP can be accepted while LEFTDOWN is refused, so checking one says nothing about the
    # other and half a click is worse than no click.
    [MediaInput]::LeftClick()
    "  input works ($([MediaInput]::Where()))"
}

# --- preconditions ----------------------------------------------------------------------------

if (-not (Test-Path $plithExe)) { throw "Build $Configuration first: $plithExe not found." }

$session = qwinsta 2>$null | Where-Object { $_ -match '^\s*>' }
if ($session -notmatch 'Active') {
    throw ("This session is not Active (`qwinsta` says: $($session -replace '\s+', ' ')). A " +
           "locked or disconnected session has no desktop: nothing can be pressed. Reconnect " +
           "and re-run.")
}
"session: $(($session -replace '\s+', ' ').Trim())"

if (-not (Test-Path $configPath)) {
    throw ("No settings at $configPath, so the presentation cannot be confirmed. Run Plith once " +
           "and set the presentation to Ambient Notch.")
}
$config = Get-Content -LiteralPath $configPath -Raw
if ($config -notmatch '(?m)^\s*Presentation\s*=\s*AmbientNotch\s*$') {
    throw ("Presentation is not AmbientNotch in $configPath. Classic OSD has no widget frame to " +
           "open, so every verdict here would be about a surface that is not running.")
}
if ($config -match '(?m)^\s*ShowNotchWidgets\s*=\s*False\s*$') {
    throw "ShowNotchWidgets is False in ${configPath}: a click on the notch opens nothing."
}
"presentation: AmbientNotch, widgets on"

# --- precondition 4: is anything playing? -----------------------------------------------------
#
# Asked of SMTC through Plith's own client, so the answer comes from the same code the product
# uses. Loaded out of Plith's own output folder, because its WinRT projection assemblies live
# beside it and not beside this script.

$null = [Reflection.Assembly]::LoadFrom($plithDll)
[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $name = ($e.Name -split ',')[0]
    $candidate = Join-Path $bin "$name.dll"
    if (Test-Path $candidate) { return [Reflection.Assembly]::LoadFrom($candidate) }
    $null
})

# The handler is COMPILED, not a scriptblock, and that is this script's own first defect rather
# than a style choice. MediaSessionClient raises Changed on a threadpool thread, and a PowerShell
# scriptblock converted to an Action<T> has no runspace on such a thread: it never runs, silently.
# Measured on 2026-09-20: the client read the session perfectly (it reported Spotify's AUMID) and
# the scriptblock handler fired zero times, which this script then reported as "SMTC never
# delivered a snapshot". It had; the collector had not.
Add-Type -ReferencedAssemblies $plithDll -TypeDefinition @'
using System;
using Plith.Services;
public static class SnapshotCollector {
    public static MediaSnapshot Latest;
    public static MediaTimeline LatestTimeline;
    public static int Count;
    public static int TimelineCount;
    public static void Attach(MediaSessionClient client) {
        client.Changed += s => { Latest = s; Count++; if (s.Timeline != null) LatestTimeline = s.Timeline; };
        client.TimelineChanged += t => { if (t != null) { LatestTimeline = t; TimelineCount++; } };
    }
}
'@

$probe = [Plith.Services.MediaSessionClient]::new()
[SnapshotCollector]::Attach($probe)
try {
    $null = $probe.StartAsync().GetAwaiter().GetResult()
} catch {
    throw ("SMTC could not be started from this process: $($_.Exception.Message). That is this " +
           "script's own failure, not an answer about what is playing.")
}

# The first snapshot is read asynchronously, after StartAsync returns.
$deadline = [Diagnostics.Stopwatch]::StartNew()
while ($null -eq [SnapshotCollector]::Latest -and $deadline.Elapsed.TotalSeconds -lt 4) {
    Start-Sleep -Milliseconds 150
}

$snapshot = [SnapshotCollector]::Latest
$aumid = $probe.CurrentSourceAppUserModelId
# NOT disposed here: the seek check below reads the position back through this same probe, which
# is the only way to see whether the source actually moved. Disposed at the end of the run.

if ($null -eq $snapshot) {
    throw ("SMTC never delivered a snapshot to this process within four seconds. This is the " +
           "third answer, not the second: it means the probe could not read the session " +
           "manager, NOT that nothing is playing. Do not record a verdict from this run.")
}

# The rule has two directions and this run measures whichever one the machine is in, naming it
# rather than refusing. Refusing while paused was this script's own second defect: the paused
# case is not a vacuous run, it is the OTHER HALF of the rule (not playing opens the clock), and
# a script that throws there leaves half the rule permanently unmeasured.
$expectMedia = [bool]$snapshot.IsPlaying

if ($snapshot.HasSession) {
    "session: '$($snapshot.Title)' by '$($snapshot.Artist)' from '$aumid'"
    "state:   $(if ($snapshot.IsPlaying) { 'PLAYING' } else { 'PAUSED' })"
} else {
    "session: none"
}
"timeline: $(if ($snapshot.Timeline) { "$($snapshot.Timeline.Position) of $($snapshot.Timeline.Duration), stamped $($snapshot.Timeline.LastUpdated)" } else { 'NONE - this source reports no usable duration, so the progress row is absent by design' })"
"measuring: the $(if ($expectMedia) { 'PLAYING' } else { 'NOT PLAYING' }) direction. The other direction needs its own run."

Assert-InputWorks

# --- the run ------------------------------------------------------------------------------------

# A stale instance holds the single-instance mutex and a new one exits silently. That made the
# drop catcher look guilty for a failure that was not its, recorded as instrument defect 5 in
# docs/SHELF-VERIFICATION.md.
$already = Get-Process Plith -ErrorAction SilentlyContinue
$script:startedPlith = $false
if ($already) {
    "plith: already running (pid $($already.Id -join ', ')), using it and leaving it running"
} else {
    Start-Process $plithExe
    Start-Sleep -Seconds 5
    $script:startedPlith = $true
    "plith: started by this run, and it will be stopped at the end"
}

$windows = Find-LayeredWindows -ProcessLike 'Plith'
if (-not $windows) {
    throw ("Plith has no visible layered window. The notch is layered with per-pixel " +
           "transparency, so its absence means the presentation never came up.")
}
foreach ($w in $windows) { "  layered window: hwnd=$($w.Hwnd) at $($w.X),$($w.Y) $($w.W)x$($w.H)" }

# The notch rests at top-centre. Of Plith's layered windows, take the one nearest the top edge.
$notch = $windows | Sort-Object Y | Select-Object -First 1

# Its own window's top-centre, a few pixels down: the resting shape is a thin strip at the top of
# that window, and the window is wider than the strip.
$x = [int](($notch.X + $notch.X + $notch.W) / 2)
$y = [int]($notch.Y + 3)

# Clicked until a widget PAGE is up, not once.
#
# INSTRUMENT DEFECT 6, and the third of this family on this script: something happening between
# the press and the read. Plith applies its first media snapshot a moment after it starts, and
# with AutoShowOnMedia on that shows the media HUD; any PlaybackInfoChanged from the source does
# the same. A click while a HUD is up opens the frame, which is deliberate, but another event
# arriving in the next second replaces it again. The run at 03:05 on 2026-09-21 read a tree
# holding exactly two names, the HUD's title and artist, and reported that the notch had not
# opened at all.
#
# A page is recognised by what only a page has: the media page's own name, or the clock page's
# "HH:mm, <date>" announcement. A HUD carries neither.
$pageUp = $false
for ($attempt = 1; $attempt -le 5; $attempt++) {
    Move-Pointer -X $x -Y $y -Settle 400
    [MediaInput]::LeftClick()
    Start-Sleep -Milliseconds 900      # the expansion animates

    $seen = Get-Names -Hwnd $notch.Hwnd
    if ($seen -contains 'Now playing' -or ($seen -match '^\d{1,2}:\d{2}, ')) {
        $pageUp = $true
        if ($attempt -gt 1) { "  (the frame took $attempt clicks: an event had replaced it)" }
        break
    }
    Start-Sleep -Milliseconds 600
}
if (-not $pageUp) {
    throw ("Five clicks and no widget page ever appeared. The tree holds: " +
           "$((Get-Names -Hwnd $notch.Hwnd) -join ' | '). If those are a title and an artist, a " +
           "HUD is being shown faster than it can be clicked through, which is a product " +
           "question rather than a missed click.")
}

# The playing state is re-read HERE, immediately after the click, not taken from the snapshot
# that was read before Plith was even started.
#
# INSTRUMENT DEFECT 3. The first version decided the expectation from that earlier snapshot, and
# on the run at 02:53 on 2026-09-21 the track was paused when it was read and playing by the time
# the notch was clicked, because a person pressed play in between. The product opened the media
# page, which is correct, and the script called it a failure. It is the same shape as defect 6 in
# drive-shelf-pair.ps1: state read long before the press it is used to judge.
$playingNow = [SnapshotCollector]::Latest.IsPlaying
if ($playingNow -ne $expectMedia) {
    "  NOTE: playback changed during the run (was $(if ($expectMedia) { 'playing' } else { 'not playing' }), " +
    "now $(if ($playingNow) { 'playing' } else { 'not playing' })). Judging against the state at the click."
    $expectMedia = $playingNow
}

$names = Get-Names -Hwnd $notch.Hwnd
$onMedia = $names -contains 'Now playing'

# The clock page announces itself as "HH:mm, <date>[, battery ...]" on its Time element, because
# a screen reader landing on "21:04" alone has no way to know what it is. That pattern is how this
# script recognises the page without looking at pixels.
$onClock = [bool]($names -match '^\d{1,2}:\d{2}, ')

if ($expectMedia) {
    Add-Verdict 'a click while playing opens the media page' $onMedia `
        "names in the tree: $($names -join ' | ')"
} else {
    Add-Verdict 'a click while NOT playing opens the clock page, not the media page' `
        ($onClock -and -not $onMedia) "names in the tree: $($names -join ' | ')"
}

# The other half of the accessibility claim: the bar is a real ProgressBar, so where the track has
# got to reaches a screen reader as a VALUE rather than as a length of pixels. Only meaningful on
# the media page, so it is skipped rather than failed when the run measured the other direction.
if ($onMedia) {
    $el = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$notch.Hwnd)
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, 'Playback position')
    $bar = $el.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)

    $value = 'bar not in the tree'
    $hasValue = $false
    if ($bar) {
        try {
            $pattern = $bar.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern)
            $value = $pattern.Current.Value
            $hasValue = $value -gt 0
        } catch {
            $value = "bar is in the tree but exposes no RangeValue pattern: $($_.Exception.Message)"
        }
    }
    Add-Verdict 'the progress bar reports a value to UI Automation' $hasValue "value: $value"
}

# --- the progress row on the real window ------------------------------------------------------
#
# Reached by paging rather than by playing, so this half is measurable whatever the machine is
# doing. One wheel notch is exactly NotchPager.CommitThreshold (120) and IdleRearmMs is 150, so a
# notch every 700 ms is one page with the accumulator rested in between. A plain vertical wheel
# pages, and the media page is installed third at most, so one lap is enough.
if (-not $onMedia) {
    # The pointer is put back on the frame before each notch, and where it is is REPORTED.
    #
    # A wheel event goes to the window under the cursor, so a pointer that has drifted off the
    # frame pages nothing and looks exactly like a pager that is broken. The run at 03:01 on
    # 2026-09-21 paged zero times while two earlier runs paged fine, and the failure said only
    # "names after paging" with the clock page still in them, which names neither cause.
    for ($i = 1; $i -le 6; $i++) {
        Move-Pointer -X $x -Y $y -Settle 150
        $where = [MediaInput]::Where()
        [MediaInput]::Wheel(-1)
        Start-Sleep -Milliseconds 700
        $names = Get-Names -Hwnd $notch.Hwnd
        "  wheel $i -> $where; tree: $(($names | Select-Object -First 2) -join ' | ')"
        if ($names -contains 'Now playing') { $onMedia = $true; break }
    }
    Add-Verdict 'the notch pages to the media widget' $onMedia `
        "names after paging: $((Get-Names -Hwnd $notch.Hwnd) -join ' | ')"
}

if ($onMedia) {
    $el = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$notch.Hwnd)
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, 'Playback position')
    $bar = $el.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)

    # The claim being measured is "the position reaches a screen reader as a VALUE", so what is
    # checked is the pattern and its range, not that the value is above zero. A track at 0:00 is
    # a legitimate state and an assertion that failed on it would be measuring playback rather
    # than accessibility.
    $detail = 'bar not in the tree'
    $ok = $false
    if ($bar) {
        try {
            $pattern = $bar.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern)
            $v = $pattern.Current.Value
            $max = $pattern.Current.Maximum
            $ok = ($max -eq 100) -and ($v -ge 0) -and ($v -le 100)
            $detail = "value=$v of $max"
        } catch {
            $detail = "in the tree but exposes no RangeValue pattern: $($_.Exception.Message)"
        }
    }
    Add-Verdict 'the progress bar reports a value to UI Automation' $ok $detail

    # And the two clocks, which are the part a sighted person reads. Their presence in the tree
    # is the only evidence available without capturing the window.
    $names = Get-Names -Hwnd $notch.Hwnd
    $clocks = @($names | Where-Object { $_ -match '^-?\d{1,2}:\d{2}$' })
    Add-Verdict 'the elapsed and remaining clocks are drawn' ($clocks.Count -ge 2) `
        "clock-shaped names: $($clocks -join ', ')"

    # --- and does dragging it actually move the source? ---------------------------------------
    #
    # The only honest evidence is the position coming back from SMTC, so this reads the timeline
    # before and after through the same probe. Skipped rather than failed when the source refuses
    # position writes: then there is nothing to measure and the bar is disabled by design.
    if (-not $snapshot.CanSeek) {
        "  (skipped: this source reports IsPlaybackPositionEnabled false, so the track is read-only)"
    } elseif (-not $bar) {
        "  (skipped: the bar is not in the tree, so there is nothing to drag)"
    } else {
        $before = [SnapshotCollector]::LatestTimeline

        # The target is chosen AWAY from where the track already is.
        #
        # INSTRUMENT DEFECT 7. A fixed 75 per cent target passes vacuously when the position is
        # already there, which is exactly what a previous run of this same script leaves behind:
        # at 03:02 on 2026-09-21 it reported "before 139s, after 139s, wanted about 137s" and
        # called that a pass. An assertion that cannot fail is not a measurement. Whichever end
        # the track is not at is the end this drags to, so before and after always differ by more
        # than the tolerance.
        $atFraction = if ($before.Duration.TotalSeconds -gt 0) {
            $before.Position.TotalSeconds / $before.Duration.TotalSeconds
        } else { 0 }
        $targetFraction = if ($atFraction -gt 0.5) { 0.20 } else { 0.75 }
        "  seeking from $([int]$before.Position.TotalSeconds)s (fraction $([Math]::Round($atFraction,2))) to fraction $targetFraction"

        $r = $bar.Current.BoundingRectangle
        $y = [int]($r.Y + $r.Height / 2)
        $from = [int]($r.X + $r.Width * $atFraction)
        $to = [int]($r.X + $r.Width * $targetFraction)

        [MediaInput]::Drag($from, $y, $to, $y)

        # Wait for the position to SETTLE near the target, not for it to merely change.
        #
        # INSTRUMENT DEFECT 4. Waiting for "any change" read the intermediate value: the press
        # seeked to where the drag started and the release seeked to where it ended, so the first
        # change to arrive was the wrong one. The run at 02:53 on 2026-09-21 reported 15s against
        # a target of 137s for that reason, next to a real product defect with the same symptom.
        # Fixed on both sides: the product now writes once per gesture, and this waits for the
        # value it is actually asserting on.
        $want = $before.Duration.TotalSeconds * $targetFraction
        $deadline = [Diagnostics.Stopwatch]::StartNew()
        while ($deadline.Elapsed.TotalSeconds -lt 5) {
            $seen = [SnapshotCollector]::LatestTimeline
            if ([Math]::Abs($seen.Position.TotalSeconds - $want) -le 6) { break }
            Start-Sleep -Milliseconds 200
        }
        $after = [SnapshotCollector]::LatestTimeline

        # Within a tolerance the gesture itself cannot beat: the bar is about 232 DIP wide, so one
        # DIP is nearly a second on a three minute track and the pointer lands on a whole pixel.
        #
        # The verdict also requires the position to have MOVED, which is what makes it a
        # measurement rather than a coincidence. See instrument defect 7 above.
        $got = $after.Position.TotalSeconds
        $moved = [Math]::Abs($got - $before.Position.TotalSeconds) -gt 6
        $landed = [Math]::Abs($got - $want) -le 6
        Add-Verdict 'dragging the track seeks the source' ($moved -and $landed) `
            ("before $([int]$before.Position.TotalSeconds)s, after $([int]$got)s, " +
             "wanted about $([int]$want)s of $([int]$after.Duration.TotalSeconds)s" +
             "$(if (-not $moved) { ' - IT DID NOT MOVE' })")
    }
}

# What this run started, it stops. A running Plith.exe holds its own binary open, so the next
# `dotnet build` fails with MSB3021/MSB3027 and the message names a file copy rather than a
# leftover process: measured on 2026-09-20, where two build errors read as a code defect for a
# minute. An instance that was ALREADY up belongs to the person at the machine, so it is left
# exactly as it was found.
# --- the output picker --------------------------------------------------------------------------
#
# This stage CHANGES A SYSTEM SETTING, so it restores it in a finally. A check that leaves
# someone's audio coming out of a different device is a rude check, and one that only leaks when
# it fails is worse: the failure is exactly when nobody is watching the cleanup.
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'NAudio.Wasapi.dll'))
$audio = New-Object NAudio.CoreAudioApi.MMDeviceEnumerator
$originalId = $audio.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
$originalName = $audio.GetDefaultAudioEndpoint('Render', 'Multimedia').FriendlyName
"output before : $originalName"

# How many outputs there are to choose between, which decides what this stage can measure at all.
#
# MEASURED 2026-09-21: over Remote Desktop there is exactly ONE active render endpoint, "Remote
# Audio", and IPolicyConfig refuses it with 0x80004002 (E_NOINTERFACE). So in an RDP session the
# switch cannot be measured and the failure is the environment's, not the product's. Saying so is
# the whole job here: a verdict that failed for this reason would send someone looking for a bug
# that is not there.
$endpoints = [Plith.Services.WindowsAudioClient]::EnumerateRenderEndpoints()
"outputs found : $($endpoints.Count) ($(($endpoints | ForEach-Object { $_.FriendlyName }) -join ', '))"
$canSwitch = $endpoints.Count -ge 2
if (-not $canSwitch) {
    "  NOTE: fewer than two outputs, so the switch itself cannot be measured in this session."
    if ($session -match 'rdp-tcp') {
        "        This is a Remote Desktop session: the local devices are not active in it. Run"
        "        this from the physical console to measure the switch."
    }
}

try {
    if (-not $onMedia) {
        "  (skipped: the media page was never reached, so its output control cannot be pressed)"
    } else {
        $outputBtn = Get-Element -Hwnd $notch.Hwnd -Name 'Change output device' -Type 'Button'
        if (-not $outputBtn) {
            Add-Verdict 'the output control is in the tree' $false 'not found by name'
        } else {
            Move-Pointer -X $outputBtn.CX -Y $outputBtn.CY -Settle 250
            [MediaInput]::LeftClick()
            Start-Sleep -Milliseconds 700

            $names = Get-Names -Hwnd $notch.Hwnd
            $onPicker = $names -contains 'Back to now playing'
            Add-Verdict 'the output control opens the picker' $onPicker `
                "names in the tree: $($names -join ' | ')"

            # The way back, which is measurable however many devices there are.
            if ($onPicker) {
                $backBtn = Get-Element -Hwnd $notch.Hwnd -Name 'Back to now playing' -Type 'Button'
                if ($backBtn) {
                    Move-Pointer -X $backBtn.CX -Y $backBtn.CY -Settle 200
                    [MediaInput]::LeftClick()
                    Start-Sleep -Milliseconds 500
                    $afterBack = Get-Names -Hwnd $notch.Hwnd
                    Add-Verdict 'the back control returns to now playing' `
                        ($afterBack -contains 'Now playing') "names: $($afterBack -join ' | ')"

                    # Open it again for whatever follows.
                    Move-Pointer -X $outputBtn.CX -Y $outputBtn.CY -Settle 200
                    [MediaInput]::LeftClick()
                    Start-Sleep -Milliseconds 600
                    $names = Get-Names -Hwnd $notch.Hwnd
                } else {
                    Add-Verdict 'the back control is pressable' $false `
                        "'Back to now playing' is in the tree but not as a Button"
                }
            }

            # A device that is NOT the current one, found by its own accessible name: the current
            # one carries ", current output" and pressing it would prove nothing at all.
            $target = $names | Where-Object {
                $_ -ne 'Back to now playing' -and $_ -ne 'Output' -and
                $_ -notmatch 'current output' -and $_ -ne 'More in Windows settings' -and
                $_ -ne 'Now playing'
            } | Select-Object -First 1

            if (-not $canSwitch) {
                "  (skipped: one output in this session, so there is nothing to switch TO)"
            } elseif (-not $target) {
                Add-Verdict 'a second output is offered' $false "names: $($names -join ' | ')"
            } else {
                $cell = Get-Element -Hwnd $notch.Hwnd -Name $target -Type 'Button'
                if (-not $cell) {
                    Add-Verdict 'the offered output is pressable' $false `
                        "'$target' is in the tree but not as a Button"
                } else {
                    Move-Pointer -X $cell.CX -Y $cell.CY -Settle 250
                    [MediaInput]::LeftClick()
                    Start-Sleep -Milliseconds 1200

                    $nowId = $audio.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
                    Add-Verdict 'pressing a cell changes the system default output' `
                        ($nowId -ne $originalId) `
                        ("pressed '$target'; default was $originalId, now $nowId")

                    $back = Get-Names -Hwnd $notch.Hwnd
                    Add-Verdict 'the page returns to now playing after a switch' `
                        ($back -contains 'Now playing') "names after: $($back -join ' | ')"
                }
            }
        }
    }
}
finally {
    # Through the product's own switcher, so the restore exercises the same path it is undoing.
    # Only when something actually moved. Calling the switcher unconditionally made the closing
    # line read "restore returned False" on a run where nothing had been switched at all, which
    # is an alarm about the wrong thing: over RDP the one endpoint there is cannot be set as
    # default, and it was already default anyway.
    $endId = $audio.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
    if ($endId -ne $originalId) {
        $restored = [Plith.Services.OutputDeviceSwitcher]::TrySetDefault($originalId, $null)
        $endId = $audio.GetDefaultAudioEndpoint('Render', 'Multimedia').ID
        $endName = $audio.GetDefaultAudioEndpoint('Render', 'Multimedia').FriendlyName
        "output after  : $endName (restore returned $restored)"
        if ($endId -ne $originalId) {
            Write-Host "WARNING: the default output was NOT restored. Set it back by hand." -ForegroundColor Red
        }
    } else {
        "output after  : $originalName (unchanged, so nothing to restore)"
    }
}

$probe.Dispose()

if ($script:startedPlith) {
    # The catcher too: Plith launches it, and stopping only Plith leaves it orphaned holding its
    # OWN binary open, which fails the next build with the same two errors one process further
    # along. Measured on 2026-09-20, after killing Plith by hand and building again.
    Get-Process Plith, Plith.DropCatcher -ErrorAction SilentlyContinue | Stop-Process
    "plith: stopped, with the catcher it launched (this run started them)"
} else {
    "plith: left running (it was up before this run; `dotnet build` will fail while it is)"
}

"`nverdicts:"
$script:verdicts
if ($script:failed -gt 0) {
    Write-Host "`n$($script:failed) verdict(s) FAILED" -ForegroundColor Red
    exit 1
}
Write-Host "`nall verdicts passed" -ForegroundColor Green
