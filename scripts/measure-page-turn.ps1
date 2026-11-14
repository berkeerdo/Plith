#requires -Version 7
<#
.SYNOPSIS
  Measures what one page turn of the open notch costs FRAME BY FRAME, and sets the turn onto the
  shelf beside the ordinary ones.

.DESCRIPTION
  docs/wpf/ROADMAP.md section 9 carried this as the open question with no instrument behind it: the
  turn onto the shelf page comes in choppy while the other four do not. Nothing in the repo could
  see it.

  What had already been measured was DURATION. docs/PERF-VERIFICATION.md section 4 says the turn
  onto the shelf takes about 340 ms end to end, split into a 260 ms slide, the handover and a 70 ms
  fade, and docs/SHELF-VERIFICATION.md section 10.28 timed the fade against Plith's own hide on
  hardware. Neither can answer this, and the reason is worth stating once: a 260 ms slide that
  delivers every frame and a 260 ms slide that delivers half of them take exactly as long.

  It measures nothing itself. Plith's own PageTurnTrace writes one line per animated page turn
  into %LOCALAPPDATA%\Plith\plith.log, and this script drives the gestures and tabulates the
  lines. Timestamps and samples taken from inside the process are the only thing that reports the
  same way whatever integrity level Plith is running at, and OsdHost's own note on the page-commit
  line says why it has to be that way: "Instrument from inside; three external sampling harnesses
  during slice 2 all gave misleading answers."

  WHAT THE COLUMNS MEAN is documented on PageTurnTrace itself, not repeated here, so the two
  cannot drift apart. Two things about reading the table:

    THE CONTROL GROUP IS THE POINT. A number for the shelf turn alone says nothing. The shelf
    page is always installed last (OsdHost.BuildWidgetPages), so the turn whose target is the
    last index is the one under suspicion and every other row in the same run is its control,
    measured by the same instrument on the same machine in the same minute. The table marks it.

    A CLEAN LINE IS NOT A VERDICT OF "NO DEFECT". Every sample is taken on the UI thread, so none
    of it sees the frame the monitor scanned out, and none of it knows whether the page had
    anything drawn on it. If the shelf row matches its controls on both axes, the remaining
    hypothesis is the one docs/SHELF-VERIFICATION.md section 10.29 named: Plith's own shelf page
    draws nothing, so the slide carries an empty frame in. That one needs a capture, not a timer.

  Two modes:

    (default)  Stops any running Plith, starts the one at -ExePath, opens the notch, and pages
               -Laps times round the widget frame. Stopping the running instance is deliberate:
               the first turn onto a page includes that page's first layout, which is a suspect,
               and an instance that has already been paged has spent it.

    -NoDrive   Starts nothing and pages nothing. It watches the log and tabulates what appears
               while YOU page the notch by hand. For a Plith this shell did not start and must
               not restart, or when an anti-cheat driver is filtering injected input.

               NOT AGAINST THE INSTALLED PLITH. PageTurnTrace landed on 2026-10-13 and the
               installed build predates it, so an installed instance writes no turn line
               however many times you page it. That silence is indistinguishable from the
               animation gate being off, which is the one thing this instrument exists to
               tell apart. -NoDrive is for a Debug build you started yourself.

  Run with: pwsh -File scripts/measure-page-turn.ps1
            pwsh -File scripts/measure-page-turn.ps1 -ExePath 'C:\Program Files\Plith\Plith.exe'
            pwsh -File scripts/measure-page-turn.ps1 -NoDrive -Laps 3
#>

