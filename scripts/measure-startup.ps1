#requires -Version 7
<#
.SYNOPSIS
  Measures what one launch of Plith costs, and how long it blocks the UI thread doing it.

.DESCRIPTION
  docs/PERF-VERIFICATION.md section 6 named app startup the prime suspect for a reported symptom
  and had nothing to measure it with: a person saw the mouse go busy for about a second on first
  use, and section 5 cleared the notch's own first open at 26 to 36 ms, which is about two per
  cent of it. The report said "first use" rather than "first open", and nothing in the repo had
  ever timed what happens between launching Plith and the notch being ready. This is that
  instrument.

  It measures nothing itself. Plith's own StartupTrace writes one line per launch into
  %LOCALAPPDATA%\Plith\plith.log, and this script starts the process and tabulates the lines.
  Timestamps written from inside the process are the only thing that reports the same way whatever
  integrity level Plith is running at, so one instrument serves every build.

  Two modes:

    (default)  Stops any running Plith and its catcher, starts the one at -ExePath, waits for the
               line, and repeats -Runs times. Stopping the running instance is not optional: the
               single-instance mutex means a second Plith exits immediately and silently, and a
               process that never started has no launch to measure.

    -NoDrive   Starts nothing and stops nothing. It watches the log and tabulates what appears
               while YOU launch Plith. For an installed build this shell must not restart, or when
               you want to measure a launch that happened the ordinary way, from the Start menu or
               at login.

  WHAT THE COLUMNS MEAN is documented on StartupTrace itself, not repeated here, so the two cannot
  drift apart. Three things are worth saying beside the table rather than inside the type:

    - The ten spans PARTITION the launch. Adding them up lands on the total, so a column can be
      read as "this phase cost that much" without any of it being double counted.

    - `clr` is the one column that is not a Stopwatch reading, because the interval it measures is
      over before a Stopwatch can exist. It is good to a few milliseconds and it is not the same
      quality of measurement as the nine beside it. See Program.RuntimeStartMs.

    - `stall` is the only column that measures the reported symptom. A launch that takes a second
      while pumping input does not put the busy cursor on screen; one that blocks for 300 ms does.
      Read the tick count beside it: over 0 ticks means the probe never looked, which is not the
      same as nothing having blocked.

  The first run of a session is not comparable to the rest and the table says so. Windows caches
  the binary and its dependencies after the first launch, so a cold first run and a warm sixth are
  two different measurements sharing a column.

  Run with: pwsh -File scripts/measure-startup.ps1
            pwsh -File scripts/measure-startup.ps1 -ExePath 'C:\Program Files\Plith\Plith.exe'
            pwsh -File scripts/measure-startup.ps1 -NoDrive -Runs 3
#>

[CmdletBinding()]
param(
    [int]$Runs = 5,
    [string]$Configuration = 'Debug',
    [string]$ExePath,
    [switch]$NoDrive,
    [int]$TimeoutSec = 30
)

$ErrorActionPreference = 'Stop'

$repo = Split-Path -Parent $PSScriptRoot
$log = Join-Path $env:LOCALAPPDATA 'Plith\plith.log'

if (-not $ExePath) {
    $ExePath = Join-Path $repo "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0\Plith.exe"
}

if (-not $NoDrive -and -not (Test-Path $ExePath)) {
    throw "No Plith.exe at $ExePath. Build it first, or pass -ExePath."
}

if (-not (Test-Path $log)) {
    throw "No log at $log. Plith has to have run at least once for the file to exist."
}

