"""
Unique 'P-Speaker fusion' concept — from a distance the silhouette reads
as a capital P (Plith), on closer inspection the bowl of the P is a
stylized speaker cone with a small sound-wave arc emerging from it.

The point of the fusion is that no existing audio app uses this shape:
P monograms are common, speaker icons are universal, but the two as a
single fused mark is Plith-specific.
"""
from __future__ import annotations
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SIZE = 512

ACCENT = (74, 214, 149, 255)
SURFACE = (22, 22, 22, 255)
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


def concept_fusion() -> Image.Image:
    img, d = base()
    cx, cy = CANVAS / 2, CANVAS / 2

    # Stem of the P — a thick vertical bar on the left. The same bar reads as
    # the body of the speaker when paired with the cone.
    stem_w = CANVAS * 0.13
    stem_x = cx - CANVAS * 0.16
    stem_top = cy - CANVAS * 0.30
    stem_bot = cy + CANVAS * 0.30
    d.rounded_rectangle(
        [(stem_x - stem_w / 2, stem_top), (stem_x + stem_w / 2, stem_bot)],
        radius=stem_w / 2, fill=ACCENT,
    )

    # Bowl of the P — replaces the usual D-curve with a TRAPEZOIDAL SPEAKER CONE
    # so the bowl literally IS a speaker cone. Anchored at the top of the stem.
    cone_neck_top = stem_top + CANVAS * 0.04
    cone_neck_bot = cy
    cone_mouth_x = stem_x + stem_w / 2 + CANVAS * 0.24
    cone_mouth_top = cone_neck_top - CANVAS * 0.08
    cone_mouth_bot = cone_neck_bot + CANVAS * 0.08
    cone_neck_x = stem_x + stem_w / 2 - CANVAS * 0.005
    d.polygon([
        (cone_neck_x, cone_neck_top),
        (cone_mouth_x, cone_mouth_top),
        (cone_mouth_x, cone_mouth_bot),
        (cone_neck_x, cone_neck_bot),
    ], fill=ACCENT)

    # A single small sound-wave arc emerging from the mouth of the cone —
    # confirms 'audio' at first glance without overpowering the P read.
    wave_r = CANVAS * 0.10
    wave_cx = cone_mouth_x + CANVAS * 0.06
    wave_cy = (cone_mouth_top + cone_mouth_bot) / 2
    bbox = [wave_cx - wave_r, wave_cy - wave_r,
            wave_cx + wave_r, wave_cy + wave_r]
    d.arc(bbox, start=-50, end=50, fill=ACCENT, width=int(CANVAS * 0.04))

    return img


def main() -> None:
    img = concept_fusion()
    p = ROOT / "logo-fusion.png"
    img.save(p)
    print(f"Wrote {p}")


if __name__ == "__main__":
    main()
