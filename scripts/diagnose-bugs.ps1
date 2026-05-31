# Diagnostic PowerShell for Phase 4i investigation. Gathers evidence for three
# reported bugs without changing any code. Run from an admin PowerShell while
# Plith is currently running so we can introspect the live HWNDs.
#
# Bugs being investigated:
#   1. Boot race — Plith starts but OSD doesn't pop on volume change
#   2. Windows search "plith" returns no results
#   3. Plith appears in Alt+Tab even when Settings is closed

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'
$banner = '=' * 64

function Section($name) {
    Write-Host ""
    Write-Host $banner -ForegroundColor Cyan
    Write-Host "  $name" -ForegroundColor Cyan
    Write-Host $banner -ForegroundColor Cyan
}

# ---------------------------------------------------------------------------
Section "Plith process snapshot"
# ---------------------------------------------------------------------------

$plith = Get-Process -Name Plith -ErrorAction SilentlyContinue
if (-not $plith) {
    Write-Host "  Plith is NOT running. Start it first, then re-run this script." -ForegroundColor Yellow
} else {
    foreach ($p in $plith) {
        Write-Host ("  PID {0}  path={1}" -f $p.Id, $p.MainModule.FileName)
        Write-Host ("    Start time : {0}" -f $p.StartTime)
        Write-Host ("    Threads    : {0}" -f $p.Threads.Count)
        Write-Host ("    Main HWND  : 0x{0:X8}" -f $p.MainWindowHandle.ToInt64())
    }
}

# ---------------------------------------------------------------------------
Section "Bug 3 — Top-level HWNDs owned by Plith + extended-style flags"
# ---------------------------------------------------------------------------

$code = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class HwndEnum {
    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc enumProc, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    private static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int nIndex);

    [DllImport("user32.dll")]
    private static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

    [DllImport("user32.dll")]
    private static extern int GetClassName(IntPtr hWnd, StringBuilder text, int count);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern IntPtr GetWindow(IntPtr hWnd, uint uCmd);

    private const int GWL_STYLE = -16;
    private const int GWL_EXSTYLE = -20;
    private const uint GW_OWNER = 4;

    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    public class WindowInfo {
        public IntPtr Hwnd;
        public string Title;
        public string ClassName;
        public long Style;
        public long ExStyle;
        public bool Visible;
        public IntPtr Owner;
    }

    public static List<WindowInfo> ForProcess(uint targetPid) {
        var result = new List<WindowInfo>();
        EnumWindows((hwnd, lparam) => {
            GetWindowThreadProcessId(hwnd, out uint pid);
            if (pid != targetPid) return true;
            var sbTitle = new StringBuilder(512);
            GetWindowText(hwnd, sbTitle, sbTitle.Capacity);
            var sbClass = new StringBuilder(256);
            GetClassName(hwnd, sbClass, sbClass.Capacity);
            result.Add(new WindowInfo {
                Hwnd = hwnd,
                Title = sbTitle.ToString(),
                ClassName = sbClass.ToString(),
                Style = (long)GetWindowLongPtr(hwnd, GWL_STYLE),
                ExStyle = (long)GetWindowLongPtr(hwnd, GWL_EXSTYLE),
                Visible = IsWindowVisible(hwnd),
                Owner = GetWindow(hwnd, GW_OWNER),
            });
            return true;
        }, IntPtr.Zero);
        return result;
    }
}
'@

Add-Type -TypeDefinition $code -Language CSharp

if ($plith) {
    foreach ($p in $plith) {
        Write-Host ""
        Write-Host ("  Top-level windows owned by PID {0}:" -f $p.Id) -ForegroundColor White
        $windows = [HwndEnum]::ForProcess([uint32]$p.Id)
        foreach ($w in $windows) {
            $exFlags = @()
            if ($w.ExStyle -band 0x00000008) { $exFlags += 'TOPMOST' }
            if ($w.ExStyle -band 0x00000020) { $exFlags += 'TRANSPARENT' }
            if ($w.ExStyle -band 0x00000080) { $exFlags += 'TOOLWINDOW' }
            if ($w.ExStyle -band 0x00040000) { $exFlags += 'APPWINDOW' }
            if ($w.ExStyle -band 0x00080000) { $exFlags += 'LAYERED' }
            if ($w.ExStyle -band 0x08000000) { $exFlags += 'NOACTIVATE' }
            if ($w.ExStyle -band 0x00200000) { $exFlags += 'NOREDIRECTIONBITMAP' }

            $visTag = if ($w.Visible) { 'VISIBLE' } else { 'hidden' }
            $altTabPredict = if (($w.ExStyle -band 0x00000080) -or ($w.Owner -ne [IntPtr]::Zero)) { 'NOT in AltTab' } else { 'WILL appear in AltTab' }

            Write-Host ""
            Write-Host ("    HWND 0x{0:X8}  '{1}'  class='{2}'" -f $w.Hwnd.ToInt64(), $w.Title, $w.ClassName)
            Write-Host ("      style   = 0x{0:X8}" -f $w.Style)
            Write-Host ("      exStyle = 0x{0:X8}  [{1}]" -f $w.ExStyle, ($exFlags -join ' | '))
            Write-Host ("      state   = {0}  owner=0x{1:X}" -f $visTag, $w.Owner.ToInt64())
            Write-Host ("      AltTab  = {0}" -f $altTabPredict) -ForegroundColor (if ($altTabPredict -eq 'WILL appear in AltTab') { 'Red' } else { 'Green' })
        }
    }
}

