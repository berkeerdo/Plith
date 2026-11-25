# scripts/build-icon.ps1: builds crates/plith/assets/plith.ico from the icon master PNG.
# The master is the approved 1024 px drawing with a transparent background
# (docs/design/identity/icon/plith-master-1024.png). Each size is cropped to the tile, drawn
# with high-quality bicubic sampling and stored as a PNG entry, so the .ico keeps full alpha.

[CmdletBinding()]
param(
    [string]$Master = (Join-Path $PSScriptRoot '..\docs\design\identity\icon\plith-master-1024.png'),
    [string]$Out = (Join-Path $PSScriptRoot '..\crates\plith\assets\plith.ico'),
    [int[]]$Sizes = @(16, 20, 24, 32, 40, 48, 64, 256),
    # The 256 px entry also lands here for the settings window's About icon.
    [string]$Tile = (Join-Path $PSScriptRoot '..\crates\plith\assets\plith-tile-256.png')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$src = [System.Drawing.Bitmap]::new((Resolve-Path $Master).Path)
try {
    # 1. The tile's bounds: every pixel with some alpha.
    $minX = $src.Width; $minY = $src.Height; $maxX = -1; $maxY = -1
    $data = $src.LockBits(
        [System.Drawing.Rectangle]::new(0, 0, $src.Width, $src.Height),
        [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
        [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        $bytes = [byte[]]::new($data.Stride * $src.Height)
        [System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $bytes, 0, $bytes.Length)
    } finally {
        $src.UnlockBits($data)
    }
    for ($y = 0; $y -lt $src.Height; $y++) {
        $row = $y * $data.Stride
        for ($x = 0; $x -lt $src.Width; $x++) {
            if ($bytes[$row + $x * 4 + 3] -gt 24) {
                if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
                if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
            }
        }
    }
    if ($maxX -lt 0) { throw "The master has no opaque pixels: $Master" }
    $side = [Math]::Max($maxX - $minX + 1, $maxY - $minY + 1)
    $cx = ($minX + $maxX + 1) / 2; $cy = ($minY + $maxY + 1) / 2
    $crop = [System.Drawing.RectangleF]::new($cx - $side / 2, $cy - $side / 2, $side, $side)

    # 2. One PNG per size. Large sizes keep a margin of 1/32 so the tile does not touch the
    #    icon's edge; 24 px and below fill the square, where every pixel counts.
    $entries = foreach ($size in $Sizes) {
        $margin = if ($size -gt 24) { [Math]::Round($size / 32.0) } else { 0 }
        $bmp = [System.Drawing.Bitmap]::new($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        try {
            $g.Clear([System.Drawing.Color]::Transparent)
            $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
            $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
            $dest = [System.Drawing.RectangleF]::new($margin, $margin, $size - 2 * $margin, $size - 2 * $margin)
            $g.DrawImage($src, $dest, $crop, [System.Drawing.GraphicsUnit]::Pixel)
        } finally {
            $g.Dispose()
        }
        $ms = [System.IO.MemoryStream]::new()
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $bmp.Dispose()
        [pscustomobject]@{ Size = $size; Png = $ms.ToArray() }
    }
} finally {
    $src.Dispose()
}

# 3. The .ico container: header, one directory entry per image, then the PNG bytes.
$ico = [System.IO.MemoryStream]::new()
$w = [System.IO.BinaryWriter]::new($ico)
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$entries.Count)
$offset = 6 + 16 * $entries.Count
foreach ($e in $entries) {
    $dim = if ($e.Size -ge 256) { 0 } else { $e.Size }
    $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$e.Png.Length); $w.Write([uint32]$offset)
    $offset += $e.Png.Length
}
foreach ($e in $entries) { $w.Write($e.Png) }
$w.Flush()
[System.IO.File]::WriteAllBytes((Join-Path (Resolve-Path (Split-Path $Out)).Path (Split-Path $Out -Leaf)), $ico.ToArray())
$w.Dispose()
Write-Host "wrote $Out ($($entries.Count) sizes: $($Sizes -join ', '))"

$big = $entries | Where-Object { $_.Size -eq 256 } | Select-Object -First 1
if ($null -eq $big) { throw 'No 256 px entry for the tile PNG.' }
[System.IO.File]::WriteAllBytes((Join-Path (Resolve-Path (Split-Path $Tile)).Path (Split-Path $Tile -Leaf)), $big.Png)
Write-Host "wrote $Tile (256 px)"
