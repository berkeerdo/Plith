"""
Generates the Plith application icon — Sound emission concept (PV04 from the
pill-variations gallery).

Design intent: an accent-green pill on the left containing small internal
waveform bars, with two-to-three trailing bars to the right shrinking
outward like sound emanating from the source. Universal 'audio source +
emission' read; distinctive silhouette no other audio app uses verbatim.

Run after design changes:
    python tools/generate-icon.py

Output: src/Plith/Resources/icons/plith.ico (multi-resolution).
"""
from __future__ import annotations

from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "Plith" / "Resources" / "icons" / "plith.ico"
SIZES = [16, 24, 32, 48, 64, 128, 256]

ACCENT = (74, 214, 149, 255)          # #4AD695
SURFACE = (22, 22, 22, 255)           # #161616


def render(size: int) -> Image.Image:
    # 4x supersample then LANCZOS downsample for crisp edges at small sizes.
    scale = 4
    canvas = size * scale
    img = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # Rounded square background.
    margin_ratio = 0.05 if size >= 32 else 0.0
    radius_ratio = 0.22
    m = int(canvas * margin_ratio)
    r = int(canvas * radius_ratio)
    d.rounded_rectangle(
        [(m, m), (canvas - m, canvas - m)],
        radius=r, fill=SURFACE,
    )

    # Subtle inner shadow for depth.
    shadow_h = max(1, canvas // 8)
    shadow = Image.new("RGBA", (canvas, shadow_h), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    for y in range(shadow_h):
        alpha = int(40 * (y / shadow_h))
        sd.line([(0, y), (canvas, y)], fill=(0, 0, 0, alpha))
    img.alpha_composite(shadow, (0, canvas - shadow_h))

    cx, cy = canvas / 2, canvas / 2

    # Pill geometry — slightly different proportions at small sizes to keep the internal
    # waveform readable after downsample.
    tiny = size <= 24

    if tiny:
        pill_w = canvas * 0.50
        pill_h = canvas * 0.34
        bars_inside = [0.10, 0.18, 0.12]
        inside_bar_w = canvas * 0.045
        # Single trailing bar at small sizes; multiple would smudge.
        emission_heights = [0.18]
        emission_bar_w = canvas * 0.045
        emission_gap = canvas * 0.05
    else:
        pill_w = canvas * 0.46
        pill_h = canvas * 0.28
        bars_inside = [0.08, 0.14, 0.10, 0.16, 0.08]
        inside_bar_w = canvas * 0.028
        emission_heights = [0.18, 0.14, 0.10]
        emission_bar_w = canvas * 0.028
        emission_gap = canvas * 0.035

    # The pill is shifted left so the emission bars have room on the right.
    pill_cx = cx - canvas * 0.10
    d.rounded_rectangle(
        [(pill_cx - pill_w / 2, cy - pill_h / 2),
         (pill_cx + pill_w / 2, cy + pill_h / 2)],
        radius=pill_h / 2, fill=ACCENT,
    )

    # Internal waveform bars (surface colour) inside the pill.
    spacing_inside = inside_bar_w * 0.6
    total_inside_w = len(bars_inside) * inside_bar_w + (len(bars_inside) - 1) * spacing_inside
    start_inside = pill_cx - total_inside_w / 2
    for i, h in enumerate(bars_inside):
        bx = start_inside + i * (inside_bar_w + spacing_inside)
        hpx = canvas * h
        d.rounded_rectangle(
            [(bx, cy - hpx / 2), (bx + inside_bar_w, cy + hpx / 2)],
            radius=inside_bar_w / 2, fill=SURFACE,
        )

    # Emission bars on the right — accent colour, shrinking outward.
    first_x = pill_cx + pill_w / 2 + emission_gap
    for i, h in enumerate(emission_heights):
        bx = first_x + i * (emission_bar_w + emission_gap)
        hpx = canvas * h
        d.rounded_rectangle(
            [(bx, cy - hpx / 2), (bx + emission_bar_w, cy + hpx / 2)],
            radius=emission_bar_w / 2, fill=ACCENT,
        )

    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    sizes_desc = sorted(SIZES, reverse=True)
    images = [render(s) for s in sizes_desc]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    images[0].save(
        OUT, format="ICO",
        sizes=[(s, s) for s in sizes_desc],
        append_images=images[1:],
    )
    print(f"Wrote {OUT} ({len(SIZES)} sizes)")


if __name__ == "__main__":
    main()
