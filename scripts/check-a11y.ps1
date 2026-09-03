#requires -Version 7
<#
.SYNOPSIS
  Fails when an interactive XAML control carries no accessible name, or when an accessible
  name is set on an element that cannot surface it.

.DESCRIPTION
  AutomationProperties live in XAML, where unit tests are weak — asserting on them needs an
  STA thread and a loaded visual tree. This static check is the regression guard instead.

  Two checks run:

  1. Every interactive control declares an accessible name (details below).

  2. No AutomationProperties are set on an element WPF gives no automation peer to. This
     second check exists because check 1 alone was green while the OSD's live region was
     completely inert: both card views set AutomationProperties.Name and LiveSetting on a
     bare <Grid>. WPF only creates automation peers for types that override
     OnCreateAutomationPeer — panels, borders and other layout/decoration elements do not —
     and UIElementAutomationPeer.GetNameCore reads the property off its owner. A name set on
     a peerless element therefore reaches nothing at all: it appears nowhere in the live UI
     Automation tree, not even in the raw view, while every build, test and lint stays green.
     Move such properties onto the nearest element that does own a peer, usually the
     UserControl or Control root.

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

# --- Check 2: AutomationProperties must sit on an element that owns an automation peer ---

# Types WPF creates no automation peer for. Not exhaustive by design: it lists the
# layout and decoration elements an accessible name plausibly gets attached to by
# mistake, which is where this class of bug actually occurs.
$peerless = @(
    'Grid', 'StackPanel', 'DockPanel', 'WrapPanel', 'Canvas', 'UniformGrid',
    'VirtualizingStackPanel', 'Border', 'Decorator', 'Viewbox', 'ContentPresenter',
    'ItemsPresenter', 'AdornerDecorator', 'BulletDecorator', 'InkPresenter',
    'Rectangle', 'Ellipse', 'Path', 'Line', 'Polygon', 'Polyline'
)

$deadProperties = [System.Collections.Generic.List[string]]::new()

foreach ($file in Get-ChildItem -Path $Root -Filter '*.xaml' -Recurse) {
    try {
        $xml = [xml](Get-Content -Raw -LiteralPath $file.FullName)
    }
    catch {
        # A XAML file this script cannot parse is a gap in coverage, not a pass.
        $rel = Resolve-Path -Relative -LiteralPath $file.FullName
        $deadProperties.Add("${rel}: could not be parsed as XML, so it was not checked — $($_.Exception.Message)")
        continue
    }

    foreach ($node in $xml.SelectNodes('//*')) {
        if ($peerless -notcontains $node.LocalName) { continue }
        if ($null -eq $node.Attributes) { continue }
        foreach ($attr in $node.Attributes) {
            if ($attr.Name -notlike 'AutomationProperties.*') { continue }
            $rel = Resolve-Path -Relative -LiteralPath $file.FullName
            $deadProperties.Add("${rel}: <$($node.LocalName)> sets $($attr.Name), but WPF gives $($node.LocalName) no automation peer")
        }
    }
}

if ($failures.Count -gt 0 -or $deadProperties.Count -gt 0) {
    Write-Host "Accessibility check failed:`n" -ForegroundColor Red
    if ($failures.Count -gt 0) {
        Write-Host "  Interactive controls without an accessible name:" -ForegroundColor Red
        $failures | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
        Write-Host "`n  Add AutomationProperties.Name, or AutomationProperties.Name=`"`" for a purely decorative element." -ForegroundColor Yellow
    }
    if ($deadProperties.Count -gt 0) {
        Write-Host "`n  AutomationProperties that never reach UI Automation:" -ForegroundColor Red
        $deadProperties | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
        Write-Host "`n  Move them onto the nearest element that owns a peer, usually the UserControl or Control root." -ForegroundColor Yellow
    }
    exit 1
}

Write-Host "Accessibility check passed: every interactive control has an accessible name, and every AutomationProperties value sits on an element that can surface it." -ForegroundColor Green
exit 0
