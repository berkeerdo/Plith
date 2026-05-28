"""
Plith logo gallery generator.

Renders ~20 Plith-themed logo concepts as PNGs and writes an index.html
that displays them all in a grid for visual selection. Each concept is
labeled with an ID so you can tell which one to ship.

Run:
    python tools/logo-gallery.py
Then open tools/gallery/index.html in your browser.
"""
from __future__ import annotations
from pathlib import Path
import math
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "gallery"
OUT.mkdir(exist_ok=True)

# Palette — Plith brand.
ACCENT = (74, 214, 149, 255)        # #4AD695
ACCENT_DIM = (74, 214, 149, 140)
ACCENT_GLOW = (74, 214, 149, 60)
SURFACE = (22, 22, 22, 255)         # #161616
DARK_BG = (10, 10, 10, 255)         # #0A0A0A
WHITE = (245, 245, 245, 255)
MUTED = (90, 90, 90, 255)

CANVAS = 512
CORNER = int(CANVAS * 0.22)
MARGIN = int(CANVAS * 0.05)


def base(bg=SURFACE) -> tuple[Image.Image, ImageDraw.ImageDraw]:
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(
        [(MARGIN, MARGIN), (CANVAS - MARGIN, CANVAS - MARGIN)],
        radius=CORNER, fill=bg,
    )
    return img, d


# ===========================================================================
# Concepts
# ===========================================================================

def find_font(weight: str = "bold") -> str:
    candidates_bold = [
        r"C:\Windows\Fonts\SegoeUIVF.ttf",
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\arialbd.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ]
    candidates_regular = [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ]
    pool = candidates_bold if weight == "bold" else candidates_regular
    for c in pool:
        if Path(c).exists():
            return c
    return candidates_bold[-1]


def thick_line(d: ImageDraw.ImageDraw, x0: float, y0: float, x1: float, y1: float,
               width: float, fill) -> None:
    """Line() with rounded caps via polygon + end ellipses."""
    angle = math.atan2(y1 - y0, x1 - x0)
    perp_x = -math.sin(angle) * width / 2
    perp_y = math.cos(angle) * width / 2
    d.polygon([
        (x0 + perp_x, y0 + perp_y),
        (x1 + perp_x, y1 + perp_y),
        (x1 - perp_x, y1 - perp_y),
        (x0 - perp_x, y0 - perp_y),
    ], fill=fill)
    r = width / 2
    d.ellipse([(x0 - r, y0 - r), (x0 + r, y0 + r)], fill=fill)
    d.ellipse([(x1 - r, y1 - r), (x1 + r, y1 + r)], fill=fill)


# 1. Bold filled P
def c01_bold_p() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    stem_w = CANVAS * 0.14
    stem_x = cx - CANVAS * 0.16
    d.rounded_rectangle(
        [(stem_x - stem_w / 2, cy - CANVAS * 0.30), (stem_x + stem_w / 2, cy + CANVAS * 0.30)],
        radius=stem_w / 2, fill=ACCENT,
    )
    bowl_r = CANVAS * 0.20
    bcx = stem_x + bowl_r * 0.55
    bcy = cy - CANVAS * 0.10
    d.ellipse([(bcx - bowl_r, bcy - bowl_r), (bcx + bowl_r, bcy + bowl_r)], fill=ACCENT)
    hole_r = bowl_r * 0.42
    d.ellipse([(bcx - hole_r, bcy - hole_r), (bcx + hole_r, bcy + hole_r)], fill=SURFACE)
    return img


# 2. Outlined P
def c02_outline_p() -> Image.Image:
    img, d = base()
    font = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.7))
    # Stroke-only via drawing twice with offset would be tedious; do filled then punch.
    text = "P"
    bbox = d.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (CANVAS - tw) / 2 - bbox[0]
    ty = (CANVAS - th) / 2 - bbox[1]
    # Filled accent letter
    d.text((tx, ty), text, font=font, fill=ACCENT)
    # Inner-cut to simulate outline by drawing a thinner inner P in surface.
    inner = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.62))
    bbox2 = d.textbbox((0, 0), text, font=inner)
    tw2 = bbox2[2] - bbox2[0]
    th2 = bbox2[3] - bbox2[1]
    tx2 = (CANVAS - tw2) / 2 - bbox2[0]
    ty2 = (CANVAS - th2) / 2 - bbox2[1]
    d.text((tx2, ty2), text, font=inner, fill=SURFACE)
    return img


