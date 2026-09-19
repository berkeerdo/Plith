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
     UserControl or Control root. Check 2 reads XAML AND code-behind (a Foo.xaml.cs file
     naming a local it built with `new`, or a field declared with x:Name in the matching
     Foo.xaml). That second half was added after ShelfSurface.xaml.cs's tile, overflow-tile
     and per-stack Border names, and ShelfWidget.cs's own Tiles panel and tile names, passed
     this script for as long as it read only XAML while reaching nothing at all. See the
     code-behind section below for what it can and cannot catch, and for the one file it
     already finds a pre-existing, out-of-scope failure in and reports rather than hides.

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

# The XAML the NAME checks read. src/Plith.DropCatcher is in it, and was not: Checks 1 and 2 both
# scanned $Root alone, so the catcher's two XAML files - one of which draws the shelf, a real
# user-facing surface with two real Buttons on it - were never read by either. They are clean
# today, which is luck rather than coverage: nothing here would have failed if they were not.
#
# The argument that kept them out was about Check 1 firing on the REST of Plith.DropCatcher,
# a project nobody had reviewed for accessible names. That has now been done, by running this
# check against it: the two Buttons in ShelfSurface.xaml both declare AutomationProperties.Name,
# and no other file in the project declares an interactive control at all. So the exclusion was
# buying nothing and costing the coverage this whole script exists for.
#
# src/Plith.Installer stays out, for the reason $Root's own comment gives: it has never had an
# accessibility pass, and its gaps are real rather than absent.
$nameCheckRoots = @($Root, (Join-Path $PSScriptRoot '..' 'src' 'Plith.DropCatcher')) |
                   Where-Object { Test-Path $_ } | Select-Object -Unique
# [\\/] rather than the [\/] the icon-font filter further down uses: a Windows path separator
# is a backslash, and that one matches only a forward slash. Inert on both today, since no
# generated .xaml under obj/ carries an interactive control, but this list is new and there is no
# reason to copy a wrong character into it. The old line is left alone rather than swept up here.
$nameCheckXaml = @($nameCheckRoots | ForEach-Object { Get-ChildItem -Path $_ -Filter '*.xaml' -Recurse }) |
                  Where-Object { $_.FullName -notmatch '[\\/](obj|bin)[\\/]' }

