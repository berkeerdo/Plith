"""
Plith logo — pill-with-waveform variations.

User picked #16 from the original gallery (pill with internal waveform bars)
as the most promising direction. This script renders 15 variations on that
theme — different pill shapes, waveform styles, and several with a "P"
integrated into the form.

Run:
    python tools/logo-pill-variations.py
Then open tools/gallery/pills.html in your browser.
"""
from __future__ import annotations
from pathlib import Path
import math
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "gallery"
OUT.mkdir(exist_ok=True)

ACCENT = (74, 214, 149, 255)
ACCENT_GLOW = (74, 214, 149, 35)
SURFACE = (22, 22, 22, 255)
SURFACE_DARK = (10, 10, 10, 255)
WHITE = (245, 245, 245, 255)
MUTED = (90, 90, 90, 255)

CANVAS = 512
CORNER = int(CANVAS * 0.22)
MARGIN = int(CANVAS * 0.05)


def base() -> tuple[Image.Image, ImageDraw.ImageDraw]:
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(
        [(MARGIN, MARGIN), (CANVAS - MARGIN, CANVAS - MARGIN)],
        radius=CORNER, fill=SURFACE,
    )
    return img, d


def find_font(weight: str = "bold") -> str:
    candidates = [
        r"C:\Windows\Fonts\SegoeUIVF.ttf",
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\arialbd.ttf",
    ]
    for c in candidates:
        if Path(c).exists():
            return c
    return candidates[-1]


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def draw_eq_bars(d, cx, cy, total_w, heights, bar_w, fill, spacing=None):
    """Draw a row of vertical bars centred on (cx, cy)."""
    if spacing is None:
        spacing = bar_w * 0.6
    full_w = len(heights) * bar_w + (len(heights) - 1) * spacing
    start_x = cx - full_w / 2
    for i, h in enumerate(heights):
        bx = start_x + i * (bar_w + spacing)
        hpx = CANVAS * h
        d.rounded_rectangle(
            [(bx, cy - hpx / 2), (bx + bar_w, cy + hpx / 2)],
            radius=bar_w / 2, fill=fill,
        )


def draw_sine_wave(d, x0, x1, y, amplitude, periods, thickness, fill, samples=80):
    """Draw a smooth sine wave as a sequence of dots/short segments."""
    pts = []
    for i in range(samples):
        t = i / (samples - 1)
        x = x0 + t * (x1 - x0)
        y_off = math.sin(t * periods * 2 * math.pi) * amplitude
        pts.append((x, y + y_off))
    for i in range(len(pts) - 1):
        d.line([pts[i], pts[i + 1]], fill=fill, width=int(thickness))
    # Round caps
    r = thickness / 2
    for x, y in (pts[0], pts[-1]):
        d.ellipse([(x - r, y - r), (x + r, y + r)], fill=fill)


# ---------------------------------------------------------------------------
# Variations
# ---------------------------------------------------------------------------

# PV01 — Baseline: horizontal pill + EQ bars (the original concept)
def pv01_baseline() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.26
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    draw_eq_bars(d, cx, cy, pw, [0.08, 0.12, 0.16, 0.12, 0.18, 0.10, 0.14],
                 CANVAS * 0.025, SURFACE)
    return img


# PV02 — Vertical pill + horizontal wave inside
def pv02_vertical_pill() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.26, CANVAS * 0.62
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=pw / 2, fill=ACCENT,
    )
    # Horizontal EQ bars rotated 90° = horizontal bars stacked vertically
    bar_h = CANVAS * 0.025
    widths = [0.08, 0.14, 0.10, 0.18, 0.12, 0.16, 0.08]
    spacing = bar_h * 0.6
    full_h = len(widths) * bar_h + (len(widths) - 1) * spacing
    start_y = cy - full_h / 2
    for i, w in enumerate(widths):
        by = start_y + i * (bar_h + spacing)
        wpx = CANVAS * w
        d.rounded_rectangle(
            [(cx - wpx / 2, by), (cx + wpx / 2, by + bar_h)],
            radius=bar_h / 2, fill=SURFACE,
        )
    return img


