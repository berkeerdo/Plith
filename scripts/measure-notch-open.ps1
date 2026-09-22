#requires -Version 7
<#
.SYNOPSIS
  Measures what the notch's own first open costs, and separates the wait from the animation.

.DESCRIPTION
  docs/PERF-VERIFICATION.md section 5 carried this as the one reported symptom with no
  instrument behind it: a person saw the mouse go busy for about a second on first use, and
  nothing in the repo could time it. This is that instrument.

  It measures nothing itself. Plith's own NotchOpenTrace writes one line per open into
  %LOCALAPPDATA%\Plith\plith.log, and this script drives the clicks and tabulates the lines.
  Timestamps written from inside the process are the only thing that reports the same way
  whatever integrity level Plith is running at, so one instrument serves every build.

  MEASURED, and it corrected this script's own first draft: a signed Release Plith runs UIAccess
  at HIGH integrity, and SendInput from a MEDIUM-integrity shell still reaches it. Accepted, no
  error, the notch opened. UIPI blocks window messages sent AT a higher-integrity window; it does
  not block injection into the raw input stream, which the system then delivers by hit test. The
  first version of this header asserted the opposite and was wrong. So -NoDrive is not the only
  way to measure an installed build, and -ExePath is the better one.

  Two modes:

    (default)  Stops any running Plith, starts the one at -ExePath, clicks the notch -Runs times,
               and prints the table. Stopping the running instance is not optional: an instance
               that has already opened its frame once has no first open left to measure.

    -NoDrive   Starts nothing and clicks nothing. It watches the log and tabulates what appears
               while YOU click the notch. For a Plith this shell did not start and must not
               restart, or when an anti-cheat driver is filtering injected input.

  What the four intervals mean is documented on NotchOpenTrace itself, not repeated here, so the
  two cannot drift apart. The one number to read first is the stall: it is the only column that
  measures the reported symptom rather than the duration of the open.

  Run with: pwsh -File scripts/measure-notch-open.ps1
            pwsh -File scripts/measure-notch-open.ps1 -ExePath 'C:\Program Files\Plith\Plith.exe'
            pwsh -File scripts/measure-notch-open.ps1 -NoDrive -Runs 4
#>

[CmdletBinding()]
param(
    [int]$Runs = 6,
    [string]$Configuration = 'Debug',
    [string]$ExePath,
    [switch]$NoDrive,
    [int]$TimeoutSeconds = 180
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
$plithExe = if ($ExePath) { $ExePath } else { Join-Path $bin 'Plith.exe' }
$logPath = Join-Path $env:LOCALAPPDATA 'Plith\plith.log'

# --- input, copied from drive-media-page.ps1 ----------------------------------------------------
#
# Copied rather than imported, which is what drive-shelf.ps1, drive-shelf-pair.ps1 and
# drive-media-page.ps1 already do with each other. Movement is SetCursorPos and NOT SendInput,
# and a partially accepted batch throws: both are drive-shelf-pair's defect 4, measured with an
# anti-cheat driver loaded, where LEFTDOWN was refused intermittently while LEFTUP was accepted
# in the same second. Half a click measures a meaningless result with the product blameless.

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class OpenInput {
    // The union is not decoration. INPUT must marshal to exactly 40 bytes on x64, and a
    // "simplified" layout with MOUSEINPUT inline plus padding measures 48 - which SendInput
    // rejects with Win32 error 87 and no other clue. Measured on this machine at the first run
    // of this script. Copied from drive-media-page.ps1 rather than rederived, for that reason.
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

    const uint LEFTDOWN=0x0002, LEFTUP=0x0004;

    static void Send(INPUT[] a, string what) {
        uint n = SendInput((uint)a.Length, a, Marshal.SizeOf(typeof(INPUT)));
        if (n != a.Length) throw new Exception("SendInput refused " + what + ": accepted " + n +
            " of " + a.Length + ", Win32 error " + Marshal.GetLastWin32Error() +
            " (87 with an anti-cheat driver loaded means injected input is being filtered; " +
            "5 means this shell is below the target window's integrity level)");
    }
    static INPUT Mouse(uint flags) { INPUT i = new INPUT(); i.type = 0; i.u.mi.dwFlags = flags; return i; }

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
}
public static class OpenWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out OpenInput.RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
}
'@