# 3. Equalizer bars
def c03_eq_bars() -> Image.Image:
    img, d = base()
    cx = CANVAS / 2
    bar_w = CANVAS * 0.10
    spacing = CANVAS * 0.08
    base_y = CANVAS * 0.74
    for off, h in zip([-(bar_w + spacing), 0, (bar_w + spacing)],
                      [CANVAS * 0.32, CANVAS * 0.46, CANVAS * 0.22]):
        x0 = cx + off - bar_w / 2
        x1 = cx + off + bar_w / 2
        d.rounded_rectangle([(x0, base_y - h), (x1, base_y)], radius=bar_w / 2, fill=ACCENT)
    return img


# 4. Volume fader
def c04_fader() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    track_len = CANVAS * 0.62
    th = CANVAS * 0.06
    x0 = cx - track_len / 2
    x1 = cx + track_len / 2
    d.rounded_rectangle([(x0, cy - th / 2), (x1, cy + th / 2)], radius=th / 2, fill=MUTED)
    fx = x0 + track_len * 0.62
    d.rounded_rectangle([(x0, cy - th / 2), (fx, cy + th / 2)], radius=th / 2, fill=ACCENT)
    tr = CANVAS * 0.075
    d.ellipse([(fx - tr, cy - tr), (fx + tr, cy + tr)], fill=WHITE,
              outline=ACCENT, width=int(CANVAS * 0.015))
    return img


# 5. Concentric sound waves
def c05_waves() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS * 0.32, CANVAS / 2
    for radius, thick, color in [
        (CANVAS * 0.18, int(CANVAS * 0.05), ACCENT),
        (CANVAS * 0.30, int(CANVAS * 0.05), ACCENT),
        (CANVAS * 0.42, int(CANVAS * 0.05), ACCENT_DIM),
    ]:
        d.arc([cx - radius, cy - radius, cx + radius, cy + radius],
              start=-50, end=50, fill=color, width=thick)
    r = CANVAS * 0.045
    d.ellipse([(cx - r, cy - r), (cx + r, cy + r)], fill=WHITE)
    return img


# 6. Stacked overlay (two pills)
def c06_overlay() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    big_w, big_h = CANVAS * 0.62, CANVAS * 0.10
    d.rounded_rectangle(
        [(cx - big_w / 2, cy + CANVAS * 0.06), (cx + big_w / 2, cy + CANVAS * 0.06 + big_h)],
        radius=big_h / 2, fill=MUTED,
    )
    small_w, small_h = CANVAS * 0.40, CANVAS * 0.10
    d.rounded_rectangle(
        [(cx - small_w / 2, cy - CANVAS * 0.18), (cx + small_w / 2, cy - CANVAS * 0.18 + small_h)],
        radius=small_h / 2, fill=ACCENT,
    )
    return img


# 7. Volume knob
def c07_knob() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    r = CANVAS * 0.30
    d.ellipse([(cx - r, cy - r), (cx + r, cy + r)], fill=ACCENT)
    angle = math.radians(-30)
    end_x = cx + r * 0.78 * math.cos(angle)
    end_y = cy + r * 0.78 * math.sin(angle)
    start_x = cx + r * 0.10 * math.cos(angle)
    start_y = cy + r * 0.10 * math.sin(angle)
    thick_line(d, start_x, start_y, end_x, end_y, CANVAS * 0.06, SURFACE)
    cr = r * 0.16
    d.ellipse([(cx - cr, cy - cr), (cx + cr, cy + cr)], fill=SURFACE)
    return img


# 8. Speaker + waves
def c08_speaker() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS * 0.42, CANVAS / 2
    cube_w, cube_h = CANVAS * 0.10, CANVAS * 0.30
    cube_x0 = cx - CANVAS * 0.20
    d.rounded_rectangle(
        [(cube_x0, cy - cube_h / 2), (cube_x0 + cube_w, cy + cube_h / 2)],
        radius=CANVAS * 0.02, fill=ACCENT,
    )
    nx = cube_x0 + cube_w + CANVAS * 0.04
    mx = cx + CANVAS * 0.04
    d.polygon([
        (nx, cy - cube_h * 0.5), (mx, cy - CANVAS * 0.22),
        (mx, cy + CANVAS * 0.22), (nx, cy + cube_h * 0.5),
    ], fill=ACCENT)
    for radius, thick in [(CANVAS * 0.18, int(CANVAS * 0.045)),
                          (CANVAS * 0.30, int(CANVAS * 0.045))]:
        d.arc([cx + CANVAS * 0.10 - radius, cy - radius,
               cx + CANVAS * 0.10 + radius, cy + radius],
              start=-45, end=45, fill=ACCENT, width=thick)
    return img