# PV03 — Pill + smooth sine wave inside
def pv03_smooth_sine() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.26
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    draw_sine_wave(d, cx - pw / 2 + CANVAS * 0.06, cx + pw / 2 - CANVAS * 0.06,
                   cy, ph * 0.18, 1.8, CANVAS * 0.025, SURFACE)
    return img


# PV04 — Pill with waveform extending outside as sound emission
def pv04_emission() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.42, CANVAS * 0.26
    pill_cx = cx - CANVAS * 0.08
    d.rounded_rectangle(
        [(pill_cx - pw / 2, cy - ph / 2), (pill_cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # Internal bars
    draw_eq_bars(d, pill_cx, cy, pw, [0.08, 0.14, 0.10, 0.16, 0.08],
                 CANVAS * 0.025, SURFACE)
    # External waveform — bars getting smaller as they emit outward
    for i, h in enumerate([0.18, 0.14, 0.10]):
        bx = pill_cx + pw / 2 + CANVAS * 0.03 + i * CANVAS * 0.06
        d.rounded_rectangle(
            [(bx, cy - CANVAS * h / 2), (bx + CANVAS * 0.025, cy + CANVAS * h / 2)],
            radius=CANVAS * 0.0125, fill=ACCENT,
        )
    return img


# PV05 — Outlined pill, filled waveform inside
def pv05_outlined() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.30
    stroke = int(CANVAS * 0.025)
    # Filled then punch interior
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    d.rounded_rectangle(
        [(cx - pw / 2 + stroke, cy - ph / 2 + stroke),
         (cx + pw / 2 - stroke, cy + ph / 2 - stroke)],
        radius=(ph - 2 * stroke) / 2, fill=SURFACE,
    )
    # Internal EQ bars in accent
    draw_eq_bars(d, cx, cy, pw, [0.10, 0.16, 0.12, 0.20, 0.14, 0.18, 0.10],
                 CANVAS * 0.025, ACCENT)
    return img


# PV06 — Pill with a P silhouette carved out
def pv06_p_carved() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.32
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # Carve a P from the pill
    font = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.32))
    text = "P"
    bbox = d.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (CANVAS - tw) / 2 - bbox[0]
    ty = (CANVAS - th) / 2 - bbox[1]
    d.text((tx, ty), text, font=font, fill=SURFACE)
    return img


# PV07 — Pill with small "P" badge at left end + wave at right
def pv07_p_badge() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.28
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # P at left end (in SURFACE colour)
    font = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.20))
    text = "P"
    bbox = d.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = cx - pw / 2 + CANVAS * 0.06 - bbox[0]
    ty = (CANVAS - th) / 2 - bbox[1]
    d.text((tx, ty), text, font=font, fill=SURFACE)
    # Wave bars in the right half
    bars_cx = cx + CANVAS * 0.12
    draw_eq_bars(d, bars_cx, cy, pw / 2, [0.10, 0.14, 0.08, 0.16, 0.10],
                 CANVAS * 0.025, SURFACE)
    return img