function Find-LayeredWindows {
    param([string]$ProcessLike)
    $script:found = @()
    $cb = [OpenWin+EnumProc]{
        param($h, $l)
        $o = [uint32]0; [void][OpenWin]::GetWindowThreadProcessId($h, [ref]$o)
        $pn = try { (Get-Process -Id $o -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -and $pn -like $ProcessLike -and [OpenWin]::IsWindowVisible($h) -and
            ([OpenWin]::GetWindowLongW($h, -20) -band 0x80000)) {
            $r = New-Object 'OpenInput+RECT'; [void][OpenWin]::GetWindowRect($h, [ref]$r)
            if (($r.Right - $r.Left) -gt 0) {
                $script:found += [pscustomobject]@{ Hwnd=$h; Pid=[int]$o; X=$r.Left; Y=$r.Top
                    W=($r.Right-$r.Left); H=($r.Bottom-$r.Top) }
            }
        }
        $true
    }
    [void][OpenWin]::EnumWindows($cb, [IntPtr]::Zero)
    $script:found
}

# --- reading the log ----------------------------------------------------------------------------
#
# Filtered by timestamp rather than by byte offset. An offset is shorter but breaks silently if
# DiagnosticLog rotates mid-run, and a silent break here would read as "the notch never opened".

function Read-OpenLines {
    param([datetime]$Since)
    if (-not (Test-Path $logPath)) { return @() }

    # Shared read: Plith holds the file open for append, and a plain Get-Content would throw.
    $text = try {
        $fs = [System.IO.File]::Open($logPath, 'Open', 'Read', 'ReadWrite')
        try { (New-Object System.IO.StreamReader($fs)).ReadToEnd() } finally { $fs.Dispose() }
    } catch { return @() }

    $rowList = [System.Collections.Generic.List[object]]::new()
    foreach ($line in $text -split "`r?`n") {
        if ($line -notmatch '^\[(?<ts>[^\]]+)\]\s+\[\w+\]\s+\[OsdHost\]\s+(?<body>Open #\d+: .*)$') { continue }
        $stamp = $Matches.ts
        $body = $Matches.body
        $ts = [datetime]::Parse($stamp, [cultureinfo]::InvariantCulture,
                                ([System.Globalization.DateTimeStyles]::AdjustToUniversal -bor
                                 [System.Globalization.DateTimeStyles]::AssumeUniversal))
        if ($ts -lt $Since) { continue }
        if ($body -notmatch 'Open #(?<n>\d+): (?<total>\d+)ms total \(layout (?<layout>\d+), defer (?<defer>\d+), show (?<show>\d+), settle (?<settle>\d+)\), max UI stall (?<stall>\d+)ms over (?<ticks>\d+) ticks') { continue }
        $rowList.Add([pscustomobject]@{
            N = [int]$Matches.n; Total = [int]$Matches.total; Layout = [int]$Matches.layout
            Defer = [int]$Matches.defer; Show = [int]$Matches.show; Settle = [int]$Matches.settle
            Stall = [int]$Matches.stall; Ticks = [int]$Matches.ticks
        })
    }

    # An ordinal is unique per process, so a duplicate means two Plith processes wrote into one
    # log. Keeping the first of each would average two machines' worth of state into one table.
    foreach ($g in ($rowList | Group-Object N)) {
        if ($g.Count -gt 1) {
            throw ("Two lines claim Open #$($g.Name). More than one Plith wrote to this log " +
                   "during the run, so the table would mix two processes. Stop them all and rerun.")
        }
    }
    @($rowList | Sort-Object N)
}

# --- the run ------------------------------------------------------------------------------------

$startedAtUtc = (Get-Date).ToUniversalTime().AddSeconds(-1)
"log: $logPath"

if ($NoDrive) {
    "mode: watching only. Nothing will be started and nothing will be clicked."
    ""
    "  Open the notch $Runs times by hand, letting it collapse fully between opens."
    "  The first open is the one under suspicion, so click it BEFORE doing anything else"
    "  with the app: a settings window or a volume key can lay the widget pages out early"
    "  and quietly spend the cost this run exists to find."
    ""
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $rows = @()
    while ((Get-Date) -lt $deadline -and $rows.Count -lt $Runs) {
        Start-Sleep -Milliseconds 500
        $next = Read-OpenLines -Since $startedAtUtc
        if ($next.Count -gt $rows.Count) { "  saw open #$($next[-1].N)" }
        $rows = $next
    }
} else {
    if (-not (Test-Path $plithExe)) {
        throw "No Plith at $plithExe. Run: dotnet build src\Plith\Plith.csproj -c $Configuration, or pass -ExePath."
    }

    # Not optional, and not merely tidiness. An instance that has already opened its frame has
    # laid the widget pages out, so its first open is spent and cannot be measured again. This
    # is also why the script refuses to reuse a running Plith the way drive-media-page.ps1 does.
    $already = Get-Process Plith -ErrorAction SilentlyContinue
    if ($already) {
        "plith: stopping the running instance (pid $($already.Id -join ', ')). A first open can only be measured on a process that has not had one yet."
        $already | Stop-Process
        Start-Sleep -Seconds 2
    }

    [OpenInput]::Move(400, 400)
    $landed = New-Object 'OpenInput+POINT'; [void][OpenInput]::GetCursorPos([ref]$landed)
    if ([Math]::Abs($landed.X - 400) -gt 2 -or [Math]::Abs($landed.Y - 400) -gt 2) {
        throw ("SetCursorPos reported success but the pointer read back at $($landed.X),$($landed.Y). " +
               "Injected input is not taking effect, so nothing below would be measuring the notch.")
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

    # Settle before the first click. Not padding: AttachAudioSource builds the widget pages, and
    # clicking before it has run would measure an open of a frame that has no pages in it.
    Start-Sleep -Seconds 3

    # Its own window's top-centre, a few pixels down: the resting shape is a thin strip at the top
    # of that window, and the window is wider than the strip.
    $x = [int](($notch.X + $notch.X + $notch.W) / 2)
    $y = [int]($notch.Y + 3)

    $rows = @()
    for ($run = 1; $run -le $Runs; $run++) {
        [OpenInput]::Move($x, $y)
        Start-Sleep -Milliseconds 400
        [OpenInput]::LeftClick()

        # Wait for the line rather than for a fixed time. A fixed sleep that is too short reports
        # a fast open as a missing one, which is the failure mode most likely to be believed.
        $seen = $false
        for ($w = 0; $w -lt 20; $w++) {
            Start-Sleep -Milliseconds 250
            $next = Read-OpenLines -Since $startedAtUtc
            if ($next.Count -ge $run) { $rows = $next; $seen = $true; break }
        }
        if (-not $seen) { "  run ${run}: no line appeared within 5 s" }

        # Away from the notch so hover cannot hold it open, then long enough for the 2 s hide
        # timer and the 260 ms collapse.
        [OpenInput]::Move(400, 400)
        Start-Sleep -Milliseconds 3200
    }

    Get-Process Plith, Plith.DropCatcher -ErrorAction SilentlyContinue | Stop-Process
    "plith: stopped"
}

# --- the table ----------------------------------------------------------------------------------

""
if (-not $rows -or $rows.Count -eq 0) {
    throw ("No open lines were written. Either the notch is not the live presentation, its " +
           "widgets are off in settings, or this build predates NotchOpenTrace.")
}

"  open    total   layout    defer     show   settle   UI stall  samples"
"  ----  -------  -------  -------  -------  -------  ---------  -------"
foreach ($r in $rows) {
    "  {0,4}  {1,5}ms  {2,5}ms  {3,5}ms  {4,5}ms  {5,5}ms  {6,7}ms  {7,7}" -f `
        $r.N, $r.Total, $r.Layout, $r.Defer, $r.Show, $r.Settle, $r.Stall, $r.Ticks
}
""

$first = $rows[0]
$rest = @($rows | Select-Object -Skip 1)
if ($rest.Count -gt 0) {
    $restLayout = ($rest | Measure-Object Layout -Average).Average
    $restTotal = ($rest | Measure-Object Total -Average).Average
    $restStall = ($rest | Measure-Object Stall -Maximum).Maximum
    "  first open:   $($first.Total)ms total, $($first.Layout)ms of it laying the pages out"
    "  later opens:  $([int]$restTotal)ms total average, $([int]$restLayout)ms layout average, over $($rest.Count)"
    "  worst stall:  $($first.Stall)ms on the first open, ${restStall}ms on the rest"
    ""
    "  The probe ticks every 15 ms, so an open shorter than that is sampled 0 or 1 times and its"
    "  stall column says nothing. Read the samples column before believing a zero."
} else {
    "  Only one open was recorded, so there is no steady state to compare it against."
}