# ---------------------------------------------------------------------------
Section "Bug 2 — Start menu shortcut + Windows Search service"
# ---------------------------------------------------------------------------

$lnkPath = Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\Plith.lnk'
if (Test-Path $lnkPath) {
    Write-Host "  Plith.lnk EXISTS at: $lnkPath" -ForegroundColor Green
    $wsh = New-Object -ComObject WScript.Shell
    $sc = $wsh.CreateShortcut($lnkPath)
    Write-Host "    TargetPath       : $($sc.TargetPath)"
    Write-Host "    WorkingDirectory : $($sc.WorkingDirectory)"
    Write-Host "    IconLocation     : $($sc.IconLocation)"
    Write-Host "    Description      : $($sc.Description)"
    if (-not (Test-Path $sc.TargetPath)) {
        Write-Host "    WARNING: TargetPath points at a missing file." -ForegroundColor Red
    }
} else {
    Write-Host "  Plith.lnk MISSING at: $lnkPath" -ForegroundColor Red
    Write-Host "  Search index can't find what isn't there." -ForegroundColor Yellow
}

# Per-user Start menu fallback (some installers write there too)
$userLnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Plith.lnk'
if (Test-Path $userLnk) {
    Write-Host "  Also found per-user shortcut: $userLnk"
}

# Windows Search service
$wsearch = Get-Service -Name WSearch -ErrorAction SilentlyContinue
if ($wsearch) {
    $color = if ($wsearch.Status -eq 'Running') { 'Green' } else { 'Red' }
    Write-Host ("  WSearch service status: {0}" -f $wsearch.Status) -ForegroundColor $color
} else {
    Write-Host "  WSearch service NOT installed?" -ForegroundColor Red
}

# Check whether the Start Menu folder is indexed
try {
    $reg = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows Search\Gather\Windows\SystemIndex\Sites\Windows Search Service\Crawl Scope Manager\WorkingSetRules' -ErrorAction SilentlyContinue
    if ($reg) {
        Write-Host "  Search index scope rules present (full inspection deferred — manual check via Indexing Options is faster)."
    }
} catch { }

# ---------------------------------------------------------------------------
Section "Bug 1 — Boot-time autostart + any prior unhandled exception in Application log"
# ---------------------------------------------------------------------------

$runValue = Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'Plith' -ErrorAction SilentlyContinue
if ($runValue) {
    Write-Host "  HKCU\Run Plith entry: $($runValue.Plith)" -ForegroundColor Green
    # Resolve the actual exe path from the (possibly quoted) value
    $candidate = $runValue.Plith.Trim('"')
    if (Test-Path $candidate) {
        Write-Host "  Target exists: $candidate"
    } else {
        Write-Host "  WARNING: Target does not exist on disk: $candidate" -ForegroundColor Red
    }
} else {
    Write-Host "  No HKCU\Run autostart entry — Plith will NOT launch at boot." -ForegroundColor Yellow
    Write-Host "  Enable 'Launch at Windows login' in Settings if you expect boot autostart." -ForegroundColor Yellow
}

# Recent Application log entries mentioning Plith
Write-Host ""
Write-Host "  Recent Application log entries mentioning 'plith' (last 24h):"
try {
    $events = Get-WinEvent -LogName Application -MaxEvents 200 -ErrorAction SilentlyContinue |
        Where-Object { $_.TimeCreated -gt (Get-Date).AddDays(-1) -and ($_.Message -match 'plith' -or $_.ProviderName -match 'plith') } |
        Select-Object -First 10
    if ($events) {
        foreach ($e in $events) {
            Write-Host ("    [{0}] {1} {2}" -f $e.TimeCreated, $e.LevelDisplayName, $e.ProviderName) -ForegroundColor Yellow
            Write-Host ("      {0}" -f ($e.Message.Substring(0, [Math]::Min(200, $e.Message.Length))))
        }
    } else {
        Write-Host "    (no Plith-related entries in the last 24h)"
    }
} catch {
    Write-Host "    (could not read Application log: $_)"
}

# Plith's own LOCALAPPDATA folder — settings + any future log location
$plithLocal = Join-Path $env:LOCALAPPDATA 'Plith'
if (Test-Path $plithLocal) {
    Write-Host ""
    Write-Host "  %LOCALAPPDATA%\Plith\ contents:"
    Get-ChildItem $plithLocal -Recurse -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host ("    {0}  ({1} bytes, modified {2})" -f $_.FullName.Replace($plithLocal, '...'), $_.Length, $_.LastWriteTime)
    }
}

Write-Host ""
Write-Host $banner -ForegroundColor Cyan
Write-Host "  Diagnostic complete. Paste the output into the conversation." -ForegroundColor Cyan
Write-Host $banner -ForegroundColor Cyan