# PV08 — Two stacked pills (one for sound, one P)
def pv08_stacked() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.55, CANVAS * 0.18
    gap = CANVAS * 0.05
    # Top pill — accent with waveform
    top_cy = cy - (ph + gap) / 2
    d.rounded_rectangle(
        [(cx - pw / 2, top_cy - ph / 2), (cx + pw / 2, top_cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    draw_eq_bars(d, cx, top_cy, pw, [0.08, 0.12, 0.08, 0.14, 0.08],
                 CANVAS * 0.022, SURFACE)
    # Bottom pill — muted with "P"
    bot_cy = cy + (ph + gap) / 2
    d.rounded_rectangle(
        [(cx - pw / 2, bot_cy - ph / 2), (cx + pw / 2, bot_cy + ph / 2)],
        radius=ph / 2, fill=MUTED,
    )
    font = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.16))
    text = "P"
    bbox = d.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (CANVAS - tw) / 2 - bbox[0]
    ty = bot_cy - th / 2 - bbox[1]
    d.text((tx, ty), text, font=font, fill=SURFACE)
    return img


# PV09 — P-shaped pill (one end rounded, other end shaped as P bowl curve)
def pv09_p_shaped_pill() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    # Vertical pill (stem of P)
    pw, ph = CANVAS * 0.20, CANVAS * 0.62
    px = cx - CANVAS * 0.12
    d.rounded_rectangle(
        [(px - pw / 2, cy - ph / 2), (px + pw / 2, cy + ph / 2)],
        radius=pw / 2, fill=ACCENT,
    )
    # Bowl at top (a horizontal pill attached)
    bw, bh = CANVAS * 0.34, CANVAS * 0.20
    bcx = px + bw * 0.30
    bcy = cy - CANVAS * 0.15
    d.rounded_rectangle(
        [(bcx - bw / 2, bcy - bh / 2), (bcx + bw / 2, bcy + bh / 2)],
        radius=bh / 2, fill=ACCENT,
    )
    # Hole inside bowl
    hole_w, hole_h = bw * 0.35, bh * 0.45
    d.rounded_rectangle(
        [(bcx + bw * 0.08 - hole_w / 2, bcy - hole_h / 2),
         (bcx + bw * 0.08 + hole_w / 2, bcy + hole_h / 2)],
        radius=hole_h / 2, fill=SURFACE,
    )
    # Waveform bars in the stem (vertical)
    bar_h = CANVAS * 0.022
    widths = [0.06, 0.10, 0.08, 0.12, 0.10]
    spacing = bar_h * 0.6
    full_h = len(widths) * bar_h + (len(widths) - 1) * spacing
    start_y = cy + CANVAS * 0.04 - full_h / 2 + CANVAS * 0.08
    for i, w in enumerate(widths):
        by = start_y + i * (bar_h + spacing)
        wpx = CANVAS * w
        d.rounded_rectangle(
            [(px - wpx / 2, by), (px + wpx / 2, by + bar_h)],
            radius=bar_h / 2, fill=SURFACE,
        )
    return img


# PV10 — Pill with glow halo
def pv10_glow() -> Image.Image:
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(
        [(MARGIN, MARGIN), (CANVAS - MARGIN, CANVAS - MARGIN)],
        radius=CORNER, fill=SURFACE,
    )
    cx, cy = CANVAS / 2, CANVAS / 2
    # Halo first
    for ring in range(6, 0, -1):
        pad = CANVAS * 0.01 * ring
        alpha = int(40 / ring)
        d.rounded_rectangle(
            [(cx - CANVAS * 0.31 - pad, cy - CANVAS * 0.13 - pad),
             (cx + CANVAS * 0.31 + pad, cy + CANVAS * 0.13 + pad)],
            radius=(CANVAS * 0.13 + pad), fill=(74, 214, 149, alpha),
        )
    pw, ph = CANVAS * 0.62, CANVAS * 0.26
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    draw_eq_bars(d, cx, cy, pw, [0.08, 0.12, 0.16, 0.12, 0.18, 0.10, 0.14],
                 CANVAS * 0.025, SURFACE)
    return img


# PV11 — Pill where waveform peaks together form a P silhouette
def pv11_wave_p() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.28
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # Bars arranged with shapes hinting at a "P": tall on left, descending then a bump
    heights = [0.20, 0.16, 0.08, 0.18, 0.12, 0.08, 0.06]
    draw_eq_bars(d, cx, cy, pw, heights, CANVAS * 0.025, SURFACE)
    return img


# PV12 — Tall vertical pill (P-stem) + horizontal wave line passing through
def pv12_stem_through() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    # Vertical pill
    pw, ph = CANVAS * 0.22, CANVAS * 0.62
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=pw / 2, fill=ACCENT,
    )
    # Horizontal wave bars passing through the pill (in SURFACE inside pill, ACCENT outside)
    wt = CANVAS * 0.025
    bar_w = CANVAS * 0.06
    gap = CANVAS * 0.03
    x = MARGIN + CANVAS * 0.04
    end = CANVAS - MARGIN - CANVAS * 0.04
    while x < end:
        xe = min(x + bar_w, end)
        # Inside pill: surface colour, outside: accent
        if x + bar_w / 2 < cx - pw / 2 or x + bar_w / 2 > cx + pw / 2:
            d.rounded_rectangle([(x, cy - wt / 2), (xe, cy + wt / 2)],
                                radius=wt / 2, fill=ACCENT)
        else:
            d.rounded_rectangle([(x, cy - wt / 2), (xe, cy + wt / 2)],
                                radius=wt / 2, fill=SURFACE)
        x = xe + gap
    return img


