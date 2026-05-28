"""Preview exporter: dump each baked size from the multi-resolution .ico as a PNG."""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
ICO = ROOT / "src" / "Plith" / "Resources" / "icons" / "plith.ico"
OUT = ROOT / "tools"

img = Image.open(ICO)
sizes = sorted(img.info.get("sizes", []), reverse=True)
print("Available sizes:", sizes)

for w, h in sizes:
    img.size = (w, h)
    img.load()
    p = OUT / f"preview-{w}.png"
    img.save(p)
    print(f"Saved {p}")
