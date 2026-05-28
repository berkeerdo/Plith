"""
Three genuinely-unique logo concepts — not equalizer, not letters, not waves.
The goal is a mark nobody else in the audio-app space is using.

Concept D: Stacked overlay layers — the OSD card metaphor literally drawn.
           A small accent-green pill floats above a wider muted pill, like a
           floating notification.

Concept E: Audio dial — a thick accent arc with an indicator tick, like a
           knob position read-off. Minimal, distinctive, geometric.

Concept F: Asymmetric level meter — vertical accent column with a small dot
           cursor at its centre + horizontal tick marks at top and bottom.
           Reads as a precision meter / fader scale.
"""
from __future__ import annotations
from pathlib import Path
import math
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SIZE = 512

ACCENT = (74, 214, 149, 255)
ACCENT_DIM = (74, 214, 149, 120)
DARK_BG = (10, 10, 10, 255)
SURFACE = (22, 22, 22, 255)
MUTED = (90, 90, 90, 255)
WHITE = (245, 245, 245, 255)

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


def concept_d_overlay() -> Image.Image:
    """Floating OSD card metaphor: small accent pill above a wider muted pill."""
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2

    # Bottom layer (background "screen")
    big_w = CANVAS * 0.62
    big_h = CANVAS * 0.10
    d.rounded_rectangle(
        [(cx - big_w / 2, cy + CANVAS * 0.06),
         (cx + big_w / 2, cy + CANVAS * 0.06 + big_h)],
        radius=big_h / 2, fill=MUTED,
    )

    # Top layer (Plith OSD floating)
    small_w = CANVAS * 0.40
    small_h = CANVAS * 0.10
    d.rounded_rectangle(
        [(cx - small_w / 2, cy - CANVAS * 0.18),
         (cx + small_w / 2, cy - CANVAS * 0.18 + small_h)],
        radius=small_h / 2, fill=ACCENT,
    )

    # Subtle connection dot (like an indicator that the top layer is "live")
    r = CANVAS * 0.025
    d.ellipse(
        [(cx + small_w / 2 + CANVAS * 0.04 - r, cy - CANVAS * 0.18 + small_h / 2 - r),
         (cx + small_w / 2 + CANVAS * 0.04 + r, cy - CANVAS * 0.18 + small_h / 2 + r)],
        fill=WHITE,
    )
    return img


def concept_e_dial() -> Image.Image:
    """Volume dial: thick accent arc on a circular track + a position tick."""
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS * 0.52

    outer_r = CANVAS * 0.32
    inner_r = CANVAS * 0.22

    # Background ring (full circle, muted)
    d.ellipse(
        [(cx - outer_r, cy - outer_r), (cx + outer_r, cy + outer_r)],
        outline=MUTED, width=int(CANVAS * 0.045),
    )

    # Accent arc — about 220 degrees, suggesting "near max volume"
    bbox = [cx - outer_r, cy - outer_r, cx + outer_r, cy + outer_r]
    d.arc(bbox, start=160, end=20, fill=ACCENT, width=int(CANVAS * 0.06))

    # Position tick at the end of the arc (small white circle)
    tick_angle = math.radians(20)
    tx = cx + outer_r * math.cos(tick_angle)
    ty = cy + outer_r * math.sin(tick_angle)
    tr = CANVAS * 0.035
    d.ellipse([(tx - tr, ty - tr), (tx + tr, ty + tr)], fill=WHITE,
              outline=ACCENT, width=int(CANVAS * 0.012))

    # Centre dot (knob axis)
    cr = CANVAS * 0.04
    d.ellipse([(cx - cr, cy - cr), (cx + cr, cy + cr)], fill=ACCENT)
    return img


def concept_f_meter() -> Image.Image:
    """Vertical precision level meter — accent column, tick marks, position dot."""
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2

    # Central accent column (the fill)
    col_w = CANVAS * 0.10
    col_top = CANVAS * 0.22
    col_bot = CANVAS * 0.78
    d.rounded_rectangle(
        [(cx - col_w / 2, col_top), (cx + col_w / 2, col_bot)],
        radius=col_w / 2, fill=ACCENT,
    )

    # Tick marks left and right at top, middle, bottom
    tick_w = CANVAS * 0.10
    tick_h = CANVAS * 0.015
    tick_offset = CANVAS * 0.16
    for ty in [col_top + tick_h / 2, (col_top + col_bot) / 2, col_bot - tick_h / 2]:
        # left
        d.rounded_rectangle(
            [(cx - col_w / 2 - tick_offset - tick_w, ty - tick_h / 2),
             (cx - col_w / 2 - tick_offset, ty + tick_h / 2)],
            radius=tick_h / 2, fill=MUTED,
        )
        # right
        d.rounded_rectangle(
            [(cx + col_w / 2 + tick_offset, ty - tick_h / 2),
             (cx + col_w / 2 + tick_offset + tick_w, ty + tick_h / 2)],
            radius=tick_h / 2, fill=MUTED,
        )

    # Position dot at ~75 percent
    pos_y = col_bot - (col_bot - col_top) * 0.75
    dot_r = CANVAS * 0.045
    d.ellipse(
        [(cx - dot_r, pos_y - dot_r), (cx + dot_r, pos_y + dot_r)],
        fill=WHITE, outline=ACCENT, width=int(CANVAS * 0.012),
    )
    return img


def main() -> None:
    for name, fn in [("d-overlay", concept_d_overlay),
                     ("e-dial", concept_e_dial),
                     ("f-meter", concept_f_meter)]:
        img = fn()
        p = ROOT / f"logo-{name}.png"
        img.save(p)
        print(f"Wrote {p}")


if __name__ == "__main__":
    main()