# PV13 — Pill with waveform peaks rising out the top (skyline / horizon)
def pv13_skyline() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS * 0.58
    pw, ph = CANVAS * 0.66, CANVAS * 0.16
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # Bars rising above the pill
    bar_w = CANVAS * 0.04
    spacing = CANVAS * 0.03
    heights = [0.10, 0.18, 0.12, 0.22, 0.14, 0.20, 0.10, 0.16]
    full_w = len(heights) * bar_w + (len(heights) - 1) * spacing
    start_x = cx - full_w / 2
    for i, h in enumerate(heights):
        bx = start_x + i * (bar_w + spacing)
        hpx = CANVAS * h
        d.rounded_rectangle(
            [(bx, cy - ph / 2 - hpx), (bx + bar_w, cy - ph / 2 + bar_w / 2)],
            radius=bar_w / 2, fill=ACCENT,
        )
    return img


# PV14 — Pill split by a vertical wave divider
def pv14_split() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.30
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # Vertical zig-zag splitter
    samples = 60
    y_top, y_bot = cy - ph / 2 + CANVAS * 0.03, cy + ph / 2 - CANVAS * 0.03
    pts = []
    for i in range(samples):
        t = i / (samples - 1)
        y = y_top + t * (y_bot - y_top)
        x = cx + math.sin(t * 6 * math.pi) * CANVAS * 0.02
        pts.append((x, y))
    for i in range(len(pts) - 1):
        d.line([pts[i], pts[i + 1]], fill=SURFACE, width=int(CANVAS * 0.025))
    # P on left half
    font = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.16))
    text = "P"
    bbox = d.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    d.text((cx - pw * 0.30 - tw / 2 - bbox[0], cy - th / 2 - bbox[1]),
           text, font=font, fill=SURFACE)
    # Mini bars on right half
    draw_eq_bars(d, cx + pw * 0.22, cy, pw / 3,
                 [0.10, 0.14, 0.08, 0.12], CANVAS * 0.022, SURFACE)
    return img


# PV15 — Pill with bowl notch (P-bowl carved into one end) + wave inside
def pv15_p_notch() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pw, ph = CANVAS * 0.62, CANVAS * 0.30
    d.rounded_rectangle(
        [(cx - pw / 2, cy - ph / 2), (cx + pw / 2, cy + ph / 2)],
        radius=ph / 2, fill=ACCENT,
    )
    # P-bowl hole at the left end
    hole_r = ph * 0.35
    hole_cx = cx - pw / 2 + ph * 0.42
    d.ellipse(
        [(hole_cx - hole_r, cy - hole_r), (hole_cx + hole_r, cy + hole_r)],
        fill=SURFACE,
    )
    # Waveform bars in the rest of the pill
    bars_cx = cx + CANVAS * 0.06
    draw_eq_bars(d, bars_cx, cy, pw * 0.50,
                 [0.10, 0.16, 0.10, 0.18, 0.12, 0.16, 0.08],
                 CANVAS * 0.022, SURFACE)
    return img