foreach ($file in $nameCheckXaml) {
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

foreach ($file in $nameCheckXaml) {
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

# --- Check 2b: the same dead-property rule, for AutomationProperties set from code-behind ---
#
# Check 2 above reads only .xaml, so a name set in a class's own .cs file was invisible to it on
# two axes at once for anything under Plith.DropCatcher: wrong extension, and wrong directory
# (this whole script defaults its root to src/Plith). That is exactly how ShelfSurface.xaml.cs's
# tile, overflow-tile and per-stack Border names, and ShelfWidget.cs's tile and Tiles-panel names,
# passed this script for as long as they did while reaching nothing at all - a review of Task 9's
# work found it after the fact. Both directories now get their own reviewed accessibility pass for
# the shelf, so both are scanned here - by adding a SEPARATE root list, not by widening $Root
# itself, which would also turn on Check 1 (every interactive control needs a name) for the rest
# of Plith.DropCatcher, a project nobody has reviewed for that yet. Same reasoning $iconFontRoots
# below already uses for the same directory.
#
# This is necessarily a heuristic, not a compiler: PowerShell regex over C# text cannot resolve a
# variable's real type the way Roslyn could. A first draft of it resolved only ONE shape (`var
# name = new Peerless { ... }`) and treated every OTHER shape as silently fine, which a review
# caught by feeding it four ordinary ways to write the same code: an explicit type with no `var`,
# a target-typed `new()`, a name resolved through a factory method, and a bare declaration
# followed by a separate assignment. All four passed silently. The fix has two halves:
#   1. Resolve more shapes (see the numbered passes inside the loop below): a bare declaration
#      later assigned, an `is Type name` pattern, a same-file factory method's return type, an
#      explicit-type declaration constructed with `new Type(...)` or target-typed `new()`, and
#      the original `var name = new Type(...)`.
#   2. NEVER let "I could not resolve this" print the same nothing as "this is fine". A target
#      this scan cannot classify at all is now its own failure category - see
#      $unresolvedProperties below - reported loudly rather than skipped with `continue`.
# A name set through an alias this scan still cannot follow, one resolved via a method declared
# in a DIFFERENT file, a collection element, or a target that is not a bare identifier at all
# (`AutomationProperties.SetName(GetElement(), ...)` is invisible to the regex that finds the
# calls in the first place, not merely to the type resolution after it) remain gaps in this
# heuristic's own coverage - the same limit the XAML-side check above already has for a name set
# on a child of a parent it did not declare itself. Anything in that remaining gap either resolves
# to a real type (and is judged) or resolves to nothing (and fails loudly as "could not
# determine"); nothing in it can silently pass any more.
$codeBehindRoots = @($Root, (Join-Path $PSScriptRoot '..' 'src' 'Plith.DropCatcher')) |
                    Where-Object { Test-Path $_ } | Select-Object -Unique

# Known, pre-existing gaps this scan finds but has not been asked to fix. Recorded as findings
# with a file to look at, not silenced by widening the Installer-style root exclusion above: that
# shape would also hide anything else this scan ever finds in the same file, forever, past the day
# this specific gap is closed. Fix the target, then delete the line here.
#
# KEYED BY THE FINDING, "<file>:<target>", NOT BY THE FILE. Keying by file was the exact shape
# this comment's own first paragraph rejects one level up, adopted one level down: every
# dead-property and unresolved-type hit in ShelfWidget.cs, MediaWidget.cs, NotchHud.cs and
# WeatherWidget.cs was downgraded to a yellow notice, so a NEW inert accessible name added to any
# of them tomorrow would pass green - including in ShelfWidget.cs, which this branch rewrote. A
# whole-branch review found that. The known gap is a named element in a named file; anything else
# in the same file is a new finding and fails.
#
# THE LIMIT OF THIS KEY, stated rather than left to be discovered: the key is a NAME, not an
# occurrence. A second inert element in the same file that happens to be called `tile` inherits
# the suppression written for the first. That is a much smaller hole than the file key it
# replaced (which covered every name in the file, whatever it was called), and closing it would
# mean keying by line number, which every unrelated edit above it would invalidate. Left as is,
# deliberately: when one of these files is fixed, delete its lines here rather than adding to
# them.
#
# The first run of the code-behind check found four files, not one. Task 9 was asked to check only
# ShelfWidget.cs; running the scan for real also caught MediaWidget.cs, NotchHud.cs and
# WeatherWidget.cs naming a Border/Grid/StackPanel the exact same way, in code that shipped well
# before this branch and is nowhere near the shelf. All six findings are filed here rather than
# fixed, for the same reason: fixing widget accessibility is not this task, and a lint that starts
# quietly rewriting product code to stay green is a worse habit than the gaps it found.
$knownCodeBehindGaps = @{
    'ShelfWidget.cs:tile' = 'predates Task 9: every Border built by Tile(...) was already named ' +
        'before this scan existed to see it. Filed, not fixed, in docs/SHELF-VERIFICATION.md ' +
        'section 5.4.'
    'ShelfWidget.cs:Tiles' = 'predates Task 9: Tiles is a StackPanel, x:Name-d in ShelfWidget.xaml. ' +
        'Filed, not fixed, in docs/SHELF-VERIFICATION.md section 5.4.'
    'MediaWidget.cs:OpenSourceArea' = 'found by this same scan, unrelated to the shelf: ' +
        'OpenSourceArea is a Border. Predates this branch. Filed, not fixed.'
    'NotchHud.cs:VolumeRow' = 'found by this same scan, unrelated to the shelf: VolumeRow is a ' +
        'Grid. Predates this branch. Filed, not fixed.'
    'NotchHud.cs:MediaRow' = 'found by this same scan, unrelated to the shelf: MediaRow is a ' +
        'Grid. Predates this branch. Filed, not fixed.'
    'WeatherWidget.cs:Readout' = 'found by this same scan, unrelated to the shelf: Readout is a ' +
        'StackPanel. Predates this branch. Filed, not fixed.'
}
$knownGapNotices = [System.Collections.Generic.List[string]]::new()
$unresolvedProperties = [System.Collections.Generic.List[string]]::new()

foreach ($file in @($codeBehindRoots | ForEach-Object { Get-ChildItem -Path $_ -Filter '*.cs' -Recurse }) |
                   Where-Object { $_.FullName -notmatch '[\\/](obj|bin)[\\/]' }) {
    $text = Get-Content -Raw -LiteralPath $file.FullName
    $rel = Resolve-Path -Relative -LiteralPath $file.FullName

    # x:Name -> declared type, read from the sibling XAML file, tried both ways this codebase
    # actually names one: Foo.xaml.cs beside Foo.xaml (ShelfSurface.xaml.cs), and Foo.cs beside
    # Foo.xaml (ShelfWidget.cs, which has no ".xaml." in its own file name at all). Trying only
    # the first shape was itself a bug in this check's first draft: it left ShelfWidget.cs's own
    # Tiles field (a StackPanel, x:Name'd in ShelfWidget.xaml) unresolved and silently unchecked,
    # over a file this same check already flags for one other reason.
    $fieldTypes = @{}
    $xamlCandidates = @(
        ($file.FullName -replace '\.cs$', '')          # Foo.xaml.cs -> Foo.xaml
        ($file.FullName -replace '\.cs$', '.xaml')      # Foo.cs      -> Foo.xaml
    ) | Select-Object -Unique
    $xamlPath = $xamlCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($xamlPath) {
        $xamlText = Get-Content -Raw -LiteralPath $xamlPath
        # The type prefix is OPTIONAL and can itself carry a XAML namespace prefix
        # (`<w:WeatherMark x:Name="Mark" .../>` in ClockWidget.xaml, a custom control in its own
        # namespace) - missed by the first draft of this pattern, which required the type name to
        # sit directly after `<` and so could never match past the `w:` in front of it. Both the
        # namespace prefix and a CLR namespace (`Foo.Bar.TypeName`) are stripped the same way:
        # keep only the last `:` - or `.` - separated segment.
        foreach ($m in [regex]::Matches($xamlText, '<(?<type>(?:[A-Za-z_][\w]*:)?[A-Za-z_][\w.]*)\s[^>]*?x:Name="(?<name>\w+)"')) {
            $fieldTypes[$m.Groups['name'].Value] = ($m.Groups['type'].Value -split '[:.]')[-1]
        }
    }

    # Every name -> type this file lets us pin down, built from several shapes at once so the
    # SAME variable resolves the same way no matter which of them wrote it. Lowest-confidence
    # first, each later pass overwriting rather than skipping, so a stronger signal for the same
    # name always wins over a weaker one seen earlier in the file.
    #
    # A re-review of this check's first draft fed it four ordinary ways to write the same code
    # and every one slipped past SILENTLY, as a pass: an explicit type with no `var`, a
    # target-typed `new()`, a name resolved through a factory method, and declare-then-assign in
    # two statements. The cause was the SAME everywhere: an unresolved type `continue`d rather
    # than being reported, so "I could not tell" and "this is fine" produced identical output.
    # That is the exact failure this whole task exists because of, reproduced one layer in. Fixed
    # two ways at once: resolve more shapes (below), and never again let "unresolved" mean
    # nothing (see the loop after this one, and its "could not determine" branch).
    $typeOf = @{}

    # 1 (weakest). A bare declaration with no initializer at all: `Border tile;`. Only a
    # declaration can legally look like "identifier identifier;" in C#, so this is syntactically
    # safe to assume even though it is a plain regex rather than a parser. Covers "declare in one
    # statement, assign in the next" together with pass 3 or 4 below, whichever resolves the
    # assignment.
    foreach ($m in [regex]::Matches($text, '(?<![.\w])(?!var\b|new\b|return\b)(?<type>[A-Za-z_]\w*)\s+(?<name>\w+)\s*;')) {
        $name = $m.Groups['name'].Value
        if (-not $typeOf.ContainsKey($name)) { $typeOf[$name] = $m.Groups['type'].Value }
    }

    # 2. `x is Type name` / `x is Type name when ...` - a real shape in this codebase today
    # (SettingsWindow.xaml.cs's swatch loop), not one of the four the review fed this check, but
    # the same class of gap: a binding this scan did not previously look for at all.
    foreach ($m in [regex]::Matches($text, '\bis\s+(?<type>[A-Za-z_][\w.]*)\s+(?<name>\w+)\b')) {
        $typeOf[$m.Groups['name'].Value] = ($m.Groups['type'].Value -split '\.')[-1]
    }

    # 3. A name resolved through a factory or helper method declared in the SAME file: `var tile
    # = BuildTile(...)`, where `private NamedBorder BuildTile(...)` says what it returns. Method
    # return types are collected once per file and matched by name; a method declared elsewhere
    # (another partial, a base class, an extension method) is outside what a single file's text
    # can answer and falls through to "could not determine" rather than being guessed at.
    $methodReturns = @{}
    foreach ($m in [regex]::Matches($text,
        '\b(?:public|private|internal|protected)(?:\s+(?:static|sealed|override|virtual|async))*\s+(?<ret>[A-Za-z_][\w<>\[\],\s]*?)\s+(?<name>[A-Za-z_]\w*)\s*\(')) {
        $ret = ($m.Groups['ret'].Value.Trim() -split '\s+')[-1]
        $methodReturns[$m.Groups['name'].Value] = ($ret -split '\.')[-1]
    }
    foreach ($m in [regex]::Matches($text, '\bvar\s+(?<name>\w+)\s*=\s*(?<method>[A-Za-z_]\w*)\s*\(')) {
        $method = $m.Groups['method'].Value
        if ($methodReturns.ContainsKey($method)) { $typeOf[$m.Groups['name'].Value] = $methodReturns[$method] }
    }

    # 4. An explicit type on the left, constructed either as `Type name = new Type(...)` /
    # `new Type { ... }`, or as a target-typed `Type name = new();`. The declared (left-hand)
    # type is what is kept, deliberately, even on the rare line where it differs from whatever
    # the constructor call names: WPF creates a peer for the RUNTIME type, a plain regex cannot
    # see past a declared type to whatever a factory really handed back, and treating the
    # declared type as authoritative fails safe - it can only flag a real Border kept behind a
    # wider declared type as "worth a second look", never wave one through unseen.
    foreach ($m in [regex]::Matches($text,
        '(?<![.\w])(?!var\b)(?<type>[A-Za-z_][\w.]*)\s+(?<name>\w+)\s*=\s*new\s*(?:\(\)|<[^>]*>\s*\(\)|[A-Za-z_][\w<>]*\s*[({])')) {
        $typeOf[$m.Groups['name'].Value] = ($m.Groups['type'].Value -split '\.')[-1]
    }

    # 5 (strongest). `var name = new Type(...)` / `new Type { ... }` - the shape this check was
    # first written against, and still the most common one in this codebase.
    foreach ($m in [regex]::Matches($text, '\bvar\s+(?<name>\w+)\s*=\s*new\s+(?<type>[A-Za-z_][\w.<>]*)\b')) {
        $typeOf[$m.Groups['name'].Value] = ($m.Groups['type'].Value -split '\.')[-1]
    }

    # The call finder matches the call, then reads its first argument BY HAND.
    #
    # It used to capture the target with `(?<target>\w+)` inside the same regex, which meant the
    # pattern matched nothing at all for any target that is not a bare identifier:
    # `SetName(BuildTile(), ...)`, `SetName(tiles[i], ...)` and `SetName(this.Foo, ...)` produced
    # no match, so they were not a third answer beside "fine" and "could not resolve" - they were
    # NO ANSWER, invisible to a check whose whole purpose is that "could not tell" must never
    # print the same nothing as "this is fine". A whole-branch review found it. Scanning the
    # argument by hand is what makes the call itself impossible to miss; whether its type can then
    # be resolved is the separate question the passes above answer, and an argument shape none of
    # them can read now lands in $unresolvedProperties and fails, as it should.
    foreach ($m in [regex]::Matches($text, 'AutomationProperties\.Set(?<prop>\w+)\s*\(')) {
        $prop = $m.Groups['prop'].Value

        # Read to the comma that ends the first argument, tracking bracket depth so a comma inside
        # a nested call or an index (`SetName(Tile(a, b), ...)`) does not end it early.
        $i = $m.Index + $m.Length
        $depth = 0
        $end = -1
        while ($i -lt $text.Length) {
            $c = $text[$i]
            if ($c -eq '(' -or $c -eq '[') { $depth++ }
            elseif ($c -eq ']') { $depth-- }
            elseif ($c -eq ')') {
                if ($depth -eq 0) { break }   # a one-argument call: no target/value pair to read
                $depth--
            }
            elseif ($c -eq ',' -and $depth -eq 0) { $end = $i; break }
            $i++
        }
        if ($end -lt 0) {
            $unresolvedProperties.Add("${rel}: AutomationProperties.Set$prop(...) - this scan could not read the call's first argument at all")
            continue
        }
        $target = $text.Substring($m.Index + $m.Length, $end - ($m.Index + $m.Length)).Trim()

        # `this` is not looked up anywhere above: it names the class this whole file defines, not
        # a local or a field, and every class in this codebase that can reach an
        # AutomationProperties call is a UserControl, a Window, or another Control-derived root.
        # UserControl was checked by hand, not assumed: UIElementAutomationPeer.CreatePeerForElement
        # returns a real peer for a plain UserControl instance. Window and other Control-derived
        # roots carry a peer by the same WPF mechanism (WindowAutomationPeer and so on) and are
        # not separately re-verified here.
        if ($target -eq 'this') { continue }

        # `this.Foo` is the same field `Foo` names, written the long way. Unwrapped rather than
        # reported, since the answer is identical and pretending otherwise would be a gap that
        # exists only because of a style choice.
        if ($target -match '^this\.(?<field>\w+)$') { $target = $Matches['field'] }

        $type = $null
        if ($target -match '^\w+$') {
            if ($typeOf.ContainsKey($target)) { $type = $typeOf[$target] }
            elseif ($fieldTypes.ContainsKey($target)) { $type = $fieldTypes[$target] }
        }

        if ($null -eq $type) {
            # UNRESOLVED IS NOT A PASS. The first draft of this check `continue`d here, which made
            # "I could not tell" print nothing at all - the identical output to a genuine pass,
            # and the review that found this called that worse than no check. This script is a
            # release gate, and the rest of it already treats an unreadable file as a failure
            # (see the XAML parse-failure branch above), not a silent skip, so the same rule
            # applies here: a name this scan cannot classify fails the build, loudly, distinct
            # from a confirmed dead property so the two are never mistaken for each other.
            $gapKey = "$($file.Name):$target"
            if ($knownCodeBehindGaps.ContainsKey($gapKey)) {
                $knownGapNotices.Add("${rel}: AutomationProperties.Set$prop($target, ...) - type could not be determined ($($knownCodeBehindGaps[$gapKey]))")
                continue
            }
            $unresolvedProperties.Add("${rel}: AutomationProperties.Set$prop($target, ...) - this scan could not determine $target's type, so it cannot say whether WPF gives it an automation peer")
            continue
        }

        if ($peerless -notcontains $type) { continue }

        $gapKey = "$($file.Name):$target"
        if ($knownCodeBehindGaps.ContainsKey($gapKey)) {
            $knownGapNotices.Add("${rel}: AutomationProperties.Set$prop($target, ...) targets a $type ($($knownCodeBehindGaps[$gapKey]))")
            continue
        }

        $deadProperties.Add("${rel}: AutomationProperties.Set$prop($target, ...) targets a $type, which WPF gives no automation peer")
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
#
# The catcher is scanned here too, added for the same reason as the installer: this rule is
# about a font varying between Windows builds, not about the accessibility NAME pass's scope,
# and Plith.DropCatcher renders a real user-facing surface (the shelf) that every root above
# left untouched. Task 5 added a shell-icon Image beside the shelf's drawn geometry without this
# line ever reading either file it touched. Added to the glyph roots ONLY, not to $Root itself:
# widening $Root would also turn on the accessibility NAME checks for a project nobody has
# reviewed for that yet, which is a separate gap this line is not the one to close.
$iconFontUses = [System.Collections.Generic.List[string]]::new()
$iconFontRoots = @($Root, (Join-Path $PSScriptRoot '..' 'src' 'Plith.Installer'),
                    (Join-Path $PSScriptRoot '..' 'src' 'Plith.DropCatcher')) |
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

if ($failures.Count -gt 0 -or $deadProperties.Count -gt 0 -or $unresolvedProperties.Count -gt 0 -or $iconFontUses.Count -gt 0) {
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
    if ($unresolvedProperties.Count -gt 0) {
        # A DIFFERENT failure from the one above, on purpose: this is not a confirmed dead
        # property, it is code-behind's own version of the XAML-parse failure earlier in this
        # script - a gap in what this scan could read, treated as a failure rather than a pass it
        # has no grounds to report. See this section's own header comment for why "could not
        # tell" must never print the same nothing as "this is fine" again.
        Write-Host "`n  AutomationProperties.SetName targets this scan could NOT resolve a type for:" -ForegroundColor Red
        $unresolvedProperties | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
        Write-Host "`n  Either the target's type is genuinely undecidable from this file alone (say so with a" -ForegroundColor Yellow
        Write-Host "  narrower AutomationProperties call this scan CAN read), or this scan's heuristic needs a" -ForegroundColor Yellow
        Write-Host "  new pattern for a real shape it has not seen yet." -ForegroundColor Yellow
    }
    if ($iconFontUses.Count -gt 0) {
        Write-Host "`n  System icon fonts, whose glyphs differ between Windows builds:" -ForegroundColor Red
        $iconFontUses | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
    }
    exit 1
}

Write-Host "Accessibility check passed: every interactive control has an accessible name, every AutomationProperties value sits on an element that can surface it, and no view depends on a system icon font." -ForegroundColor Green
foreach ($notice in $knownGapNotices | Sort-Object -Unique) { Write-Host "  KNOWN GAP, not fixed here: $notice" -ForegroundColor Yellow }
exit 0
