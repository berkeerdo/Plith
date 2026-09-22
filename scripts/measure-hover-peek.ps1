#requires -Version 7
<#
  Measures HOVER PEEK on the real notch, which is the one path an adaptive poll rate can slow.

  THE ORACLE IS THE SCREEN, not the window. The notch window is sized to the OPEN panel and stays
  that size while parked, so its height never moves and a first version of this script read "never
  peeked" against a build that peeks fine. What changes on a peek is what is DRAWN: the surface
  grows down from the top edge, so a band of screen a little below the resting strip goes from
  desktop to notch. This samples that band and calls a peek when it stops looking like it did at
  rest.
#>
#
#   IT DRIVES THE POINTER. SetCursorPos moves the real mouse, repeatedly, for the whole run.
#   Do not start it and then use the machine: a contested pointer measures nothing, and one
#   glide in docs/PERF-VERIFICATION.md section 11 was thrown away for exactly that.
param(
    [string]$Exe = "$PSScriptRoot\..\src\Plithin\Debug
et10.0-windows10.0.22000.0\Plith.exe",
    [string]$Label = '',
    [int]$Runs = 4)

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class HP {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
}
'@

function Find-PlithWindows {
    $script:found = @()
    $cb = [HP+EnumProc]{
        param($h, $l)
        $o = [uint32]0; [void][HP]::GetWindowThreadProcessId($h, [ref]$o)
        $pn = try { (Get-Process -Id $o -ErrorAction Stop).ProcessName } catch { $null }
        if ($pn -eq 'Plith' -and [HP]::IsWindowVisible($h) -and ([HP]::GetWindowLongW($h, -20) -band 0x80000)) {
            $r = New-Object 'HP+RECT'; [void][HP]::GetWindowRect($h, [ref]$r)
            if (($r.Right - $r.Left) -gt 0) {
                $script:found += [pscustomobject]@{ Hwnd=$h; X=$r.Left; Y=$r.Top; W=($r.Right-$r.Left); H=($r.Bottom-$r.Top) }
            }
        }
        $true
    }
    [void][HP]::EnumWindows($cb, [IntPtr]::Zero)
    $script:found
}

$bmp = New-Object System.Drawing.Bitmap 80, 18
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
function Get-BandSignature {
    param([int]$X, [int]$Y)
    $gfx.CopyFromScreen($X, $Y, 0, 0, (New-Object System.Drawing.Size 80, 18))
    $sum = 0L
    for ($px = 0; $px -lt 80; $px += 4) {
        for ($py = 0; $py -lt 18; $py += 3) {
            $c = $bmp.GetPixel($px, $py)
            $sum += ($c.R * 65536L) + ($c.G * 256L) + $c.B
        }
    }
    $sum
}

Stop-Process -Name Plith -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2
Start-Process $Exe
Start-Sleep -Seconds 8

$wins = Find-PlithWindows
Write-Host ("=== {0} ===" -f $Label) -ForegroundColor Magenta
$wins | Format-Table -AutoSize
# The notch is the top-anchored window: the one whose top edge is the screen's top edge.
$notch = $wins | Where-Object { $_.Y -le 2 } | Sort-Object W | Select-Object -First 1
if (-not $notch) { throw "No top-anchored layered Plith window found." }

$cx = $notch.X + [int]($notch.W / 2)
$cy = $notch.Y + 2
# The band sits below the resting strip (a few DIP tall) and inside the peek's growth.
$bandX = $cx - 40
$bandY = $notch.Y + 14
Write-Host ("notch {0},{1} {2}x{3}; hover at {4},{5}; band at {6},{7}" -f $notch.X,$notch.Y,$notch.W,$notch.H,$cx,$cy,$bandX,$bandY)

$far = @{ X = 60; Y = 1300 }
$rows = @()

foreach ($mode in @('glide', 'teleport')) {
    for ($i = 1; $i -le $Runs; $i++) {
        [void][HP]::SetCursorPos($far.X, $far.Y)
        Start-Sleep -Milliseconds 900
        $rest = Get-BandSignature -X $bandX -Y $bandY

        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        if ($mode -eq 'glide') {
            for ($s = 1; $s -le 24; $s++) {
                $x = [int]($far.X + ($cx - $far.X) * $s / 24)
                $y = [int]($far.Y + ($cy - $far.Y) * $s / 24)
                [void][HP]::SetCursorPos($x, $y)
                Start-Sleep -Milliseconds 5
            }
        }
        [void][HP]::SetCursorPos($cx, $cy)

        $peeked = -1
        while ($sw.Elapsed.TotalMilliseconds -lt 1500) {
            if ((Get-BandSignature -X $bandX -Y $bandY) -ne $rest) { $peeked = [int]$sw.Elapsed.TotalMilliseconds; break }
            Start-Sleep -Milliseconds 4
        }
        $sw.Stop()
        $rows += [pscustomobject]@{ mode = $mode; run = $i; 'peek ms' = $peeked }
        [void][HP]::SetCursorPos($far.X, $far.Y)
        Start-Sleep -Milliseconds 700
    }
}

$rows | Format-Table -AutoSize
foreach ($m in @('glide', 'teleport')) {
    $set = @($rows | Where-Object { $_.mode -eq $m -and $_.'peek ms' -ge 0 })
    $miss = @($rows | Where-Object { $_.mode -eq $m -and $_.'peek ms' -lt 0 }).Count
    if ($set.Count) {
        Write-Host ("{0,-9} peeked {1}/{2}, {3} to {4} ms" -f $m, $set.Count, $Runs,
            ($set.'peek ms' | Measure-Object -Minimum).Minimum, ($set.'peek ms' | Measure-Object -Maximum).Maximum)
    }
    if ($miss) { Write-Host ("{0,-9} NEVER PEEKED on {1} run(s)" -f $m, $miss) -ForegroundColor Red }
}
$gfx.Dispose(); $bmp.Dispose()
