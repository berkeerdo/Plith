# Usage: pwsh -File make-states.ps1 -In <folder holding inst/p2.png, inst/flat.png, inst/button.png> -Out <new folder>
# Builds the four installer backgrounds (0 to 3 pressed ripples) from the all-pressed pass (p2)
# and the flat pass (flat), and writes them at 1x (640x420) and 2x (1280x840), plus the button.
# Ripple k's mask: pixels where flat is clearly brighter than p2 (the groove shadow), assigned to
# a ring by their distance from (960, 100), grown and feathered by a box blur.
param([string]$In, [string]$Out, [float[]]$Bounds = @(160, 310, 450))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing, System.Drawing.Primitives, System.Drawing.Common -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Ripples {
    public static byte[] Read(Bitmap b) {
        var d = b.LockBits(new Rectangle(0, 0, b.Width, b.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        var px = new byte[d.Stride * b.Height];
        Marshal.Copy(d.Scan0, px, 0, px.Length);
        b.UnlockBits(d);
        return px;
    }

    public static Bitmap Write(byte[] px, int w, int h) {
        var b = new Bitmap(w, h, PixelFormat.Format32bppArgb);
        var d = b.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
        Marshal.Copy(px, 0, d.Scan0, px.Length);
        b.UnlockBits(d);
        return b;
    }

    static float Lum(byte[] p, int i) { return 0.0722f * p[i] + 0.7152f * p[i + 1] + 0.2126f * p[i + 2]; }

    // Box blur of a float field with radius r, separable, edges clamped.
    static float[] Blur(float[] f, int w, int h, int r) {
        var t = new float[f.Length];
        var o = new float[f.Length];
        for (int y = 0; y < h; y++) {
            float s = 0; for (int x = -r; x <= r; x++) s += f[y * w + Math.Min(w - 1, Math.Max(0, x))];
            for (int x = 0; x < w; x++) {
                t[y * w + x] = s / (2 * r + 1);
                s += f[y * w + Math.Min(w - 1, x + r + 1)] - f[y * w + Math.Max(0, x - r)];
            }
        }
        for (int x = 0; x < w; x++) {
            float s = 0; for (int y = -r; y <= r; y++) s += t[Math.Min(h - 1, Math.Max(0, y)) * w + x];
            for (int y = 0; y < h; y++) {
                o[y * w + x] = s / (2 * r + 1);
                s += t[Math.Min(h - 1, y + r + 1) * w + x] - t[Math.Max(0, y - r) * w + x];
            }
        }
        return o;
    }

    // Returns three masks (0..1), one per ring, inner first.
    public static float[][] Masks(byte[] pressed, byte[] flat, int w, int h, float[] bounds) {
        var rings = new float[3][];
        for (int k = 0; k < 3; k++) rings[k] = new float[w * h];
        for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
            int i = (y * w + x) * 4;
            if (Lum(flat, i) - Lum(pressed, i) < 40f) continue;
            double r = Math.Sqrt((x - 960.0) * (x - 960.0) + (y - 100.0) * (y - 100.0));
            if (r < bounds[0]) continue; // the source's own rim
            int k = r < bounds[1] ? 0 : r < bounds[2] ? 1 : 2;
            rings[k][y * w + x] = 1f;
        }
        for (int k = 0; k < 3; k++) {
            var grown = Blur(rings[k], w, h, 30);
            for (int j = 0; j < grown.Length; j++) grown[j] = Math.Min(1f, grown[j] * 6f);
            rings[k] = Blur(grown, w, h, 14);
        }
        return rings;
    }

    public static byte[] Blend(byte[] flat, byte[] pressed, float[][] masks, int upto) {
        var o = new byte[flat.Length];
        for (int j = 0; j < flat.Length / 4; j++) {
            float m = 0; for (int k = 0; k < upto; k++) m = Math.Max(m, masks[k][j]);
            for (int c = 0; c < 3; c++) o[j * 4 + c] = (byte)Math.Round(flat[j * 4 + c] * (1 - m) + pressed[j * 4 + c] * m);
            o[j * 4 + 3] = 255;
        }
        return o;
    }
}
'@

$dir = $In
New-Item -ItemType Directory -Path $Out | Out-Null
$p = [System.Drawing.Bitmap]::new("$dir\inst\p2.png")
$f = [System.Drawing.Bitmap]::new("$dir\inst\flat.png")
$w = $p.Width; $h = $p.Height
$pp = [Ripples]::Read($p); $fp = [Ripples]::Read($f)
$masks = [Ripples]::Masks($pp, $fp, $w, $h, $Bounds)

function Save-Scaled($bmp, [int]$sw, [int]$sh, [string]$path) {
    $o = [System.Drawing.Bitmap]::new($sw, $sh, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($o)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.DrawImage($bmp, 0, 0, $sw, $sh)
    $g.Dispose()
    $o.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $o.Dispose()
}

for ($k = 0; $k -le 3; $k++) {
    $px = [Ripples]::Blend($fp, $pp, $masks, $k)
    $b = [Ripples]::Write($px, $w, $h)
    Save-Scaled $b 1280 840 "$Out\bg-$k-2x.png"
    Save-Scaled $b 640 420 "$Out\bg-$k-1x.png"
    $b.Dispose()
}

# Button: crop to its alpha bounds, then 2x at 400 px wide and 1x at 200.
$btn = [System.Drawing.Bitmap]::new("$dir\inst\button.png")
$bp = [Ripples]::Read($btn)
$minX = $btn.Width; $minY = $btn.Height; $maxX = 0; $maxY = 0
for ($y = 0; $y -lt $btn.Height; $y++) { for ($x = 0; $x -lt $btn.Width; $x++) {
    if ($bp[($y * $btn.Width + $x) * 4 + 3] -gt 8) {
        if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
        if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y } } } }
$crop = $btn.Clone([System.Drawing.Rectangle]::new($minX, $minY, $maxX - $minX + 1, $maxY - $minY + 1), $btn.PixelFormat)
$ratio = $crop.Height / $crop.Width
Save-Scaled $crop 400 ([int][Math]::Round(400 * $ratio)) "$Out\button-2x.png"
Save-Scaled $crop 200 ([int][Math]::Round(200 * $ratio)) "$Out\button-1x.png"
"button crop $($crop.Width)x$($crop.Height)"
$p.Dispose(); $f.Dispose(); $btn.Dispose(); $crop.Dispose()
Get-ChildItem $Out | Select-Object Name, Length
