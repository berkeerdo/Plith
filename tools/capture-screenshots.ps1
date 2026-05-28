# Captures Plith UI elements into docs/screenshots/.
# - settings.png: full Settings window
# - osd.png: OSD card (only visible when triggered — caller pops one before this script runs)
# Uses System.Drawing.Graphics + Win32 GetWindowRect, no external tools.

param(
    [string]$OutDir = "$PSScriptRoot\..\docs\screenshots"
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class Win {
    [DllImport("user32.dll")]
    public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);

    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool EnumWindows(EnumWindowsProc enumProc, IntPtr lParam);

    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("dwmapi.dll")]
    public static extern int DwmGetWindowAttribute(IntPtr hwnd, int dwAttribute, out RECT pvAttribute, int cbAttribute);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

function Get-WindowHandlesForProcess([int]$pid) {
    $handles = New-Object System.Collections.ArrayList
    $proc = [Win+EnumWindowsProc]{
        param($hWnd, $lParam)
        $procId = 0
        [Win]::GetWindowThreadProcessId($hWnd, [ref]$procId) | Out-Null
        if ($procId -eq $pid -and [Win]::IsWindowVisible($hWnd)) {
            $sb = New-Object System.Text.StringBuilder(256)
            [Win]::GetWindowText($hWnd, $sb, 256) | Out-Null
            $rect = New-Object Win+RECT
            [Win]::GetWindowRect($hWnd, [ref]$rect) | Out-Null
            $w = $rect.Right - $rect.Left
            $h = $rect.Bottom - $rect.Top
            if ($w -gt 10 -and $h -gt 10) {
                $handles.Add(@{ Handle = $hWnd; Title = $sb.ToString(); Rect = $rect; W = $w; H = $h }) | Out-Null
            }
        }
        return $true
    }
    [Win]::EnumWindows($proc, [IntPtr]::Zero) | Out-Null
    return $handles
}

function Capture-Rect([Win+RECT]$rect, [string]$path) {
    $w = $rect.Right - $rect.Left
    $h = $rect.Bottom - $rect.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, [System.Drawing.Size]::new($w, $h))
    $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "Saved $path ($w x $h)"
}

$plith = Get-Process -Name Plith -ErrorAction SilentlyContinue
if (-not $plith) {
    Write-Host "Plith is not running — launch it first." -ForegroundColor Red
    exit 1
}

$windows = Get-WindowHandlesForProcess $plith.Id
Write-Host "Found $($windows.Count) visible Plith windows:"
$windows | ForEach-Object { Write-Host ("  [{0}] '{1}' {2}x{3}" -f $_.Handle, $_.Title, $_.W, $_.H) }

# Heuristics: Settings window has 'Plith' in the title and tall height. OSD is short and wide.
$settings = $windows | Where-Object { $_.H -gt 400 } | Select-Object -First 1
$osd = $windows | Where-Object { $_.H -lt 400 -and $_.W -gt 200 } | Select-Object -First 1

if ($settings) {
    Capture-Rect $settings.Rect (Join-Path $OutDir "settings.png")
} else {
    Write-Host "No Settings window — open it from the tray first." -ForegroundColor Yellow
}

if ($osd) {
    Capture-Rect $osd.Rect (Join-Path $OutDir "osd.png")
} else {
    Write-Host "No OSD visible — change volume or hit the summon hotkey first." -ForegroundColor Yellow
}