CONCEPTS = [
    ("PV01", "Original pill + EQ bars", pv01_baseline),
    ("PV02", "Vertical pill", pv02_vertical_pill),
    ("PV03", "Pill + smooth sine wave", pv03_smooth_sine),
    ("PV04", "Sound emission (wave exits)", pv04_emission),
    ("PV05", "Outlined pill", pv05_outlined),
    ("PV06", "P carved out", pv06_p_carved),
    ("PV07", "P badge + wave", pv07_p_badge),
    ("PV08", "Two stacked pills", pv08_stacked),
    ("PV09", "P-shaped pill (stem + bowl)", pv09_p_shaped_pill),
    ("PV10", "Pill with glow halo", pv10_glow),
    ("PV11", "Bars forming P hint", pv11_wave_p),
    ("PV12", "Vertical pill + wave through", pv12_stem_through),
    ("PV13", "Skyline waveform on pill", pv13_skyline),
    ("PV14", "Split pill (P + wave)", pv14_split),
    ("PV15", "P-bowl notch + wave", pv15_p_notch),
]


def write_html() -> None:
    cards = []
    for cid, name, _ in CONCEPTS:
        cards.append(f"""
        <div class="card">
          <div class="img-wrap"><img src="pill-{cid}.png" alt="{name}" /></div>
          <div class="meta">
            <span class="id">#{cid}</span>
            <span class="name">{name}</span>
          </div>
        </div>""")

    html = f"""<!doctype html>
<html lang="tr">
<head>
  <meta charset="utf-8" />
  <title>Plith — Pill Variations</title>
  <style>
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      padding: 40px;
      font-family: "Segoe UI Variable", "Segoe UI", system-ui, sans-serif;
      background: #0A0A0A;
      color: #E5E5E5;
    }}
    h1 {{
      font-size: 28px;
      font-weight: 600;
      margin: 0 0 4px 0;
      color: #F5F5F5;
    }}
    h1::before {{
      content: "";
      display: inline-block;
      width: 8px; height: 8px;
      border-radius: 50%;
      background: #4AD695;
      margin-right: 12px;
      vertical-align: middle;
    }}
    .sub {{ color: #A1A1AA; font-size: 14px; margin-bottom: 36px; }}
    .grid {{
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
      gap: 20px;
    }}
    .card {{
      background: #161616;
      border: 1px solid #1F1F1F;
      border-radius: 12px;
      padding: 16px;
      transition: border-color 0.15s, transform 0.15s;
    }}
    .card:hover {{
      border-color: #4AD695;
      transform: translateY(-2px);
    }}
    .img-wrap {{
      background: #0A0A0A;
      border-radius: 8px;
      padding: 12px;
      display: flex;
      align-items: center;
      justify-content: center;
    }}
    .img-wrap img {{
      width: 100%;
      height: auto;
      display: block;
      image-rendering: -webkit-optimize-contrast;
    }}
    .meta {{
      display: flex;
      align-items: baseline;
      gap: 8px;
      margin-top: 12px;
    }}
    .id {{
      font-family: "Cascadia Code", Consolas, monospace;
      font-size: 11px;
      color: #4AD695;
      background: rgba(74, 214, 149, 0.1);
      padding: 2px 6px;
      border-radius: 4px;
    }}
    .name {{ font-size: 13px; color: #E5E5E5; }}
  </style>
</head>
<body>
  <h1>Plith — Pill variations</h1>
  <p class="sub">{len(CONCEPTS)} varyasyon, 16 ana fikrinden türetildi. Beğendiğini söyle (ör. "PV09 olsun") veya birkaç favori söyle, daha alt-varyant üretirim.</p>
  <div class="grid">
{''.join(cards)}
  </div>
</body>
</html>
"""
    (OUT / "pills.html").write_text(html, encoding="utf-8")


def main() -> None:
    for cid, name, fn in CONCEPTS:
        print(f"[{cid}] {name}")
        img = fn()
        img.save(OUT / f"pill-{cid}.png")
    write_html()
    path = (OUT / "pills.html").resolve()
    print(f"\nGallery: {path}")


if __name__ == "__main__":
    main()
