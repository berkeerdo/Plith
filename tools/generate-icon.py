"""
Generates the Plith application icon — Concept D (Stacked overlay).

Design intent: a small accent-green pill floats above a wider muted pill,
on a dark rounded square. The literal Plith metaphor — an OSD layer above
whatever is underneath. Strong silhouette at 16x16.

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
MUTED = (90, 90, 90, 255)             # #5A5A5A
WHITE = (245, 245, 245, 255)


def render(size: int) -> Image.Image:
    # Render at 4x supersample then downsample with LANCZOS for crisp edges.
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

    # Subtle inner shadow at the bottom for depth.
    shadow_h = max(1, canvas // 8)
    shadow = Image.new("RGBA", (canvas, shadow_h), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    for y in range(shadow_h):
        alpha = int(40 * (y / shadow_h))
        sd.line([(0, y), (canvas, y)], fill=(0, 0, 0, alpha))
    img.alpha_composite(shadow, (0, canvas - shadow_h))

    cx, cy = canvas / 2, canvas / 2

    # Tiny / small sizes: skip the indicator dot, slightly thicker pills so the two-layer
    # silhouette still reads after downsample.
    if size <= 24:
        pill_h = canvas * 0.16
        top_w = canvas * 0.46
        bot_w = canvas * 0.66
        gap = canvas * 0.10
        draw_dot = False
    else:
        pill_h = canvas * 0.12
        top_w = canvas * 0.42
        bot_w = canvas * 0.64
        gap = canvas * 0.08
        draw_dot = True

    top_y0 = cy - gap / 2 - pill_h
    top_y1 = cy - gap / 2
    bot_y0 = cy + gap / 2
    bot_y1 = cy + gap / 2 + pill_h

    # Bottom layer (the "screen content" beneath)
    d.rounded_rectangle(
        [(cx - bot_w / 2, bot_y0), (cx + bot_w / 2, bot_y1)],
        radius=pill_h / 2, fill=MUTED,
    )

    # Top layer (the Plith OSD floating)
    d.rounded_rectangle(
        [(cx - top_w / 2, top_y0), (cx + top_w / 2, top_y1)],
        radius=pill_h / 2, fill=ACCENT,
    )

    # Tiny live-indicator dot to the right of the top pill — drops out at small sizes where
    # it would alias into a smudge.
    if draw_dot:
        dot_r = pill_h * 0.32
        dx = cx + top_w / 2 + pill_h * 0.42
        dy = (top_y0 + top_y1) / 2
        d.ellipse([(dx - dot_r, dy - dot_r), (dx + dot_r, dy + dot_r)], fill=WHITE)

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
