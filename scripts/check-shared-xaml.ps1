# scripts/check-shared-xaml.ps1 — fails if a XAML file shared with the installer reaches into
# the Plith assembly by name.
#
# Why this exists: 0.1.6 shipped an installer that died before its first log line. SettingsTheme
# .xaml is compiled into BOTH assemblies — Plith.Installer links it as a Page — and a commit gave
# it an absolute source:
#
#     pack://application:,,,/Plith;component/Resources/PlithIcons.xaml
#
# Absolute means "the Plith assembly", wherever it runs. The installer carries Plith as an
# embedded zip, not as a reference, so the lookup threw FileNotFoundException inside
# App.InitializeComponent and the process was gone before anything could report it.
#
# Nothing caught it, and nothing was going to: a pack URI is resolved at RUN time, so the build is
# clean; the installer's tests do not launch its UI; and the whole branch went by without anyone
# running the installer. It took a second machine and a user to find.
#
# This check reads the installer's csproj for the files it links, and fails on any absolute
# same-solution pack URI inside them. A relative source resolves against whichever assembly hosts
# the dictionary, which is what a shared file needs.
#
# Run with: pwsh -File scripts/check-shared-xaml.ps1

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$installerProj = Join-Path $root 'src\Plith.Installer\Plith.Installer.csproj'

if (-not (Test-Path $installerProj)) { throw "Installer project not found at $installerProj" }

$projXml = Get-Content $installerProj -Raw

# Every <Page Include="..\Plith\..."> and <Resource Include="..\Plith\...">: the files that end up
# compiled into the installer as well as into Plith.
$linked = [regex]::Matches($projXml, '<(?:Page|Resource)\s+Include="([^"]*\\Plith\\[^"]+)"') |
    ForEach-Object { $_.Groups[1].Value }

if ($linked.Count -eq 0) {
    Write-Host "No shared XAML linked into the installer; nothing to check."
    exit 0
}

$problems = @()
$checked = 0

# Follow one level of merge: a linked file is compiled into the installer, and so is anything it
# pulls in relatively. Both have to be free of absolute self-references.
$queue = [System.Collections.Generic.Queue[string]]::new()
$seen = @{}
foreach ($rel in $linked) { $queue.Enqueue((Join-Path $root ($rel -replace '^\.\.\\', 'src\'))) }

while ($queue.Count -gt 0) {
    $path = $queue.Dequeue()
    if ($seen.ContainsKey($path)) { continue }
    $seen[$path] = $true

    if (-not (Test-Path $path)) {
        $problems += "  $path — linked by the installer but missing on disk"
        continue
    }
    if ($path -notmatch '\.xaml$') { continue }

    $checked++
    $lines = Get-Content $path
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match 'pack://application:,,,/Plith;component/') {
            $problems += "  $((Resolve-Path $path -Relative))($($i + 1)): absolute pack URI names the Plith assembly"
        }

        # Relative merges are fine, but they must also be linked, or the reference dangles at run
        # time exactly the way the absolute one did.
        if ($lines[$i] -match 'ResourceDictionary\s+Source="(?!pack://)([^"]+\.xaml)"') {
            $sibling = Join-Path (Split-Path $path -Parent) $Matches[1]
            $queue.Enqueue($sibling)

            $leaf = Split-Path $sibling -Leaf
            if ($projXml -notmatch [regex]::Escape($leaf)) {
                $problems += "  $((Resolve-Path $path -Relative)): merges '$leaf', which the installer does not link"
            }
        }
    }
}

if ($problems.Count -gt 0) {
    Write-Host ""
    Write-Host "Shared XAML check failed:" -ForegroundColor Red
    Write-Host ""
    $problems | ForEach-Object { Write-Host $_ }
    Write-Host ""
    Write-Host "A file compiled into both assemblies must not name one of them. Use a source"
    Write-Host "relative to the dictionary, and link every file it merges into the installer too."
    exit 1
}

Write-Host "Shared XAML check passed: $checked file(s) compiled into both Plith and the installer, none naming an assembly."
exit 0