# The first launch line written after a given moment, or nothing yet.
#
# Selected by the timestamp the line carries rather than by counting lines or bytes, and that is a
# defect this script had on its first run rather than a precaution. DiagnosticLog rotates at a
# size cap: the live file is renamed to .1 and a new one started. Both a byte offset and a line
# count point at the wrong place the moment that happens, and what it looks like is a launch that
# never reported. It cost the fifth run of the first real measurement, which timed out while Plith
# had in fact started and logged normally.
#
# The newest line is always in the live file, because rotation happens on the way to writing it.
function Get-LaunchLineAfter([datetime]$marker) {
    foreach ($match in (Get-Content $log -ErrorAction SilentlyContinue | Select-String 'Perf\] Startup: ')) {
        $text = $match.ToString()
        if ($text -notmatch '^\[([^\]]+)\]') { continue }

        [datetime]$stamp = [datetime]::MinValue
        if (-not [datetime]::TryParse($Matches[1], [cultureinfo]::InvariantCulture,
                                      [Globalization.DateTimeStyles]::AdjustToUniversal -bor
                                      [Globalization.DateTimeStyles]::AssumeUniversal, [ref]$stamp)) { continue }

        if ($stamp -gt $marker) { return $text }
    }
    return $null
}

function Stop-Plith {
    Get-Process Plith, Plith.DropCatcher -ErrorAction SilentlyContinue | Stop-Process -Force
    # The mutex is released by the OS on process exit, but the exit is not instant and a launch
    # racing it would be the silent no-op this script exists to avoid.
    Start-Sleep -Milliseconds 700
}

# "Startup: 898ms total (clr 53, app 78, ...), max UI stall 972ms over 1 ticks"
function Read-LaunchLine([string]$line) {
    if ($line -notmatch 'Startup: (\d+)ms total \((.+?)\), max UI stall (\d+)ms over (\d+) ticks') { return $null }

    $row = [ordered]@{ total = [int]$Matches[1] }
    foreach ($span in $Matches[2] -split ',\s*') {
        $parts = $span.Trim() -split '\s+'
        $row[$parts[0]] = [int]$parts[1]
    }
    $row['stall'] = [int]$Matches[3]
    $row['ticks'] = [int]$Matches[4]
    [pscustomobject]$row
}

$rows = @()

if ($NoDrive) {
    Write-Host "Watching $log. Launch Plith $Runs time(s); Ctrl+C to stop." -ForegroundColor Cyan
} else {
    Write-Host "Driving $ExePath, $Runs launch(es)." -ForegroundColor Cyan
}

for ($i = 1; $i -le $Runs; $i++) {
    # Before the launch, and before the stop, because neither writes a launch line.
    $marker = [datetime]::UtcNow

    if (-not $NoDrive) {
        Stop-Plith
        Start-Process -FilePath $ExePath | Out-Null
    }

    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $line = $null
    while ((Get-Date) -lt $deadline) {
        $line = Get-LaunchLineAfter $marker
        if ($line) { break }
        Start-Sleep -Milliseconds 200
    }

    if (-not $line) {
        Write-Warning "Run ${i}: no launch line within $TimeoutSec s."
        continue
    }

    $row = Read-LaunchLine $line
    if (-not $row) {
        Write-Warning "Run ${i}: a launch line that did not parse. $line"
        continue
    }

    $row | Add-Member -NotePropertyName run -NotePropertyValue $i
    $rows += $row
    Write-Host ("  run {0}: {1} ms total, stall {2} ms over {3} ticks" -f $i, $row.total, $row.stall, $row.ticks)
}

if (-not $rows) { throw 'Nothing measured.' }

Write-Host ''
$rows | Format-Table run, total, clr, app, settings, cards, window, audio, hooks, tray, shelf, settle, stall, ticks -AutoSize

Write-Host 'The first run of a session is cold and the rest are warm. They are not the same measurement.' -ForegroundColor DarkGray
Write-Host 'Columns partition the total; clr is wall-clock and the rest are one Stopwatch; stall is the only one that measures the busy cursor.' -ForegroundColor DarkGray

if (-not $NoDrive) {
    Write-Host ''
    Write-Host 'Plith is left running from the last launch. Stop it, or restart your installed build, as you prefer.' -ForegroundColor DarkGray
}
