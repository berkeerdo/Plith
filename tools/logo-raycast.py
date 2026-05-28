"""
Raycast-tier logo concepts — iconic abstract marks, not lettering or
generic equalizer bars. The Raycast house style: a recognizable everyday
object rendered in minimal flat geometry, lives inside a rounded square.

Concept G: Volume knob — circle + thick directional indicator. Raycast's
           magnifier-with-handle parallel applied to audio.
Concept H: Stylized speaker — chunky trapezoid cone + two minimal sound
           waves curving outward. Reads as 'speaker' without being a stock icon.
Concept I: Bold geometric P — letter but rendered with Raycast's chunky
           custom-mark feel. Solid bowl + thick stem, no thin strokes.
"""
from __future__ import annotations
from pathlib import Path
import math
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SIZE = 512

ACCENT = (74, 214, 149, 255)
SURFACE = (22, 22, 22, 255)
WHITE = (245, 245, 245, 255)
MUTED = (90, 90, 90, 255)

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


def concept_g_knob() -> Image.Image:
    """Solid circle with a thick triangular wedge cut out — like a volume knob
    pointing to 'three quarters loud'. The wedge is the position indicator."""
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2
    r = CANVAS * 0.30

    # Outer ring (the knob body) — solid accent fill
    d.ellipse([(cx - r, cy - r), (cx + r, cy + r)], fill=ACCENT)

    # Indicator: thick straight bar from centre pointing up-right
    # representing the knob's current angle.
    angle = math.radians(-30)   # up-right, ~3/4 volume
    end_x = cx + r * 0.78 * math.cos(angle)
    end_y = cy + r * 0.78 * math.sin(angle)
    bar_w = CANVAS * 0.06
    # Draw as a thick rounded line using a polygon trick — line() doesn't take radius.
    perp_x = -math.sin(angle) * bar_w / 2
    perp_y = math.cos(angle) * bar_w / 2
    # Bar from a point near centre to the tip
    start_x = cx + r * 0.10 * math.cos(angle)
    start_y = cy + r * 0.10 * math.sin(angle)
    d.polygon([
        (start_x + perp_x, start_y + perp_y),
        (end_x + perp_x, end_y + perp_y),
        (end_x - perp_x, end_y - perp_y),
        (start_x - perp_x, start_y - perp_y),
    ], fill=SURFACE)
    # Rounded cap on the tip
    cap_r = bar_w / 2
    d.ellipse([(end_x - cap_r, end_y - cap_r), (end_x + cap_r, end_y + cap_r)], fill=SURFACE)
    d.ellipse([(start_x - cap_r, start_y - cap_r), (start_x + cap_r, start_y + cap_r)], fill=SURFACE)

    # Centre dot
    inner_r = r * 0.16
    d.ellipse([(cx - inner_r, cy - inner_r), (cx + inner_r, cy + inner_r)], fill=SURFACE)
    return img


def concept_h_speaker() -> Image.Image:
    """Chunky trapezoidal speaker cone + two simple sound-wave arcs."""
    img, d = base()
    cx, cy = CANVAS * 0.42, CANVAS / 2

    # Speaker body (cube on left + trapezoid cone on right)
    # Left cube
    cube_w = CANVAS * 0.10
    cube_h = CANVAS * 0.30
    d.rounded_rectangle(
        [(cx - CANVAS * 0.20, cy - cube_h / 2),
         (cx - CANVAS * 0.20 + cube_w, cy + cube_h / 2)],
        radius=CANVAS * 0.02, fill=ACCENT,
    )

    # Cone (trapezoid via polygon)
    cone_short = CANVAS * 0.04   # gap between cube and cone neck
    cone_neck = cy - cube_h * 0.5
    cone_neck2 = cy + cube_h * 0.5
    cone_mouth_top = cy - CANVAS * 0.22
    cone_mouth_bot = cy + CANVAS * 0.22
    nx = cx - CANVAS * 0.20 + cube_w + cone_short
    mx = cx + CANVAS * 0.04
    d.polygon([
        (nx, cone_neck),
        (mx, cone_mouth_top),
        (mx, cone_mouth_bot),
        (nx, cone_neck2),
    ], fill=ACCENT)

    # Two sound-wave arcs to the right of the speaker
    for i, (radius, thickness) in enumerate([
        (CANVAS * 0.18, int(CANVAS * 0.045)),
        (CANVAS * 0.30, int(CANVAS * 0.045)),
    ]):
        bbox = [cx + CANVAS * 0.10 - radius, cy - radius,
                cx + CANVAS * 0.10 + radius, cy + radius]
        d.arc(bbox, start=-45, end=45, fill=ACCENT, width=thickness)

    return img


def concept_i_bold_p() -> Image.Image:
    """Bold geometric 'P' — solid bowl + thick stem, Raycast custom-mark feel."""
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2

    # Stem of the P — a thick vertical bar on the left
    stem_w = CANVAS * 0.14
    stem_x = cx - CANVAS * 0.16
    stem_top = cy - CANVAS * 0.30
    stem_bot = cy + CANVAS * 0.30
    d.rounded_rectangle(
        [(stem_x - stem_w / 2, stem_top), (stem_x + stem_w / 2, stem_bot)],
        radius=stem_w / 2, fill=ACCENT,
    )

    # Bowl of the P — a thick D-shaped curve. We approximate via a circle and a
    # rectangle cut.
    bowl_r = CANVAS * 0.20
    bowl_cx = stem_x + bowl_r * 0.55
    bowl_cy = cy - CANVAS * 0.10
    # Filled circle for the bowl
    d.ellipse(
        [(bowl_cx - bowl_r, bowl_cy - bowl_r),
         (bowl_cx + bowl_r, bowl_cy + bowl_r)],
        fill=ACCENT,
    )
    # Punch a smaller circle out of the centre to hollow the bowl
    hole_r = bowl_r * 0.42
    d.ellipse(
        [(bowl_cx - hole_r, bowl_cy - hole_r),
         (bowl_cx + hole_r, bowl_cy + hole_r)],
        fill=SURFACE,
    )

    return img


def main() -> None:
    for name, fn in [("g-knob", concept_g_knob),
                     ("h-speaker", concept_h_speaker),
                     ("i-boldp", concept_i_bold_p)]:
        img = fn()
        p = ROOT / f"logo-{name}.png"
        img.save(p)
        print(f"Wrote {p}")


if __name__ == "__main__":
    main()
