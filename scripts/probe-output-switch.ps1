#requires -Version 7
<#
.SYNOPSIS
  Proves OutputDeviceSwitcher's call path without changing anyone's audio.

.DESCRIPTION
  IPolicyConfig is undocumented, so "it compiles" says nothing at all: a COM interface is a
  vtable, and a declaration with the wrong number of methods ahead of the one you want compiles
  perfectly and then calls something else. This asks the machine instead.

  It re-selects the endpoint that is ALREADY default, so a success is a real S_OK from the real
  interface and nothing anyone is listening to moves. That is the same trick docs/ROADMAP.md
  records from the System Controls measurement, and it is the reason this can be run at any time.

  Run with: pwsh -File scripts/probe-output-switch.ps1
#>

[CmdletBinding()]
param([string]$Configuration = 'Debug')

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root "src\Plith\bin\$Configuration\net10.0-windows10.0.22000.0"
if (-not (Test-Path (Join-Path $bin 'Plith.dll'))) {
    throw "Build $Configuration first: no Plith.dll in $bin"
}

[AppDomain]::CurrentDomain.add_AssemblyResolve({
    param($s, $e)
    $n = ($e.Name -split ',')[0]
    $c = Join-Path $bin "$n.dll"
    if (Test-Path $c) { return [Reflection.Assembly]::LoadFrom($c) }
    $null
})
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'Plith.dll'))
$null = [Reflection.Assembly]::LoadFrom((Join-Path $bin 'NAudio.Wasapi.dll'))

$en = New-Object NAudio.CoreAudioApi.MMDeviceEnumerator
$before = $en.GetDefaultAudioEndpoint('Render', 'Multimedia')
"default before : $($before.FriendlyName)"
"id             : $($before.ID)"

$ok = [Plith.Services.OutputDeviceSwitcher]::TrySetDefault($before.ID, $null)
"TrySetDefault  : $ok"

$after = $en.GetDefaultAudioEndpoint('Render', 'Multimedia')
"default after  : $($after.FriendlyName)"

if (-not $ok) {
    throw ("TrySetDefault returned false: the interop does not work on this system. The two " +
           "likely causes are the PolicyConfig class not being registered (a stripped SKU) and " +
           "the interface GUID having changed. Both are reportable facts; do not go looking for " +
           "another GUID to try.")
}
if ($after.ID -ne $before.ID) {
    throw "The default output MOVED. It was asked to stay exactly where it was."
}

Write-Host 'PASS: the call path works and nothing moved.' -ForegroundColor Green