[CmdletBinding()]
param(
    [int]$Laps = 3,
    [string]$Configuration = 'Debug',
    [string]$ExePath,
    [switch]$NoDrive,
    [int]$TimeoutSeconds = 240
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
$plithExe = if ($ExePath) { $ExePath } else { Join-Path $bin 'Plith.exe' }
$logPath = Join-Path $env:LOCALAPPDATA 'Plith\plith.log'

# --- input, copied from measure-notch-open.ps1 --------------------------------------------------
#
# Copied rather than imported, which is what every driver in this directory already does with
# every other one. Movement is SetCursorPos and NOT SendInput, and a partially accepted batch
# throws: both are drive-shelf-pair's instrument defect 4, measured with an anti-cheat driver
# loaded, where LEFTDOWN was refused intermittently while LEFTUP was accepted in the same second.

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class TurnInput {
    // The union is not decoration. INPUT must marshal to exactly 40 bytes on x64, and a struct
    // holding only MOUSEINPUT is 32: SendInput then rejects every item with error 87.
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {
        public int dx; public int dy; public uint mouseData; public uint dwFlags;
        public uint time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT {
        public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Explicit)] public struct INPUTUNION {
        [FieldOffset(0)] public MOUSEINPUT mi;
        [FieldOffset(0)] public KEYBDINPUT ki;
    }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public INPUTUNION u; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

    const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004, WHEEL = 0x0800, HWHEEL = 0x01000;

    [DllImport("user32.dll", SetLastError = true)]
    static extern uint SendInput(uint n, INPUT[] a, int size);
    [DllImport("user32.dll", SetLastError = true)] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);

    static void Send(INPUT[] a, string what) {
        uint sent = SendInput((uint)a.Length, a, Marshal.SizeOf<INPUT>());
        if (sent != a.Length)
            throw new Exception("SendInput(" + what + ") accepted " + sent + " of " + a.Length +
                " , Win32 error " + Marshal.GetLastWin32Error() +
                " (87 with an anti-cheat driver loaded means injected input is being filtered)");
    }
    static INPUT Mouse(uint flags, uint data) {
        INPUT i = new INPUT();
        i.type = 0; i.u.mi.dwFlags = flags; i.u.mi.mouseData = data;
        return i;
    }
    // One notch is exactly NotchPager.CommitThreshold, so one call pages once.
    public static void Wheel(int notches) { Send(new INPUT[] { Mouse(WHEEL, unchecked((uint)(notches * 120))) }, "WHEEL"); }
    public static void HWheel(int notches) { Send(new INPUT[] { Mouse(HWHEEL, unchecked((uint)(notches * 120))) }, "HWHEEL"); }
    public static void Move(int x, int y) {
        if (!SetCursorPos(x, y))
            throw new Exception("SetCursorPos(" + x + "," + y + ") failed, Win32 error " + Marshal.GetLastWin32Error());
    }
    public static void LeftClick() {
        Send(new INPUT[] { Mouse(LEFTDOWN, 0) }, "LEFTDOWN");
        System.Threading.Thread.Sleep(70);
        Send(new INPUT[] { Mouse(LEFTUP, 0) }, "LEFTUP");
    }
    public static string Where() {
        POINT p; GetCursorPos(out p);
        IntPtr h = WindowFromPoint(p);
        uint owner; GetWindowThreadProcessId(h, out owner);
        return "cursor " + p.X + "," + p.Y + " over hwnd=" + h + " pid=" + owner;
    }
}
public static class TurnWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out TurnInput.RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
}
'@