# 9. Vertical precision meter
def c09_meter() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    col_w = CANVAS * 0.10
    col_top, col_bot = CANVAS * 0.22, CANVAS * 0.78
    d.rounded_rectangle([(cx - col_w / 2, col_top), (cx + col_w / 2, col_bot)],
                        radius=col_w / 2, fill=ACCENT)
    tick_w = CANVAS * 0.10
    tick_h = CANVAS * 0.015
    off = CANVAS * 0.16
    for ty in [col_top + tick_h / 2, (col_top + col_bot) / 2, col_bot - tick_h / 2]:
        d.rounded_rectangle(
            [(cx - col_w / 2 - off - tick_w, ty - tick_h / 2),
             (cx - col_w / 2 - off, ty + tick_h / 2)], radius=tick_h / 2, fill=MUTED,
        )
        d.rounded_rectangle(
            [(cx + col_w / 2 + off, ty - tick_h / 2),
             (cx + col_w / 2 + off + tick_w, ty + tick_h / 2)], radius=tick_h / 2, fill=MUTED,
        )
    pos_y = col_bot - (col_bot - col_top) * 0.75
    dr = CANVAS * 0.045
    d.ellipse([(cx - dr, pos_y - dr), (cx + dr, pos_y + dr)], fill=WHITE,
              outline=ACCENT, width=int(CANVAS * 0.012))
    return img


# 10. Cochlear spiral
def c10_spiral() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    n_turns = 2.5
    n_points = 200
    radius_step = CANVAS * 0.025
    pts = []
    for i in range(n_points):
        t = i / n_points * n_turns * 2 * math.pi
        r = CANVAS * 0.06 + (radius_step * t / (2 * math.pi))
        x = cx + r * math.cos(t)
        y = cy + r * math.sin(t)
        pts.append((x, y))
    for i in range(len(pts) - 1):
        thick_line(d, pts[i][0], pts[i][1], pts[i + 1][0], pts[i + 1][1],
                   CANVAS * 0.04, ACCENT)
    return img


# 11. Single thick P stroke (Linear-style minimal)
def c11_stroke_p() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    sw = CANVAS * 0.10
    stem_x = cx - CANVAS * 0.15
    d.rounded_rectangle(
        [(stem_x - sw / 2, cy - CANVAS * 0.30), (stem_x + sw / 2, cy + CANVAS * 0.30)],
        radius=sw / 2, fill=ACCENT,
    )
    # D-shape bowl as arc
    bcx, bcy = stem_x + CANVAS * 0.08, cy - CANVAS * 0.10
    br = CANVAS * 0.18
    d.arc([bcx - br, bcy - br, bcx + br, bcy + br],
          start=-90, end=90, fill=ACCENT, width=int(CANVAS * 0.10))
    # Close the bowl with a short connector at the top and bottom
    d.rounded_rectangle(
        [(stem_x, bcy - br - CANVAS * 0.04), (bcx, bcy - br + CANVAS * 0.04)],
        radius=CANVAS * 0.04, fill=ACCENT,
    )
    d.rounded_rectangle(
        [(stem_x, bcy + br - CANVAS * 0.04), (bcx, bcy + br + CANVAS * 0.04)],
        radius=CANVAS * 0.04, fill=ACCENT,
    )
    return img


# 12. P with sound wave through bowl (radical)
def c12_p_dashes() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    sw = CANVAS * 0.14
    stem_x = cx - CANVAS * 0.16
    d.rounded_rectangle(
        [(stem_x - sw / 2, cy - CANVAS * 0.30), (stem_x + sw / 2, cy + CANVAS * 0.30)],
        radius=sw / 2, fill=ACCENT,
    )
    br = CANVAS * 0.21
    bcx, bcy = stem_x + br * 0.55, cy - CANVAS * 0.10
    d.ellipse([(bcx - br, bcy - br), (bcx + br, bcy + br)], fill=ACCENT)
    hr = br * 0.42
    d.ellipse([(bcx - hr, bcy - hr), (bcx + hr, bcy + hr)], fill=SURFACE)
    # Dashes
    wt = CANVAS * 0.025
    x0 = MARGIN + CANVAS * 0.06
    x1 = CANVAS - MARGIN - CANVAS * 0.06
    dash, gap = CANVAS * 0.05, CANVAS * 0.03
    x = x0
    while x < x1:
        xe = min(x + dash, x1)
        d.rounded_rectangle([(x, bcy - wt / 2), (xe, bcy + wt / 2)],
                            radius=wt / 2, fill=ACCENT)
        x = xe + gap
    return img


