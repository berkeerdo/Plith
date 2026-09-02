#requires -Version 7
<#
.SYNOPSIS
  Fails when an interactive XAML control carries no accessible name.

.DESCRIPTION
  AutomationProperties live in XAML, where unit tests are weak — asserting on them needs an
  STA thread and a loaded visual tree. This static check is the regression guard instead.

  A control passes when it declares AutomationProperties.Name (including an explicitly empty
  one, which marks a decorative element) or AutomationProperties.LabeledBy.

  Exemption for template parts: an interactive element that carries Focusable="False" is
  exempt from this check. Focusable="False" is a WPF semantic, not a positional one — it means
  the element can never receive keyboard focus, so a screen reader can never land on it via
  Tab/arrow navigation, and giving it an accessible name would be either dead weight or, worse,
  actively misleading (announcing a control the user can't actually reach). The concrete case
  this covers is the ToggleButton chevron inside ModernComboStyle's ControlTemplate in
  SettingsTheme.xaml: it is a template part of the combo box, not a user-facing control, and is
  correctly marked Focusable="False".
  We deliberately did NOT choose "skip anything inside a <ControlTemplate>" — that's a
  structural heuristic that would also blind the script to a genuinely focusable, user-facing
  control that someone later drops into a template by mistake. Tying the exemption to
  Focusable="False" keeps the check honest: it only exempts elements that are provably
  unreachable by keyboard/assistive tech, not merely elements that live in a template file.
#>
[CmdletBinding()]
param(
    # Defaults to src/Plith only. src/Plith.Installer is a separate WPF project that this
    # phase's accessibility work (Tasks 10-13) never touched or reviewed — its own test suite
    # is explicitly out of scope for this phase too. Scanning it here would fail the guard on
    # pre-existing gaps nobody has signed off on fixing yet, for controls this script's author
    # has no context to name correctly. Pass -Root explicitly to check the installer once it
    # gets its own accessibility pass.
    [string] $Root = (Join-Path $PSScriptRoot '..' 'src' 'Plith')
)

$ErrorActionPreference = 'Stop'

$interactive = @('Button', 'ComboBox', 'Slider', 'ToggleButton', 'CheckBox', 'TextBox', 'RadioButton')
$failures = [System.Collections.Generic.List[string]]::new()

foreach ($file in Get-ChildItem -Path $Root -Filter '*.xaml' -Recurse) {
    $lines = Get-Content -LiteralPath $file.FullName
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $match = [regex]::Match($lines[$i], '<(' + ($interactive -join '|') + ')[\s>]')
        if (-not $match.Success) { continue }

        # Element attributes can wrap across lines; scan forward to the tag's closing bracket.
        $element = ''
        for ($j = $i; $j -lt $lines.Count; $j++) {
            $element += $lines[$j]
            if ($lines[$j] -match '/?>\s*$') { break }
        }

        if ($element -match 'Focusable\s*=\s*"False"') {
            # Not reachable via keyboard/assistive tech (e.g. a template part) — see header note.
            continue
        }

        if ($element -notmatch 'AutomationProperties\.(Name|LabeledBy)') {
            $rel = Resolve-Path -Relative -LiteralPath $file.FullName
            $failures.Add("$rel($($i + 1)): <$($match.Groups[1].Value)> has no AutomationProperties.Name or LabeledBy")
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host "Accessibility check failed:`n" -ForegroundColor Red
    $failures | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    Write-Host "`nAdd AutomationProperties.Name, or AutomationProperties.Name=`"`" for a purely decorative element." -ForegroundColor Yellow
    exit 1
}

Write-Host "Accessibility check passed: every interactive control has an accessible name." -ForegroundColor Green
exit 0