function Find-LayeredWindows {
    param([string]$ProcessLike)
    $script:found = @()
    $cb = [TurnWin+EnumProc]{
        param($h, $l)
        $o = [uint32]0; [void][TurnWin]::GetWindowThreadProcessId($h, [ref]$o)
        $pn = try { (Get-Process -Id $o -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -and $pn -like $ProcessLike -and [TurnWin]::IsWindowVisible($h) -and
            ([TurnWin]::GetWindowLongW($h, -20) -band 0x80000)) {
            $r = New-Object 'TurnInput+RECT'; [void][TurnWin]::GetWindowRect($h, [ref]$r)
            if (($r.Right - $r.Left) -gt 0) {
                $script:found += [pscustomobject]@{ Hwnd=$h; Pid=[int]$o; X=$r.Left; Y=$r.Top
                    W=($r.Right-$r.Left); H=($r.Bottom-$r.Top) }
            }
        }
        $true
    }
    [void][TurnWin]::EnumWindows($cb, [IntPtr]::Zero)
    $script:found
}

# --- reading the log ----------------------------------------------------------------------------
#
# Filtered by timestamp rather than by byte offset, for the reason measure-notch-open.ps1 gives:
# an offset is shorter but breaks silently if DiagnosticLog rotates mid-run, and a silent break
# here would read as "the notch never paged".

function Read-TurnLines {
    param([datetime]$Since)
    if (-not (Test-Path $logPath)) { return @() }

    # Shared read: Plith holds the file open for append, and a plain Get-Content would throw.
    $text = try {
        $fs = [System.IO.File]::Open($logPath, 'Open', 'Read', 'ReadWrite')
        try { (New-Object System.IO.StreamReader($fs)).ReadToEnd() } finally { $fs.Dispose() }
    } catch { return @() }

    $rowList = [System.Collections.Generic.List[object]]::new()
    foreach ($line in $text -split "`r?`n") {
        if ($line -notmatch '^\[(?<ts>[^\]]+)\]\s+\[\w+\]\s+\[OsdHost\]\s+(?<body>Turn #\d+: .*)$') { continue }
        $ts = [datetime]::Parse($Matches.ts, [cultureinfo]::InvariantCulture,
                                ([System.Globalization.DateTimeStyles]::AdjustToUniversal -bor
                                 [System.Globalization.DateTimeStyles]::AssumeUniversal))
        if ($ts -lt $Since) { continue }
        $body = $Matches.body

        # A turn that rendered nothing is its own line shape, and it is the worst finding this
        # instrument can make. Matched first so it can never fall through to the table as zeros.
        if ($body -match '^Turn #(?<n>\d+): (?<from>-?\d+)->(?<to>\d+) of (?<count>\d+) in (?<dur>\d+)ms: no frames rendered$') {
            $rowList.Add([pscustomobject]@{
                N=[int]$Matches.n; From=[int]$Matches.from; To=[int]$Matches.to
                Count=[int]$Matches.count; Dur=[int]$Matches.dur
                Frames=0; Median=$null; Worst=$null; Late=$null
                Travel=$null; Step=$null; Still=$null; NoFrames=$true
            })
            continue
        }

        if ($body -notmatch ('^Turn #(?<n>\d+): (?<from>-?\d+)->(?<to>\d+) of (?<count>\d+) in (?<dur>\d+)ms: ' +
                             '(?<frames>\d+) frames, median (?<median>[\d.]+)ms, worst (?<worst>[\d.]+)ms, (?<late>\d+) late; ' +
                             'travel (?<travel>[\d.]+) DIP, biggest step (?<step>[\d.]+), (?<still>\d+) still$')) { continue }

        $rowList.Add([pscustomobject]@{
            N=[int]$Matches.n; From=[int]$Matches.from; To=[int]$Matches.to
            Count=[int]$Matches.count; Dur=[int]$Matches.dur
            Frames=[int]$Matches.frames
            Median=[double]$Matches.median; Worst=[double]$Matches.worst; Late=[int]$Matches.late
            Travel=[double]$Matches.travel; Step=[double]$Matches.step; Still=[int]$Matches.still
            NoFrames=$false
        })
    }

    # An ordinal is unique per process, so a duplicate means two Plith processes wrote into one
    # log. Keeping the first of each would average two machines' worth of state into one table.
    foreach ($g in ($rowList | Group-Object N)) {
        if ($g.Count -gt 1) {
            throw ("Two lines claim Turn #$($g.Name). More than one Plith wrote to this log " +
                   "during the run, so the table would mix two processes. Stop them all and rerun.")
        }
    }
    @($rowList | Sort-Object N)
}

# --- the precondition that cost this instrument its first run -----------------------------------
#
# THERE IS NO SLIDE TO MEASURE WHEN WINDOWS SAYS ANIMATIONS ARE OFF, and without this check that
# arrives as "No page turn was logged", which reads as a broken instrument or a product that does
# not page. It is neither. WidgetFrame.IsAnimationAllowed reads
# SystemParameters.ClientAreaAnimation and passes animate:false when it is off, so ShowCurrentPage
# puts the page there with no movement, nothing calls BeginSampling, and the trace correctly has
# nothing to say.
#
# MEASURED on 2026-10-13, which is why this exists rather than a comment: the first run of this
# script paged 15 times, committed every one, and logged no turn. Plith's own log shows the
# handover firing 9 ms after the page commit where ReconcileShelfFrame defers it by
# SlideRemainingMs, which is the same fact from the other side: the slide's duration was zero.
#
# Reported rather than worked around. Forcing the parameter on would measure a path the person
# never sees, and the setting is also the answer to a question about the product rather than about
# the harness: a machine with animations off never sees the page slide at all.
function Assert-AnimationsOn {
    Add-Type -AssemblyName PresentationFramework
    if ([System.Windows.SystemParameters]::ClientAreaAnimation) { return }

    $session = try { (qwinsta 2>$null | Where-Object { $_ -match '^>' }) -replace '\s+', ' ' } catch { 'unknown' }
    throw ("SystemParameters.ClientAreaAnimation is FALSE in this session, so WidgetFrame pages " +
           "WITHOUT animating and there is no slide for this instrument to measure.`n`n" +
           "Active session: $session`n`n" +
           "Windows reports it false for two different reasons, and they are not the same finding:`n" +
           "  - a REMOTE DESKTOP session, where Windows forces it off whatever the machine prefers;`n" +
           "  - the person's own 'Show animations in Windows' setting being off, which holds at`n" +
           "    the console too and means they never see the page slide at all.`n`n" +
           "Tell them apart before concluding anything: rerun this at the CONSOLE. If it is false " +
           "there too, the setting is the machine's, and the reported choppiness cannot be about " +
           "the slide, because no slide runs.")
}

function Assert-InputWorks {
    [TurnInput]::Move(400, 400)
    $landed = New-Object 'TurnInput+POINT'; [void][TurnInput]::GetCursorPos([ref]$landed)
    if ([Math]::Abs($landed.X - 400) -gt 2 -or [Math]::Abs($landed.Y - 400) -gt 2) {
        throw ("SetCursorPos reported success but the pointer read back at $($landed.X),$($landed.Y). " +
               "Injected input is not taking effect, so nothing below would be measuring the notch.")
    }
}

# --- the run ------------------------------------------------------------------------------------

$startedAtUtc = (Get-Date).ToUniversalTime().AddSeconds(-1)
"log: $logPath"
"NOTE: this drives the mouse. Do not use the machine while it runs."

# Checked in BOTH modes, and before anything is started or clicked. A -NoDrive run would
# otherwise ask a person to page the notch by hand for four minutes and then tell them nothing
# was logged.
Assert-AnimationsOn

if ($NoDrive) {
    "mode: watching only. Nothing will be started and nothing will be paged."
    ""
    "  Page the notch round the frame $Laps times by hand, pausing between gestures so the"
    "  pager rearms. Include the shelf page: it is the last one, and it is the row this run"
    "  exists to compare."
    ""
    $want = $Laps
    $rows = @()
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline -and $rows.Count -lt $want) {
        Start-Sleep -Milliseconds 500
        $next = Read-TurnLines -Since $startedAtUtc
        if ($next.Count -gt $rows.Count) { "  saw turn #$($next[-1].N) ($($next[-1].From)->$($next[-1].To))" }
        $rows = $next
    }
} else {
    if (-not (Test-Path $plithExe)) {
        throw "No Plith at $plithExe. Run: dotnet build src\Plith\Plith.csproj -c $Configuration, or pass -ExePath."
    }

    Assert-InputWorks

    # Not tidiness. The FIRST turn onto a page includes that page's first measure and arrange,
    # because a page that has never been shown has never been laid out, and that is one of the
    # suspects this run is here to see. An instance that has already been paged has spent it.
    # AN INSTALLED PLITH CANNOT BE STOPPED FROM HERE, and the failure is worth naming rather than
    # letting it arrive as "Access is denied". A Release install runs UIAccess at HIGH integrity,
    # so a medium-integrity shell cannot touch its process even though it can drive its windows
    # with SendInput (measured, see measure-notch-open.ps1's header). The same trap, from the
    # other direction, is docs/SHELF-VERIFICATION.md section 10.29.
    #
    # It also cannot simply be left running: Plith is single-instance, so a second one exits
    # silently, and the Debug build this script measures would never come up at all. The
    # instrument therefore stops here and says what to run, rather than measuring the wrong thing.
    $already = Get-Process Plith -ErrorAction SilentlyContinue
    if ($already) {
        "plith: stopping the running instance (pid $($already.Id -join ', ')). A first turn onto a page can only be measured on a process that has not had one."
        try {
            $already | Stop-Process -ErrorAction Stop
        } catch {
            throw ("Could not stop the running Plith (pid $($already.Id -join ', ')): $($_.Exception.Message)`n`n" +
                   "An INSTALLED Plith runs elevated at high integrity and a plain Stop-Process cannot reach it. " +
                   "Stop it with an elevated shell first and rerun:`n`n" +
                   "    sudo taskkill /F /IM Plith.exe /T          (PowerShell)`n" +
                   "    sudo taskkill //F //IM Plith.exe //T       (Git Bash)`n`n" +
                   "The doubled slashes are not a typo. MSYS rewrites a lone /F into the path F:\ " +
                   "and taskkill then reports `"Invalid argument/option - 'F:/'`", which is what the " +
                   "single-slash form in docs/SHELF-VERIFICATION.md section 10.29 does from that shell.`n`n" +
                   "It cannot be left running instead: Plith is single-instance, so the build under " +
                   "measurement would exit silently and this run would measure nothing.")
        }
        Start-Sleep -Seconds 2
    }

    $startedAtUtc = (Get-Date).ToUniversalTime().AddSeconds(-1)
    Start-Process $plithExe
    "plith: started from $plithExe"

    $notch = $null
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 500
        $windows = Find-LayeredWindows -ProcessLike 'Plith'
        if ($windows) { $notch = $windows | Sort-Object Y | Select-Object -First 1; break }
    }
    if (-not $notch) {
        throw ("Plith came up with no visible layered window. The notch is layered with per-pixel " +
               "transparency, so its absence means the presentation never appeared.")
    }
    "notch: hwnd=$($notch.Hwnd) at $($notch.X),$($notch.Y) $($notch.W)x$($notch.H)"

    # Settle before the first click. Not padding: AttachAudioSource and AttachShelf build the
    # widget pages, and paging before they have run would measure a frame with no pages in it.
    Start-Sleep -Seconds 4

    # Its own window's top-centre, a few pixels down: the resting shape is a thin strip at the top
    # of that window, and the window is wider than the strip.
    $x = [int](($notch.X + $notch.X + $notch.W) / 2)
    $y = [int]($notch.Y + 3)

    # Open the frame. A click while an event HUD is up opens the frame too, but the HUD can be
    # replaced a moment later by another event, so this is retried rather than assumed - the same
    # instrument defect drive-media-page.ps1 records as its sixth.
    $opened = $false
    for ($attempt = 1; $attempt -le 5; $attempt++) {
        [TurnInput]::Move($x, $y)
        Start-Sleep -Milliseconds 400
        [TurnInput]::LeftClick()
        Start-Sleep -Milliseconds 900          # the expansion animates

        # The pointer has to stay on the frame for the wheel to reach it, and a widget page being
        # up is what the first paging gesture will prove. One click is normally enough.
        $opened = $true
        break
    }
    if (-not $opened) { throw 'The notch never opened, so there was nothing to page.' }

    # Paging. The pointer is put back on the frame before every gesture and where it landed is
    # REPORTED: a wheel event goes to the window under the cursor, so a pointer that has drifted
    # off pages nothing and looks exactly like a pager that is broken.
    #
    # 700 ms between gestures clears both guards that would swallow one: NotchPager.IdleRearmMs
    # (120) so the accumulator rests, and OsdHost.ShelfSettleMs (400) so a delta arriving after a
    # handover is not ignored as the tail of the gesture that caused it.
    #
    # A vertical wheel pages. WM_MOUSEHWHEEL is the touchpad's own path and WheelDecoder notes it
    # is still unmeasured on this machine's mouse, so the measured path is the one driven here.
    $turns = $Laps * 5
    for ($t = 1; $t -le $turns; $t++) {
        [TurnInput]::Move($x, $y)
        Start-Sleep -Milliseconds 150
        $where = [TurnInput]::Where()
        [TurnInput]::Wheel(-1)
        Start-Sleep -Milliseconds 700

        $seen = Read-TurnLines -Since $startedAtUtc
        if ($seen.Count -lt $t) { "  gesture $t -> no turn logged; $where" }
    }

    $rows = Read-TurnLines -Since $startedAtUtc
}

# --- the table ----------------------------------------------------------------------------------

""
if (-not $rows -or $rows.Count -eq 0) {
    throw ("No page turn was logged. Either nothing paged, or this Plith predates PageTurnTrace. " +
           "Check $logPath for lines reading 'Turn #'.")
}

"$($rows.Count) turn(s) measured. The shelf page is the LAST index, and its rows are marked."
""
"{0,5}  {1,7}  {2,6}  {3,7}  {4,7}  {5,5}  {6,7}  {7,6}  {8,6}  {9}" -f `
    'turn', 'pages', 'frames', 'median', 'worst', 'late', 'travel', 'step', 'still', ''
foreach ($r in $rows) {
    $isShelf = ($r.To -eq ($r.Count - 1))
    $mark = if ($isShelf) { '<- shelf' } else { '' }
    if ($r.NoFrames) {
        "{0,5}  {1,7}  {2,6}  {3,7}  {4,7}  {5,5}  {6,7}  {7,6}  {8,6}  {9}" -f `
            "#$($r.N)", "$($r.From)->$($r.To)", 0, 'NONE', 'NONE', '-', '-', '-', '-', "$mark  NO FRAMES RENDERED"
    } else {
        "{0,5}  {1,7}  {2,6}  {3,7}  {4,7}  {5,5}  {6,7}  {7,6}  {8,6}  {9}" -f `
            "#$($r.N)", "$($r.From)->$($r.To)", $r.Frames,
            ('{0:0.0}' -f $r.Median), ('{0:0.0}' -f $r.Worst), $r.Late,
            ('{0:0.00}' -f $r.Travel), ('{0:0.00}' -f $r.Step), $r.Still, $mark
    }
}

# --- the comparison, which is the whole point ---------------------------------------------------

$shelf = @($rows | Where-Object { $_.To -eq ($_.Count - 1) -and -not $_.NoFrames })
$other = @($rows | Where-Object { $_.To -ne ($_.Count - 1) -and -not $_.NoFrames })

""
if ($shelf.Count -eq 0) {
    "The run never turned onto the shelf page, so it measured no shelf row. Nothing here can be"
    "concluded about the reported symptom. Rerun with more laps, or drive it by hand with -NoDrive."
} elseif ($other.Count -eq 0) {
    "The run measured shelf turns only, with no control rows. A shelf number on its own says"
    "nothing: rerun so the ordinary turns are measured in the same session."
} else {
    function Mean { param($v) if ($v.Count -eq 0) { 0 } else { ($v | Measure-Object -Average).Average }}

    "               {0,8}  {1,8}" -f 'shelf', 'others'
    "worst ms       {0,8}  {1,8}" -f ('{0:0.0}' -f (Mean ($shelf.Worst))), ('{0:0.0}' -f (Mean ($other.Worst)))
    "late frames    {0,8}  {1,8}" -f ('{0:0.00}' -f (Mean ($shelf.Late))), ('{0:0.00}' -f (Mean ($other.Late)))
    "still frames   {0,8}  {1,8}" -f ('{0:0.00}' -f (Mean ($shelf.Still))), ('{0:0.00}' -f (Mean ($other.Still)))
    "frames         {0,8}  {1,8}" -f ('{0:0.0}' -f (Mean ($shelf.Frames))), ('{0:0.0}' -f (Mean ($other.Frames)))
    ""
    "Averages over $($shelf.Count) shelf turn(s) and $($other.Count) ordinary turn(s)."
    ""
    "If the shelf column matches the others on every row, this instrument has ruled out both of"
    "its axes and the remaining hypothesis is the one SHELF-VERIFICATION.md section 10.29 named:"
    "Plith's own shelf page draws nothing, so a perfectly smooth slide carries an empty frame in."
    "That needs a capture of the screen, not another timer."
}

if (-not $NoDrive) {
    ""
    "Leaving Plith running. Stop it with: Stop-Process -Name Plith"
}
