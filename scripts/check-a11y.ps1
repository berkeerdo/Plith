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

# ScrollViewer is in this list because WPF makes it keyboard-focusable by default, which
# makes it a genuine tab stop rather than passive chrome. Settings' ScrollViewer was reached
# fourth by Tab and announced as nothing but "pane". A ScrollViewer that is not meant to be a
# tab stop declares Focusable="False" and is exempted below like any other template part.
$interactive = @('Button', 'ComboBox', 'Slider', 'ToggleButton', 'CheckBox', 'TextBox', 'RadioButton', 'ScrollViewer')
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

# --- System icon fonts under Views ---
#
# A FontIcon bound to "Segoe MDL2 Assets" or "Segoe Fluent Icons" depends on a font whose
# contents differ between Windows builds. Slice 2 found that out the expensive way: seven of ten
# weather glyphs picked from Segoe MDL2 turned out not to exist in it and would have rendered as
# tofu boxes on a user's machine. Every icon this product draws now lives in
# Resources/PlithIcons.xaml as geometry, which cannot go missing.
#
# This rule was owed from the widget slice and could not pass until the classic card's icons were
# drawn. It is deliberately not narrowed to the directories that happened to be clean at the time
# — a rule narrowed to fit the code is a gate that has stopped meaning anything.
# Both XAML and code-behind. Scanning only XAML was the rule's own blind spot on the day it was
# written: SettingsWindow built three marks in C# with new FontFamily("Segoe MDL2 Assets"), and a
# gate that misses the half of the codebase where a thing is easiest to do is not a gate.
# Two forms, because the name is only half of it.
#
# A glyph can be used WITHOUT ever naming the font: Plith.Installer's caption buttons carried
# Content="&#xE921;" and Content="&#xE8BB;" and no font name anywhere, inheriting one from a
# shared style. When that style stopped supplying Segoe MDL2, the characters stayed and the
# buttons rendered as nothing - present, clickable, invisible. Shipped in 0.1.6. A rule that
# searches only for font names would have scanned that file and passed it.
#
# So code points are checked too. U+E000-U+F8FF is the Private Use Area: no standard character
# lives there, and a literal one in markup is an icon-font glyph by definition.
#
# And the scan covers the installer, which it did not. The exclusion below is right for the
# accessibility NAME checks - that pass never covered the installer and would fail on gaps nobody
# has signed off on - but this rule is about not depending on a font that varies between Windows
# builds, and the installer proved it needs the rule as much as anything else.
$iconFontUses = [System.Collections.Generic.List[string]]::new()
$iconFontRoots = @($Root, (Join-Path $PSScriptRoot '..' 'src' 'Plith.Installer')) |
                 Where-Object { Test-Path $_ }
$iconFontFiles = @($iconFontRoots | ForEach-Object {
                     @(Get-ChildItem -Path $_ -Filter '*.xaml' -Recurse) +
                     @(Get-ChildItem -Path $_ -Filter '*.cs' -Recurse)
                 }) | Where-Object { $_.FullName -notmatch '[\/](obj|bin)[\/]' }
foreach ($file in $iconFontFiles) {
    $text = Get-Content -LiteralPath $file.FullName -Raw
    # Strip comments in both languages, so a comment explaining why the font is NOT used does
    # not itself trip the rule.
    $stripped = [regex]::Replace($text, '(?s)<!--.*?-->', '')
    $stripped = [regex]::Replace($stripped, '(?s)/\*.*?\*/', '')
    $stripped = [regex]::Replace($stripped, '(?m)^\s*//.*$', '')
    foreach ($font in @('Segoe MDL2 Assets', 'Segoe Fluent Icons')) {
        if ($stripped -like "*$font*") {
            $rel = Resolve-Path -Relative -LiteralPath $file.FullName
            $iconFontUses.Add("${rel}: uses '$font'. Draw the icon in Resources/PlithIcons.xaml instead.")
        }
    }

    # A Private Use Area code point, with or without a font named beside it.
    $pua = [regex]::Matches($stripped, '&#x(?<cp>[eE][0-9a-fA-F]{3}|[fF][0-8][0-9a-fA-F]{2});')
    if ($pua.Count -gt 0) {
        $rel = Resolve-Path -Relative -LiteralPath $file.FullName
        $points = ($pua | ForEach-Object { 'U+' + $_.Groups['cp'].Value.ToUpper() } | Select-Object -Unique) -join ', '
        $iconFontUses.Add("${rel}: icon-font code point(s) $points. Draw the icon in Resources/PlithIcons.xaml instead.")
    }
}

if ($failures.Count -gt 0 -or $deadProperties.Count -gt 0 -or $iconFontUses.Count -gt 0) {
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
    if ($iconFontUses.Count -gt 0) {
        Write-Host "`n  System icon fonts, whose glyphs differ between Windows builds:" -ForegroundColor Red
        $iconFontUses | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
    }
    exit 1
}

Write-Host "Accessibility check passed: every interactive control has an accessible name, every AutomationProperties value sits on an element that can surface it, and no view depends on a system icon font." -ForegroundColor Green
exit 0