# 13. Hexagonal speaker grille
def c13_hex_grille() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    dot_r = CANVAS * 0.035
    spacing = CANVAS * 0.10
    for row in range(-3, 4):
        offset = (spacing / 2) if row % 2 else 0
        for col in range(-3, 4):
            x = cx + col * spacing + offset
            y = cy + row * spacing * 0.866
            if (x - cx) ** 2 + (y - cy) ** 2 < (CANVAS * 0.30) ** 2:
                d.ellipse([(x - dot_r, y - dot_r), (x + dot_r, y + dot_r)], fill=ACCENT)
    return img


# 14. Negative-space P
def c14_negative_p() -> Image.Image:
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # Accent green rounded square fills the canvas
    d.rounded_rectangle(
        [(MARGIN, MARGIN), (CANVAS - MARGIN, CANVAS - MARGIN)],
        radius=CORNER, fill=ACCENT,
    )
    # Cut a P out using surface color
    font = ImageFont.truetype(find_font("bold"), int(CANVAS * 0.7))
    text = "P"
    bbox = d.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (CANVAS - tw) / 2 - bbox[0]
    ty = (CANVAS - th) / 2 - bbox[1]
    d.text((tx, ty), text, font=font, fill=SURFACE)
    return img


# 15. Asymmetric waveform peak
def c15_wave_peak() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    # A single tall asymmetric waveform-like peak
    pts = []
    for x in range(-int(CANVAS * 0.35), int(CANVAS * 0.35), 2):
        # An asymmetric Gaussian-ish peak
        gx = x / (CANVAS * 0.18)
        y = -CANVAS * 0.28 * math.exp(-gx * gx) * (1 + 0.4 * math.sin(gx * 2))
        pts.append((cx + x, cy + y))
    # Mirror to ground
    base_y = cy + CANVAS * 0.10
    poly = pts + [(pts[-1][0], base_y), (pts[0][0], base_y)]
    d.polygon(poly, fill=ACCENT)
    # Add a thin horizontal axis line under it
    d.rounded_rectangle(
        [(cx - CANVAS * 0.32, base_y - CANVAS * 0.01),
         (cx + CANVAS * 0.32, base_y + CANVAS * 0.01)],
        radius=CANVAS * 0.01, fill=MUTED,
    )
    return img


# 16. Pill with internal waveform
def c16_pill_wave() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    pill_w = CANVAS * 0.62
    pill_h = CANVAS * 0.26
    d.rounded_rectangle(
        [(cx - pill_w / 2, cy - pill_h / 2), (cx + pill_w / 2, cy + pill_h / 2)],
        radius=pill_h / 2, fill=ACCENT,
    )
    # Mini waveform bars inside the pill
    bar_w = CANVAS * 0.025
    spacing = CANVAS * 0.04
    heights = [0.08, 0.12, 0.16, 0.12, 0.18, 0.10, 0.14]
    total_w = len(heights) * bar_w + (len(heights) - 1) * spacing
    start_x = cx - total_w / 2
    for i, h in enumerate(heights):
        bx = start_x + i * (bar_w + spacing)
        hpx = CANVAS * h
        d.rounded_rectangle(
            [(bx, cy - hpx / 2), (bx + bar_w, cy + hpx / 2)],
            radius=bar_w / 2, fill=SURFACE,
        )
    return img


# 17. Play triangle + bar (media controls)
def c17_play_bar() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    # Triangle (play icon)
    s = CANVAS * 0.18
    d.polygon([
        (cx - s, cy - s),
        (cx - s, cy + s),
        (cx + s * 0.8, cy),
    ], fill=ACCENT)
    # Thin bar above
    d.rounded_rectangle(
        [(cx - s * 1.4, cy - s * 1.8), (cx + s * 1.4, cy - s * 1.6)],
        radius=CANVAS * 0.01, fill=ACCENT,
    )
    return img


# 18. Sound wave forming a P silhouette
def c18_wave_p() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    # Stem as a wave
    sw = CANVAS * 0.08
    stem_x = cx - CANVAS * 0.18
    # The stem is a wavy vertical line
    n = 30
    for i in range(n):
        t = i / (n - 1)
        y = cy - CANVAS * 0.30 + t * CANVAS * 0.60
        wave_off = CANVAS * 0.02 * math.sin(t * 6 * math.pi)
        d.ellipse([(stem_x + wave_off - sw / 2, y - sw / 2),
                   (stem_x + wave_off + sw / 2, y + sw / 2)], fill=ACCENT)
    # Bowl as an arc
    bcx, bcy = stem_x + CANVAS * 0.10, cy - CANVAS * 0.12
    br = CANVAS * 0.18
    d.arc([bcx - br, bcy - br, bcx + br, bcy + br],
          start=-90, end=90, fill=ACCENT, width=int(CANVAS * 0.08))
    return img


