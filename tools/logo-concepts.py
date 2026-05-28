"""
Generate three logo concept PNGs for visual comparison.
Concept A: Equalizer bars (3 vertical, different heights, rounded ends)
Concept B: Fader on a horizontal track with a circular thumb
Concept C: Concentric sound waves / arcs emanating outward
"""
from __future__ import annotations
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SIZE = 512
SCALE = 1   # already large; LANCZOS at small sizes if needed

ACCENT = (74, 214, 149, 255)        # #4AD695
ACCENT_SOFT = (74, 214, 149, 160)
DARK_BG = (10, 10, 10, 255)         # #0A0A0A
SURFACE = (22, 22, 22, 255)         # #161616

CANVAS = SIZE
CORNER = int(CANVAS * 0.22)
MARGIN = int(CANVAS * 0.05)


def base_canvas(bg=DARK_BG) -> tuple[Image.Image, ImageDraw.ImageDraw]:
    img = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(
        [(MARGIN, MARGIN), (CANVAS - MARGIN, CANVAS - MARGIN)],
        radius=CORNER, fill=bg,
    )
    return img, d


def concept_a_equalizer() -> Image.Image:
    """Three vertical bars, accent green on dark surface — universal audio mark."""
    img, d = base_canvas(SURFACE)

    # Bar geometry: centred trio, varying heights so the silhouette reads as "audio"
    cx = CANVAS / 2
    bar_w = CANVAS * 0.10
    spacing = CANVAS * 0.08
    base_y = CANVAS * 0.74
    heights = [CANVAS * 0.32, CANVAS * 0.46, CANVAS * 0.22]
    offsets = [-(bar_w + spacing), 0, (bar_w + spacing)]

    for off, h in zip(offsets, heights):
        x0 = cx + off - bar_w / 2
        x1 = cx + off + bar_w / 2
        y0 = base_y - h
        y1 = base_y
        d.rounded_rectangle([(x0, y0), (x1, y1)], radius=bar_w / 2, fill=ACCENT)

    return img


def concept_b_fader() -> Image.Image:
    """Horizontal track with a circular thumb — visual of a volume fader."""
    img, d = base_canvas(SURFACE)

    cx, cy = CANVAS / 2, CANVAS / 2
    track_len = CANVAS * 0.62
    track_thick = CANVAS * 0.06
    x0 = cx - track_len / 2
    x1 = cx + track_len / 2

    # Inactive track
    d.rounded_rectangle(
        [(x0, cy - track_thick / 2), (x1, cy + track_thick / 2)],
        radius=track_thick / 2, fill=(80, 80, 80, 255),
    )
    # Filled portion (about 60% — feels live, not "muted")
    fill_x = x0 + track_len * 0.62
    d.rounded_rectangle(
        [(x0, cy - track_thick / 2), (fill_x, cy + track_thick / 2)],
        radius=track_thick / 2, fill=ACCENT,
    )
    # Thumb
    thumb_r = CANVAS * 0.075
    d.ellipse(
        [(fill_x - thumb_r, cy - thumb_r), (fill_x + thumb_r, cy + thumb_r)],
        fill=(245, 245, 245, 255),
        outline=ACCENT,
        width=int(CANVAS * 0.015),
    )
    return img


def concept_c_waves() -> Image.Image:
    """Concentric arcs emanating right — sound-emission abstraction."""
    img, d = base_canvas(SURFACE)

    cx, cy = CANVAS * 0.32, CANVAS / 2

    # Three arcs, opening to the right, each thicker than the last
    for i, (radius, thickness, color) in enumerate([
        (CANVAS * 0.18, int(CANVAS * 0.05), ACCENT),
        (CANVAS * 0.30, int(CANVAS * 0.05), ACCENT),
        (CANVAS * 0.42, int(CANVAS * 0.05), ACCENT_SOFT),
    ]):
        bbox = [cx - radius, cy - radius, cx + radius, cy + radius]
        d.arc(bbox, start=-50, end=50, fill=color, width=thickness)

    # Source dot
    r = CANVAS * 0.045
    d.ellipse([(cx - r, cy - r), (cx + r, cy + r)], fill=(245, 245, 245, 255))

    return img


def main() -> None:
    for name, fn in [("a-equalizer", concept_a_equalizer),
                     ("b-fader", concept_b_fader),
                     ("c-waves", concept_c_waves)]:
        img = fn()
        p = ROOT / f"logo-{name}.png"
        img.save(p)
        print(f"Wrote {p}")


if __name__ == "__main__":
    main()
