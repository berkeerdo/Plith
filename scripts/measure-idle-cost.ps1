#requires -Version 7
<#
.SYNOPSIS
  Measures what Plith costs while it is doing nothing: CPU, and how often it wakes the UI thread.

.DESCRIPTION
  docs/PERF-VERIFICATION.md sections 1 to 3 are about the resting cost, and section 3 states the
  standard this script exists to serve: a change that removes timer wakeups has to be measured on
  BOTH axes, because "it is also the one change here that reduces timer WAKEUPS, from 33 a second
  to 2, which matters on a laptop for reasons CPU per cent does not show." Those numbers were
  taken by hand, with no instrument left behind. Section 6's watchdog item asks for the same pair
  again, before and after, so this time there is one.

  TWO AXES, and they answer different questions.

    CPU       Per cent of ONE core, from the process's own TotalProcessorTime across the window.
              Whole process, every thread. This is the axis section 1 reports and the one that is
              nearly useless at these magnitudes: section 1 measured 0,81, 0,79 and 0,15 per cent
              on three consecutive samples of the SAME build, so anything smaller than about half
              a point is inside this instrument's own spread and cannot be claimed.

    wakeups   Context switches per second, from Win32_PerfRawData_PerfProc_Thread, reported for
              the whole process and separately for the UI thread. A DispatcherTimer tick wakes a
              thread that was idle, and that wake IS a context switch, so a 250 ms probe shows up
              here as about four a second even when it is far too small to show up as CPU. This
              is the axis section 3 cared about and had no instrument for.

  WHY THE RAW COUNTER rather than Get-Counter. `\Thread(plith/*)\Context Switches/sec` does not
  filter on this machine: the instance wildcard silently matches every thread on the system and
  the total comes back at six figures. The raw class is filtered on IDProcess instead, which is
  exact, and its ContextSwitchesPersec field is cumulative since boot despite the name, so two
  readings and the clock between them give the rate directly.

  THE UI THREAD IS IDENTIFIED BY NAME, `<process>/0`, which is the thread that has existed longest
  rather than the thread that pumps the dispatcher. They are the same thread in this app, because
  WPF's Application runs on the thread that entered Main. The script prints the whole-process
  figure beside it so a wrong guess about which thread is which cannot hide a wakeup.

  WHAT THIS CANNOT SAY. It samples a process that is ALREADY at rest and it does not verify that
  it is: an open notch, a track playing, or a settings window left up all change every number
  here. Put the app at rest yourself, then run this. It also cannot attribute a wakeup to a
  timer. It counts them; deciding which timer owns them is done by changing one and measuring
  again, which is the whole method.

  Run with: pwsh -File scripts/measure-idle-cost.ps1
            pwsh -File scripts/measure-idle-cost.ps1 -Samples 5 -Seconds 20
            pwsh -File scripts/measure-idle-cost.ps1 -ProcessName Plith.DropCatcher
#>

[CmdletBinding()]
param(
    [string]$ProcessName = 'Plith',
    [int]$Samples = 3,
    [int]$Seconds = 10,
    # Seconds to wait before the first sample. A launch leaves work queued behind it, and a
    # window that starts inside that tail measures the launch rather than the rest.
    [int]$SettleSeconds = 10
)

$ErrorActionPreference = 'Stop'

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
if (-not $proc) { throw "No process named '$ProcessName' is running. Start it first." }
if ($proc -is [array]) { throw "$($proc.Count) processes named '$ProcessName' are running. Per-process figures would be ambiguous." }

function Get-ContextSwitches {
    param([int]$ProcessId)

    $threads = Get-CimInstance Win32_PerfRawData_PerfProc_Thread -Filter "IDProcess=$ProcessId"
    $total = 0
    $ui = 0
    foreach ($t in $threads) {
        $total += [double]$t.ContextSwitchesPersec
        # Suffix rather than equality: the instance name is "<process>/<index>" and the process
        # name half is not always cased the way Get-Process reports it.
        if ($t.Name -like '*/0') { $ui = [double]$t.ContextSwitchesPersec }
    }
    [pscustomobject]@{ Total = $total; Ui = $ui }
}

Write-Host ""
Write-Host "$ProcessName (pid $($proc.Id)) — $Samples samples of $Seconds s, after a $SettleSeconds s settle" -ForegroundColor Cyan
Write-Host "Put the app at rest first: notch closed, nothing playing, no settings window." -ForegroundColor DarkGray
Write-Host ""

Start-Sleep -Seconds $SettleSeconds

$rows = @()
for ($i = 1; $i -le $Samples; $i++) {
    $proc.Refresh()
    $cpu0 = $proc.TotalProcessorTime
    $cs0 = Get-ContextSwitches -ProcessId $proc.Id
    $t0 = [System.Diagnostics.Stopwatch]::StartNew()

    Start-Sleep -Seconds $Seconds

    $proc.Refresh()
    $cpu1 = $proc.TotalProcessorTime
    $cs1 = Get-ContextSwitches -ProcessId $proc.Id
    $t0.Stop()

    $elapsed = $t0.Elapsed.TotalSeconds
    $rows += [pscustomobject]@{
        sample     = $i
        'cpu %'    = [math]::Round((($cpu1 - $cpu0).TotalMilliseconds / ($elapsed * 1000)) * 100, 2)
        'cpu ms'   = [math]::Round(($cpu1 - $cpu0).TotalMilliseconds, 0)
        'cs/s all' = [math]::Round(($cs1.Total - $cs0.Total) / $elapsed, 1)
        'cs/s ui'  = [math]::Round(($cs1.Ui - $cs0.Ui) / $elapsed, 1)
    }
}

$rows | Format-Table -AutoSize

$cpuValues = $rows.'cpu %'
$uiValues = $rows.'cs/s ui'
Write-Host ("cpu %%   : {0:N2} to {1:N2} of one core" -f ($cpuValues | Measure-Object -Minimum).Minimum, ($cpuValues | Measure-Object -Maximum).Maximum)
Write-Host ("cs/s ui : {0:N1} to {1:N1}" -f ($uiValues | Measure-Object -Minimum).Minimum, ($uiValues | Measure-Object -Maximum).Maximum)
Write-Host ""
Write-Host "Read the spread, not the average. Section 1 measured 0,81 / 0,79 / 0,15 on one build." -ForegroundColor DarkGray