# 19. Compact dot matrix display (5x7 P)
def c19_dot_p() -> Image.Image:
    img, d = base()
    grid = [
        "11110",
        "10001",
        "10001",
        "11110",
        "10000",
        "10000",
        "10000",
    ]
    cell = CANVAS * 0.08
    grid_w = 5 * cell
    grid_h = 7 * cell
    ox = (CANVAS - grid_w) / 2
    oy = (CANVAS - grid_h) / 2
    dot_r = cell * 0.4
    for ry, row in enumerate(grid):
        for cx_i, ch in enumerate(row):
            if ch == "1":
                x = ox + cx_i * cell + cell / 2
                y = oy + ry * cell + cell / 2
                d.ellipse([(x - dot_r, y - dot_r), (x + dot_r, y + dot_r)], fill=ACCENT)
    return img


# 20. Minimal chevron P (arrow + bar)
def c20_chevron_p() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    # Thick vertical bar on left
    sw = CANVAS * 0.10
    stem_x = cx - CANVAS * 0.18
    d.rounded_rectangle(
        [(stem_x - sw / 2, cy - CANVAS * 0.30), (stem_x + sw / 2, cy + CANVAS * 0.30)],
        radius=sw / 2, fill=ACCENT,
    )
    # Two chevrons (>) facing right of stem — suggesting volume / play / sound emission
    for i in range(2):
        off = i * CANVAS * 0.14
        x0 = stem_x + CANVAS * 0.06 + off
        d.polygon([
            (x0, cy - CANVAS * 0.18),
            (x0 + CANVAS * 0.10, cy),
            (x0, cy + CANVAS * 0.18),
            (x0 - CANVAS * 0.03, cy + CANVAS * 0.18 - CANVAS * 0.02),
            (x0 + CANVAS * 0.05, cy),
            (x0 - CANVAS * 0.03, cy - CANVAS * 0.18 + CANVAS * 0.02),
        ], fill=ACCENT)
    return img


CONCEPTS = [
    ("01", "Bold filled P", c01_bold_p),
    ("02", "Outlined P", c02_outline_p),
    ("03", "Equalizer bars", c03_eq_bars),
    ("04", "Volume fader", c04_fader),
    ("05", "Concentric waves", c05_waves),
    ("06", "Stacked overlay", c06_overlay),
    ("07", "Volume knob", c07_knob),
    ("08", "Speaker + waves", c08_speaker),
    ("09", "Vertical meter", c09_meter),
    ("10", "Cochlear spiral", c10_spiral),
    ("11", "Stroke P (Linear-style)", c11_stroke_p),
    ("12", "P + sound wave", c12_p_dashes),
    ("13", "Hexagonal grille", c13_hex_grille),
    ("14", "Negative-space P", c14_negative_p),
    ("15", "Waveform peak", c15_wave_peak),
    ("16", "Pill with waveform", c16_pill_wave),
    ("17", "Play + bar", c17_play_bar),
    ("18", "Wave-formed P", c18_wave_p),
    ("19", "Dot-matrix P", c19_dot_p),
    ("20", "Chevron P", c20_chevron_p),
]


def write_html() -> None:
    cards = []
    for cid, name, _ in CONCEPTS:
        cards.append(f"""
        <div class="card">
          <div class="img-wrap"><img src="logo-{cid}.png" alt="{name}" /></div>
          <div class="meta">
            <span class="id">#{cid}</span>
            <span class="name">{name}</span>
          </div>
        </div>""")

    html = f"""<!doctype html>
<html lang="tr">
<head>
  <meta charset="utf-8" />
  <title>Plith — Logo Gallery</title>
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
  <h1>Plith — Logo Gallery</h1>
  <p class="sub">{len(CONCEPTS)} konsept. Beğendiğinin numarasını söyle, ben final .ico'yu üretirim.</p>
  <div class="grid">
{''.join(cards)}
  </div>
</body>
</html>
"""
    (OUT / "index.html").write_text(html, encoding="utf-8")


def main() -> None:
    for cid, name, fn in CONCEPTS:
        print(f"[{cid}] {name}")
        img = fn()
        img.save(OUT / f"logo-{cid}.png")
    write_html()
    index_path = (OUT / "index.html").resolve()
    print(f"\nGallery: {index_path}")
    print(f"Open in browser: start \"\" \"{index_path}\"")


if __name__ == "__main__":
    main()
