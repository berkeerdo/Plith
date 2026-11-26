# Usage: pwsh -File make-halka.ps1 -In <folder holding inst/p2.png, inst/flat.png, inst/button.png> -Out <new folder>
# Cuts the Halka theme art from the installer sources (see make-states.ps1 for the ring masks):
#   floor-{1x,2x}.png   the lime fabric: flat with the source disc cloned out, 640x420 and 1280x840
#   ring-K-{1x,2x}.png  ring K (1..3) of p2 with alpha = the ring mask, cropped to its bounding box
#   disc-{1x,2x}.png    the glossy black source cut from p2 (black body plus its rim shadow), cropped
#   pill-{1x,2x}.png    button.png, its lower half, 32 DIP high (1x) / 64 px (2x)
# It also prints the measurements the README records: sizes, rectangles relative to the source
# centre, fabric colours, the text zone's WCAG luminance, the composite check against bg-3-1x.png.
param(
    [string]$In,
    [string]$Out,
    [float[]]$Bounds = @(160, 310, 450),
    [string]$Ink = '141A05',
    # Text zone in DIP relative to the open island's top left: x, y, width, height (see README).
    [float[]]$Zone = @(30, 12, 566, 394),
    # The fabric's left edge sits IslandX DIP left of the island's left edge: the source centre
    # (320 DIP in the 640 wide fabric) lands at the island's centre (310 DIP on a 620 wide page).
    [float]$IslandX = 10,
    [string]$Compare = ''
)
$ErrorActionPreference = 'Stop'
[System.Threading.Thread]::CurrentThread.CurrentCulture = [System.Globalization.CultureInfo]::InvariantCulture
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing, System.Drawing.Primitives, System.Drawing.Common, System.Collections -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Halka {
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

    public static float Lum(byte[] p, int i) { return 0.0722f * p[i] + 0.7152f * p[i + 1] + 0.2126f * p[i + 2]; }

    // Box blur of a float field with radius r, separable, edges clamped (from make-states.ps1).
    public static float[] Blur(float[] f, int w, int h, int r) {
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

    // Three ring masks (0..1), inner first. Same code as make-states.ps1.
    public static float[][] Masks(byte[] pressed, byte[] flat, int w, int h, float[] bounds) {
        var rings = new float[3][];
        for (int k = 0; k < 3; k++) rings[k] = new float[w * h];
        for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
            int i = (y * w + x) * 4;
            if (Lum(flat, i) - Lum(pressed, i) < 40f) continue;
            double r = Math.Sqrt((x - 960.0) * (x - 960.0) + (y - 100.0) * (y - 100.0));
            if (r < bounds[0]) continue;
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

    // The source disc: body (dark, filled down to the lowest dark pixel of each column so the
    // specular highlights stay inside) plus a soft rim shadow outside it. Returns alpha 0..1.
    public static float[] DiscAlpha(byte[] p2, int w, int h, float[] body) {
        // Walk each column down from the top edge: the body is the dark run that starts there,
        // bridging gaps (specular highlights) of up to 45 px, so the ripple grooves stay out.
        var bottoms = new int[w];
        for (int x = 740; x < 1180; x++) {
            int bottom = -1, gap = 0;
            for (int y = 0; y < 300; y++) {
                if (Lum(p2, (y * w + x) * 4) < 60f) { bottom = y; gap = 0; }
                else if (++gap > 45) break;
            }
            bottoms[x] = bottom;
        }
        // A column inside the body that failed (a wide specular highlight) takes its neighbours' depth.
        var fixedBottoms = (int[])bottoms.Clone();
        for (int x = 756; x < 1164; x++) {
            if (bottoms[x - 16] >= 0 && bottoms[x + 16] >= 0)
                fixedBottoms[x] = Math.Max(bottoms[x], Math.Min(bottoms[x - 16], bottoms[x + 16]));
        }
        for (int x = 740; x < 1180; x++)
            for (int y = 0; y <= fixedBottoms[x]; y++) body[y * w + x] = 1f;
        var soft = Blur(body, w, h, 1);
        var near = Blur(body, w, h, 28);
        double sum = 0; int n = 0;
        for (int y = 900; y < 1200; y += 3) for (int x = 100; x < 400; x += 3) { sum += Lum(p2, (y * w + x) * 4); n++; }
        float fref = (float)(sum / n);
        var a = new float[w * h];
        for (int j = 0; j < a.Length; j++) {
            float s = 0;
            if (near[j] > 0.001f && body[j] < 0.5f) {
                s = Math.Max(0f, Math.Min(1f, (1f - Lum(p2, j * 4) / fref - 0.08f) / 0.9f));
                s = Math.Min(0.7f, s) * Math.Min(1f, near[j] * 4f);
            }
            a[j] = Math.Max(soft[j], s);
        }
        return a;
    }

    // Median-cut quantizer: an opaque 8 bit indexed bitmap with a 256 colour palette, no dithering.
    // Used for the floor only (the weave is noise, so a 24 bit PNG of it is huge). Returns the mean
    // absolute error per channel through err.
    public static Bitmap Quantize(byte[] px, int w, int h, out double err) {
        int n = w * h;
        var cnt = new int[32768];
        var sum = new long[32768 * 3];
        for (int j = 0; j < n; j++) {
            int b = px[j * 4], g = px[j * 4 + 1], r = px[j * 4 + 2];
            int key = ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3);
            cnt[key]++; sum[key * 3] += r; sum[key * 3 + 1] += g; sum[key * 3 + 2] += b;
        }
        var boxes = new System.Collections.Generic.List<System.Collections.Generic.List<int>>();
        var all = new System.Collections.Generic.List<int>();
        for (int k = 0; k < 32768; k++) if (cnt[k] > 0) all.Add(k);
        boxes.Add(all);
        while (boxes.Count < 256) {
            int bi = -1; double bs = 0;
            for (int i = 0; i < boxes.Count; i++) {
                if (boxes[i].Count < 2) continue;
                long c = 0; foreach (int k in boxes[i]) c += cnt[k];
                int[] lo = { 31, 31, 31 }, hi = { 0, 0, 0 };
                foreach (int k in boxes[i]) { int[] v = { k >> 10, (k >> 5) & 31, k & 31 };
                    for (int a = 0; a < 3; a++) { if (v[a] < lo[a]) lo[a] = v[a]; if (v[a] > hi[a]) hi[a] = v[a]; } }
                double s = c * (double)Math.Max(hi[0] - lo[0], Math.Max(hi[1] - lo[1], hi[2] - lo[2]) + 1);
                if (s > bs) { bs = s; bi = i; }
            }
            if (bi < 0) break;
            var bx = boxes[bi];
            int[] lo2 = { 31, 31, 31 }, hi2 = { 0, 0, 0 };
            foreach (int k in bx) { int[] v = { k >> 10, (k >> 5) & 31, k & 31 };
                for (int a = 0; a < 3; a++) { if (v[a] < lo2[a]) lo2[a] = v[a]; if (v[a] > hi2[a]) hi2[a] = v[a]; } }
            int axis = 0; for (int a = 1; a < 3; a++) if (hi2[a] - lo2[a] > hi2[axis] - lo2[axis]) axis = a;
            int sh = axis == 0 ? 10 : axis == 1 ? 5 : 0;
            bx.Sort((p, q) => ((p >> sh) & 31).CompareTo((q >> sh) & 31));
            long total = 0; foreach (int k in bx) total += cnt[k];
            long acc = 0; int cut = 1;
            for (int i = 0; i < bx.Count - 1; i++) { acc += cnt[bx[i]]; cut = i + 1; if (acc * 2 >= total) break; }
            var left = bx.GetRange(0, cut); var right = bx.GetRange(cut, bx.Count - cut);
            boxes[bi] = left; boxes.Add(right);
        }
        var pal = new int[boxes.Count * 3];
        for (int i = 0; i < boxes.Count; i++) {
            long c = 0, r = 0, g = 0, b = 0;
            foreach (int k in boxes[i]) { c += cnt[k]; r += sum[k * 3]; g += sum[k * 3 + 1]; b += sum[k * 3 + 2]; }
            pal[i * 3] = (int)Math.Round((double)r / c); pal[i * 3 + 1] = (int)Math.Round((double)g / c); pal[i * 3 + 2] = (int)Math.Round((double)b / c);
        }
        var bmp = new Bitmap(w, h, PixelFormat.Format8bppIndexed);
        var cp = bmp.Palette;
        for (int i = 0; i < 256; i++) cp.Entries[i] = i < boxes.Count ? Color.FromArgb(255, pal[i * 3], pal[i * 3 + 1], pal[i * 3 + 2]) : Color.Black;
        bmp.Palette = cp;
        var d = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format8bppIndexed);
        var idx = new byte[d.Stride * h];
        var cache = new int[32768]; for (int i = 0; i < cache.Length; i++) cache[i] = -1;
        double e = 0;
        for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
            int j = y * w + x;
            int b = px[j * 4], g = px[j * 4 + 1], r = px[j * 4 + 2];
            int key = ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3);
            int best = 0; int bd = int.MaxValue;
            for (int i = 0; i < boxes.Count; i++) {
                int dr = r - pal[i * 3], dg = g - pal[i * 3 + 1], db = b - pal[i * 3 + 2];
                int dd = dr * dr + dg * dg + db * db;
                if (dd < bd) { bd = dd; best = i; }
            }
            idx[y * d.Stride + x] = (byte)best;
            e += (Math.Abs(r - pal[best * 3]) + Math.Abs(g - pal[best * 3 + 1]) + Math.Abs(b - pal[best * 3 + 2])) / 3.0;
        }
        Marshal.Copy(idx, 0, d.Scan0, idx.Length);
        bmp.UnlockBits(d);
        err = e / n;
        return bmp;
    }

    // Clone the source disc out of flat: copy the fabric from a horizontal shift whose weave
    // lines up best with the surroundings of the patch, feathered at the edge. Returns the shift.
    public static int CloneOut(byte[] flat, byte[] outPx, float[] body, int w, int h) {
        var core = Blur(body, w, h, 30);
        var wide = Blur(body, w, h, 46);
        var band = new System.Collections.Generic.List<int>();
        for (int j = 0; j < w * h; j++) if (wide[j] > 0.001f && core[j] <= 0.001f) band.Add(j);
        long bestErr = long.MaxValue; int best = 0;
        for (int dx = -820; dx <= 820; dx++) {
            if (Math.Abs(dx) < 520) continue;
            long err = 0; bool ok = true;
            foreach (int j in band) {
                int x = j % w + dx;
                if (x < 0 || x >= w) { ok = false; break; }
                int i = j * 4, k = (j - j % w + x) * 4;
                for (int c = 0; c < 3; c++) err += Math.Abs(flat[i + c] - flat[k + c]);
            }
            if (ok && err < bestErr) { bestErr = err; best = dx; }
        }
        var m = new float[w * h];
        for (int j = 0; j < m.Length; j++) m[j] = core[j] > 0.001f ? 1f : 0f;
        var feather = Blur(m, w, h, 8);
        Array.Copy(flat, outPx, flat.Length);
        for (int j = 0; j < w * h; j++) {
            float f = Math.Min(1f, feather[j] * 2f);
            if (f <= 0f) continue;
            int k = (j - j % w + (j % w + best)) * 4;
            for (int c = 0; c < 3; c++) outPx[j * 4 + c] = (byte)Math.Round(flat[j * 4 + c] * (1 - f) + flat[k + c] * f);
        }
        return best;
    }
}
'@

