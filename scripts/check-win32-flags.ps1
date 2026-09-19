# scripts/check-win32-flags.ps1 - fails on Win32 calls whose flags do not say what the call means.
#
# A home for call-shape rules that the compiler cannot express and no test can reach, because the
# call is a P/Invoke into the window manager from a WPF window the suite cannot construct.
#
# Rule 1: SetWindowPos passing x, y, cx, cy of 0 must also pass SWP_NOMOVE and SWP_NOSIZE.
#
# Why this exists: the notch came back from a shelf in the top-left corner of the screen, and
# stayed there for several seconds. Measured at the physical console on 2026-09-19 with a window
# watcher: the shelf opened centred at 1088,0 and Plith's own window then reappeared at 0,0,
# 1088 px left of where it belongs.
#
# The cause was HideForCatcher and RestoreNotch calling
#
#     SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_HIDEWINDOW)
#
# The four zeros are the conventional filler for "I am not moving or sizing anything", but that
# meaning comes from the FLAGS, not from the zeros. Without SWP_NOMOVE the zeros are a real
# instruction and the window is moved to the corner. The correct call was already in the same
# file, fifty lines below, passing SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE.
#
# Nothing could catch it: it compiles, no test constructs the window, the render harness does not
# place windows on a screen, and the visible symptom only appears after a gesture a person has to
# make. It was found by a person looking at the screen, which is the most expensive way to find a
# one-token defect.
#
# Run with: pwsh -File scripts/check-win32-flags.ps1

[CmdletBinding()]
param(
    [string]$Root = (Join-Path $PSScriptRoot '..\src')
)

$ErrorActionPreference = 'Stop'
$failures = @()
$checked = 0

# Matches a SetWindowPos call whose four coordinate arguments are all literal 0, capturing the
# flag expression that follows.
#
# Scanned over the WHOLE FILE rather than line by line, and this is not a detail. The first
# version of this script matched one line at a time. Fixing the two defective calls wrapped them
# onto a second line, and the gate immediately went from inspecting three calls to inspecting
# one: it had become blind to the exact two call sites it was written to protect, while
# reporting success. A check that cannot see the thing it guards is worse than no check, because
# it also stops anyone looking.
$pattern = '(?<!extern\s\w{0,40})SetWindowPos\s*\(\s*[^,]+,\s*[^,]+,\s*0\s*,\s*0\s*,\s*0\s*,\s*0\s*,\s*(?<flags>[^)]+)\)'
$options = [Text.RegularExpressions.RegexOptions]::Singleline

foreach ($file in Get-ChildItem -Path $Root -Recurse -Include *.cs -File) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    if ($null -eq $text) { continue }

    foreach ($m in [regex]::Matches($text, $pattern, $options)) {
        $checked++
        # Both spellings are in this repository and both are correct: OsdHost declares its own
        # `const uint SWP_NOMOVE`, while BandWindow uses an enum and writes `SWP.NOMOVE`. Matching
        # only the underscore form reported BandWindow's perfectly correct call as a defect on the
        # first run of this gate, which is the other way a check can be useless.
        $flags = $m.Groups['flags'].Value
        $missing = @()
        if ($flags -notmatch 'SWP[._]NOMOVE') { $missing += 'SWP_NOMOVE' }
        if ($flags -notmatch 'SWP[._]NOSIZE') { $missing += 'SWP_NOSIZE' }

        if ($missing.Count -gt 0) {
            # Line number from the match offset, so a wrapped call still reports where it starts.
            $lineNumber = ($text.Substring(0, $m.Index) -split "`n").Count
            $rel = Resolve-Path -LiteralPath $file.FullName -Relative
            $failures += [pscustomobject]@{
                Where   = "${rel}:${lineNumber}"
                Missing = ($missing -join ' | ')
                Flags   = ($flags -replace '\s+', ' ').Trim()
            }
        }
    }
}

Write-Host "Inspected $checked SetWindowPos call(s) passing four zero coordinates."

if ($failures.Count -gt 0) {
    Write-Host "check-win32-flags.ps1 FAILED: $($failures.Count) call(s) pass 0,0,0,0 without saying so in the flags." -ForegroundColor Red
    foreach ($f in $failures) {
        Write-Host "  $($f.Where)" -ForegroundColor Red
        Write-Host "      missing: $($f.Missing)"
        Write-Host "      flags:   $($f.Flags)"
    }
    Write-Host ""
    Write-Host "Four zeros do not mean 'leave it alone'. SWP_NOMOVE and SWP_NOSIZE do."
    exit 1
}

Write-Host "Win32 flag check passed: every SetWindowPos with zero coordinates declares SWP_NOMOVE and SWP_NOSIZE." -ForegroundColor Green
exit 0
