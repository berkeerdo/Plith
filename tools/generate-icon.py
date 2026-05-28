"""
Generates the Plith application icon.

Design intent: accent-green rounded square + a dark 'P' monogram, using the same
color treatment the settings window's primary button does. Strong silhouette at
16x16, distinctive on both light and dark taskbars.

Run once after design changes:
    python tools/generate-icon.py

Output: src/Plith/Resources/icons/plith.ico (multi-resolution).
"""
from __future__ import annotations

from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "Plith" / "Resources" / "icons" / "plith.ico"
SIZES = [16, 24, 32, 48, 64, 128, 256]

ACCENT = (74, 214, 149, 255)        # #4AD695
ACCENT_DARK = (63, 191, 133, 255)   # #3FBF85, for the inner shadow
FOREGROUND = (10, 10, 10, 255)      # #0A0A0A, matches the Save button text


def find_font() -> str:
    candidates = [
        r"C:\Windows\Fonts\SegoeUIVF.ttf",   # Segoe UI Variable (Win11)
        r"C:\Windows\Fonts\segoeuib.ttf",    # Segoe UI Bold
        r"C:\Windows\Fonts\arialbd.ttf",     # Arial Bold fallback
        r"C:\Windows\Fonts\arial.ttf",
    ]
    for c in candidates:
        if Path(c).exists():
            return c
    raise RuntimeError("No suitable font found.")


def render(size: int, font_path: str) -> Image.Image:
    # Render at 4x supersample and downsample for smoother anti-aliasing at small sizes.
    scale = 4
    canvas_size = size * scale
    img = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded-square background. Margin shrinks at very small sizes so the mark fills
    # more of the bitmap (gives a stronger silhouette at 16x16).
    margin_ratio = 0.05 if size >= 32 else 0.0
    radius_ratio = 0.22
    m = int(canvas_size * margin_ratio)
    r = int(canvas_size * radius_ratio)
    draw.rounded_rectangle(
        [(m, m), (canvas_size - m, canvas_size - m)],
        radius=r,
        fill=ACCENT,
    )

    # Very subtle inner shadow at the bottom for a hint of depth.
    shadow_height = max(1, canvas_size // 8)
    shadow_band = Image.new("RGBA", (canvas_size, shadow_height), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow_band)
    for y in range(shadow_height):
        alpha = int(40 * (y / shadow_height))
        sd.line([(0, y), (canvas_size, y)], fill=(0, 0, 0, alpha))
    img.alpha_composite(shadow_band, (0, canvas_size - shadow_height))

    # Letter P. The Segoe UI 'P' is a clean geometric form that reads well even at tiny sizes.
    text = "P"
    font_size = int(canvas_size * 0.72)
    font = ImageFont.truetype(font_path, font_size)
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    tx = (canvas_size - tw) / 2 - bbox[0]
    # Lift the P slightly above true centre because the bowl visually centres higher than the
    # geometric centre due to the unbalanced descender area.
    ty = (canvas_size - th) / 2 - bbox[1] - canvas_size * 0.02
    draw.text((tx, ty), text, font=font, fill=FOREGROUND)

    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    font_path = find_font()
    # Render the largest size first; Pillow's ICO writer uses the seed image's dimensions
    # and resizes per the sizes= argument. Pre-rendering every size into append_images
    # makes Pillow honour the higher-quality per-size renders instead of just downsampling.
    sizes_desc = sorted(SIZES, reverse=True)
    images = [render(s, font_path) for s in sizes_desc]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    images[0].save(
        OUT,
        format="ICO",
        sizes=[(s, s) for s in sizes_desc],
        append_images=images[1:],
    )
    print(f"Wrote {OUT} ({len(SIZES)} sizes)")


if __name__ == "__main__":
    main()