function New-Scaled($bmp, [int]$sw, [int]$sh) {
    $o = [System.Drawing.Bitmap]::new($sw, $sh, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($o)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.DrawImage($bmp, 0, 0, $sw, $sh)
    $g.Dispose()
    return $o
}

# Scales an opaque RGB field and a 0..1 alpha field to (sw, sh); returns @(rgb bytes, alpha bytes).
function Get-Scaled([byte[]]$rgb, [float[]]$alpha, [int]$w, [int]$h, [int]$sw, [int]$sh) {
    $src = [Halka]::Write($rgb, $w, $h)
    $s = New-Scaled $src $sw $sh
    $rgbS = [Halka]::Read($s); $s.Dispose(); $src.Dispose()
    $a = [byte[]]::new($w * $h * 4)
    for ($j = 0; $j -lt $w * $h; $j++) {
        $v = [byte][Math]::Round([Math]::Min(1, [Math]::Max(0, $alpha[$j])) * 255)
        $a[$j * 4] = $v; $a[$j * 4 + 1] = $v; $a[$j * 4 + 2] = $v; $a[$j * 4 + 3] = 255
    }
    $ab = [Halka]::Write($a, $w, $h)
    $asc = New-Scaled $ab $sw $sh
    $aRead = [Halka]::Read($asc); $asc.Dispose(); $ab.Dispose()
    $al = [byte[]]::new($sw * $sh)
    for ($j = 0; $j -lt $sw * $sh; $j++) { $al[$j] = $aRead[$j * 4 + 1] }
    return , @($rgbS, $al)
}

function Get-Box([byte[]]$al, [int]$w, [int]$h, [int]$min) {
    $x0 = $w; $y0 = $h; $x1 = -1; $y1 = -1
    for ($y = 0; $y -lt $h; $y++) { for ($x = 0; $x -lt $w; $x++) {
        if ($al[$y * $w + $x] -gt $min) {
            if ($x -lt $x0) { $x0 = $x }; if ($x -gt $x1) { $x1 = $x }
            if ($y -lt $y0) { $y0 = $y }; if ($y -gt $y1) { $y1 = $y } } } }
    return @($x0, $y0, ($x1 - $x0 + 1), ($y1 - $y0 + 1))
}

# Writes the box (x, y, w, h) of a scaled rgb + alpha layer as a straight-alpha PNG.
function Save-Layer([byte[]]$rgb, [byte[]]$al, [int]$w, [int[]]$box, [string]$path) {
    $bw = $box[2]; $bh = $box[3]
    $o = [byte[]]::new($bw * $bh * 4)
    for ($y = 0; $y -lt $bh; $y++) { for ($x = 0; $x -lt $bw; $x++) {
        $s = (($y + $box[1]) * $w + $x + $box[0]); $d = ($y * $bw + $x) * 4
        $o[$d] = $rgb[$s * 4]; $o[$d + 1] = $rgb[$s * 4 + 1]; $o[$d + 2] = $rgb[$s * 4 + 2]; $o[$d + 3] = $al[$s] } }
    $b = [Halka]::Write($o, $bw, $bh)
    $b.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $b.Dispose()
}

function Get-Lin([double]$c) { $c = $c / 255.0; if ($c -le 0.03928) { return $c / 12.92 } else { return [Math]::Pow(($c + 0.055) / 1.055, 2.4) } }

New-Item -ItemType Directory -Path $Out | Out-Null
$p = [System.Drawing.Bitmap]::new("$In\inst\p2.png")
$f = [System.Drawing.Bitmap]::new("$In\inst\flat.png")
$w = $p.Width; $h = $p.Height
$pp = [Halka]::Read($p); $fp = [Halka]::Read($f)
$p.Dispose(); $f.Dispose()
"source ${w}x${h}"

$masks = [Halka]::Masks($pp, $fp, $w, $h, $Bounds)
$body = [float[]]::new($w * $h)
$disc = [Halka]::DiscAlpha($pp, $w, $h, $body)
$floor = [byte[]]::new($fp.Length)
$dx = [Halka]::CloneOut($fp, $floor, $body, $w, $h)
"disc cloned out of the floor with horizontal shift $dx px"

# Opaque colour for the disc: p2 inside the body, black elsewhere (the shadow is black with alpha).
$discRgb = [byte[]]::new($pp.Length)
$opaqueP = [byte[]]::new($pp.Length)
for ($j = 0; $j -lt $w * $h; $j++) {
    if ($disc[$j] -gt 0.5) { for ($c = 0; $c -lt 3; $c++) { $discRgb[$j * 4 + $c] = $pp[$j * 4 + $c] } }
    $discRgb[$j * 4 + 3] = 255
    for ($c = 0; $c -lt 3; $c++) { $opaqueP[$j * 4 + $c] = $pp[$j * 4 + $c] }
    $opaqueP[$j * 4 + 3] = 255
    $floor[$j * 4 + 3] = 255
}

$one = [float[]]::new($w * $h); for ($j = 0; $j -lt $one.Length; $j++) { $one[$j] = 1 }
$layers = @(
    @{ name = 'ring-1'; rgb = $opaqueP; a = $masks[0] },
    @{ name = 'ring-2'; rgb = $opaqueP; a = $masks[1] },
    @{ name = 'ring-3'; rgb = $opaqueP; a = $masks[2] },
    @{ name = 'disc'; rgb = $discRgb; a = $disc }
)
$cx1 = 320.0; $cy1 = 100.0 * 420 / $h   # source centre in 1x px (the bg files scale y by 420/1264)
$store = @{}
"--- files: pixel size, then the DIP rect relative to the source centre (x, y, w, h)"
foreach ($sc in @(@(1, 640, 420), @(2, 1280, 840))) {
    $m = $sc[0]; $sw = $sc[1]; $sh = $sc[2]
    $fl = Get-Scaled $floor $one $w $h $sw $sh
    $qerr = 0.0
    $qb = [Halka]::Quantize($fl[0], $sw, $sh, [ref]$qerr)
    $qb.Save("$Out\floor-${m}x.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $store["floor$m"] = [Halka]::Read($qb)   # measure what ships: the quantized floor
    $qb.Dispose()
    "floor-${m}x.png ${sw}x${sh} (256 colour palette, mean quantization error {0:0.00} levels per channel)" -f $qerr
    foreach ($l in $layers) {
        $s = Get-Scaled $l.rgb $l.a $w $h $sw $sh
        if ($m -eq 1) { $store[$l.name + 'box'] = Get-Box $s[1] $sw $sh 2 }
        $b1 = $store[$l.name + 'box']
        $box = @(($b1[0] * $m), ($b1[1] * $m), ($b1[2] * $m), ($b1[3] * $m))
        Save-Layer $s[0] $s[1] $sw $box "$Out\$($l.name)-${m}x.png"
        $store[$l.name + $m + 'rgb'] = $s[0]; $store[$l.name + $m + 'al'] = $s[1]
        "{0}-{1}x.png {2}x{3}  rect {4:0.##}, {5:0.##}, {6}, {7}" -f $l.name, $m, $box[2], $box[3], ($b1[0] - $cx1), ($b1[1] - $cy1), $b1[2], $b1[3]
    }
}

# Pill: button.png cropped to its body, lower half, 32 DIP high at 1x.
$btn = [System.Drawing.Bitmap]::new("$In\inst\button.png")
$bp = [Halka]::Read($btn); $bw = $btn.Width; $bh = $btn.Height; $btn.Dispose()
$ba = [byte[]]::new($bw * $bh); for ($j = 0; $j -lt $ba.Length; $j++) { $ba[$j] = $bp[$j * 4 + 3] }
$bb = Get-Box $ba $bw $bh 128
$halfY = $bb[1] + [int][Math]::Floor($bb[3] / 2)
$halfH = $bb[1] + $bb[3] - $halfY
for ($j = 0; $j -lt $bw * $bh; $j++) { if ($bp[$j * 4 + 3] -lt 16) { $bp[$j * 4] = 0; $bp[$j * 4 + 1] = 0; $bp[$j * 4 + 2] = 0 } }
$srcBmp = [Halka]::Write($bp, $bw, $bh)
foreach ($m in 1, 2) {
    $th = 32 * $m
    $tw = [int][Math]::Round($bb[2] * $th / $halfH)
    $crop = $srcBmp.Clone([System.Drawing.Rectangle]::new($bb[0], $halfY, $bb[2], $halfH), [System.Drawing.Imaging.PixelFormat]::Format32bppPArgb)
    $o = [System.Drawing.Bitmap]::new($tw, $th, [System.Drawing.Imaging.PixelFormat]::Format32bppPArgb)
    $g = [System.Drawing.Graphics]::FromImage($o)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
    $g.DrawImage($crop, 0, 0, $tw, $th)
    $g.Dispose()
    $o.Save("$Out\pill-${m}x.png", [System.Drawing.Imaging.ImageFormat]::Png)
    "pill-${m}x.png ${tw}x${th}  (button body $($bb[2])x$($bb[3]) px, lower half from row $halfY)"
    $o.Dispose(); $crop.Dispose()
}
$srcBmp.Dispose()

# Fabric mean colour (1x floor).
$fl1 = $store['floor1']
$n = 640 * 420; $sr = 0.0; $sg = 0.0; $sb = 0.0
for ($j = 0; $j -lt $n; $j++) { $sb += $fl1[$j * 4]; $sg += $fl1[$j * 4 + 1]; $sr += $fl1[$j * 4 + 2] }
"fabric mean colour (1x floor): #{0:X2}{1:X2}{2:X2}" -f [int][Math]::Round($sr / $n), [int][Math]::Round($sg / $n), [int][Math]::Round($sb / $n)

$lut = [double[]]::new(256); for ($i = 0; $i -lt 256; $i++) { $lut[$i] = Get-Lin $i }
$inkR = [Convert]::ToInt32($Ink.Substring(0, 2), 16); $inkG = [Convert]::ToInt32($Ink.Substring(2, 2), 16); $inkB = [Convert]::ToInt32($Ink.Substring(4, 2), 16)
$lInk = 0.2126 * $lut[$inkR] + 0.7152 * $lut[$inkG] + 0.0722 * $lut[$inkB]
foreach ($m in 1, 2) {
    $fw = 640 * $m; $fl = $store["floor$m"]
    $zx0 = [int][Math]::Round(($Zone[0] + $IslandX) * $m); $zy0 = [int][Math]::Round($Zone[1] * $m)
    $zx1 = [int][Math]::Round(($Zone[0] + $Zone[2] + $IslandX) * $m); $zy1 = [int][Math]::Round(($Zone[1] + $Zone[3]) * $m)
    $lo = 9.0; $hi = -1.0; $loC = ''; $hiC = ''; $sum = 0.0; $cnt = 0
    for ($y = $zy0; $y -lt $zy1; $y++) { for ($x = $zx0; $x -lt $zx1; $x++) {
        $i = ($y * $fw + $x) * 4
        $l = 0.2126 * $lut[$fl[$i + 2]] + 0.7152 * $lut[$fl[$i + 1]] + 0.0722 * $lut[$fl[$i]]
        $sum += $l; $cnt++
        if ($l -lt $lo) { $lo = $l; $loC = '#{0:X2}{1:X2}{2:X2}' -f $fl[$i + 2], $fl[$i + 1], $fl[$i] }
        if ($l -gt $hi) { $hi = $l; $hiC = '#{0:X2}{1:X2}{2:X2}' -f $fl[$i + 2], $fl[$i + 1], $fl[$i] } } }
    $ratio = ($lo + 0.05) / ($lInk + 0.05)
    "text zone ${m}x px ($zx0,$zy0)-($zx1,$zy1): lum min {0:0.0000} ({1}) max {2:0.0000} ({3}) mean {4:0.0000}; ink L {5:0.0000}; worst contrast {6:0.00}:1" -f $lo, $loC, $hi, $hiC, ($sum / $cnt), $lInk, $ratio
}

# Composite check: floor + ring 1..3 + disc at 1x against bg-3-1x over the ripple area.
if ($Compare -eq '') { $Compare = "$In\bg-3-1x.png" }
$bgBmp = [System.Drawing.Bitmap]::new($Compare); $bg = [Halka]::Read($bgBmp); $bgBmp.Dispose()
$comp = [byte[]]::new($fl1.Length); [Array]::Copy($fl1, $comp, $fl1.Length)
$area = [bool[]]::new($n)
foreach ($name in 'ring-1', 'ring-2', 'ring-3', 'disc') {
    $rgb = $store[$name + '1rgb']; $al = $store[$name + '1al']
    for ($j = 0; $j -lt $n; $j++) {
        $a = $al[$j] / 255.0
        if ($a -gt 0.02) { $area[$j] = $true }
        if ($a -gt 0) { for ($c = 0; $c -lt 3; $c++) { $comp[$j * 4 + $c] = [byte][Math]::Round($comp[$j * 4 + $c] * (1 - $a) + $rgb[$j * 4 + $c] * $a) } }
    }
}
$dsum = 0.0; $dn = 0; $asum = 0.0
for ($j = 0; $j -lt $n; $j++) {
    $d = 0; for ($c = 0; $c -lt 3; $c++) { $d += [Math]::Abs($comp[$j * 4 + $c] - $bg[$j * 4 + $c]) }
    $asum += $d / 3.0
    if ($area[$j]) { $dsum += $d / 3.0; $dn++ }
}
"composite vs bg-3-1x: mean abs diff per channel over the ripple area ({0} px): {1:0.00} levels; whole image: {2:0.00}" -f $dn, ($dsum / $dn), ($asum / $n)
$cb = [Halka]::Write($comp, 640, 420); $cb.Save("$Out\composite-check-1x.png", [System.Drawing.Imaging.ImageFormat]::Png); $cb.Dispose()

$total = 0; Get-ChildItem $Out -Filter *.png | Where-Object { $_.Name -ne 'composite-check-1x.png' } | ForEach-Object { $total += $_.Length; "{0,-16} {1,9}" -f $_.Name, $_.Length }
"total bytes (art files, without composite-check-1x.png): $total"
