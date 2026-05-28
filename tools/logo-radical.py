"""
Radical-unique concept — a bold geometric 'P' with a horizontal
sound-wave passing straight through the bowl, exiting both sides.

The fusion of letter (Plith identity) + sound wave (audio function) in
one mark. The wave-through-letter trick has been used by Notion (N
with a slash) and a few others, but never with a sound-wave / never on
a P / never for an audio app.
"""
from __future__ import annotations
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SIZE = 512

ACCENT = (74, 214, 149, 255)
SURFACE = (22, 22, 22, 255)

CANVAS = SIZE
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


def render_radical() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2

    # Bold P — solid bowl + thick stem.
    # Stem: thick vertical bar on the left.
    stem_w = CANVAS * 0.14
    stem_x = cx - CANVAS * 0.16
    stem_top = cy - CANVAS * 0.30
    stem_bot = cy + CANVAS * 0.30
    d.rounded_rectangle(
        [(stem_x - stem_w / 2, stem_top), (stem_x + stem_w / 2, stem_bot)],
        radius=stem_w / 2, fill=ACCENT,
    )

    # Bowl: filled circle, then hollow with the surface color.
    bowl_r = CANVAS * 0.21
    bowl_cx = stem_x + bowl_r * 0.55
    bowl_cy = cy - CANVAS * 0.10
    d.ellipse(
        [(bowl_cx - bowl_r, bowl_cy - bowl_r),
         (bowl_cx + bowl_r, bowl_cy + bowl_r)],
        fill=ACCENT,
    )
    hole_r = bowl_r * 0.42
    d.ellipse(
        [(bowl_cx - hole_r, bowl_cy - hole_r),
         (bowl_cx + hole_r, bowl_cy + hole_r)],
        fill=SURFACE,
    )

    # Horizontal sound-wave passing through the bowl. The wave exits both
    # sides of the P and reads as 'audio passing through'. Drawn as a sequence
    # of small accent dots and a thin connecting line to keep the silhouette
    # crisp at all sizes.
    wave_y = bowl_cy
    wave_thickness = CANVAS * 0.025
    wave_x0 = MARGIN + CANVAS * 0.06
    wave_x1 = CANVAS - MARGIN - CANVAS * 0.06

    # Draw the wave as alternating rounded dashes (Morse-like) for visual rhythm,
    # so it doesn't blend with the bowl's outline.
    dash_len = CANVAS * 0.05
    gap_len = CANVAS * 0.03
    x = wave_x0
    while x < wave_x1:
        x_end = min(x + dash_len, wave_x1)
        d.rounded_rectangle(
            [(x, wave_y - wave_thickness / 2), (x_end, wave_y + wave_thickness / 2)],
            radius=wave_thickness / 2, fill=ACCENT,
        )
        x = x_end + gap_len

    return img


def main() -> None:
    img = render_radical()
    p = ROOT / "logo-radical.png"
    img.save(p)
    print(f"Wrote {p}")


if __name__ == "__main__":
    main()
