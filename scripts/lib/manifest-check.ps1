# scripts/lib/manifest-check.ps1: reads the UAC request from a built exe's embedded manifest.
# Dot-source it. Reads only; never changes a file.

# Returns the value of the uiAccess attribute of the requestedExecutionLevel element in the
# manifest embedded in $Path ('true', 'false'), or $null when the exe has no such element. The
# manifest is plain UTF-8 XML in the resource section, so the bytes are searched directly.
function Get-PlithUiAccess {
    param([string]$Path)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    $element = [regex]::Matches($text, '<requestedExecutionLevel\b[^>]*>')
    if ($element.Count -ne 1) { return $null }
    $attribute = [regex]::Match($element[0].Value, 'uiAccess\s*=\s*["'']([^"'']*)["'']')
    if (-not $attribute.Success) { return $null }
    return $attribute.Groups[1].Value
}

# Throws unless the exe at $Path embeds a manifest that requests uiAccess="true".
function Assert-PlithUiAccess {
    param([string]$Path)
    $value = Get-PlithUiAccess -Path $Path
    if ($value -ne 'true') {
        $shown = if ($null -eq $value) { 'no requestedExecutionLevel' } else { "uiAccess=$value" }
        throw "$Path does not request UIAccess ($shown); was PLITH_OSD_UIACCESS=1 set for the build?"
    }
    Write-Host "uiAccess=true  $Path"
}
