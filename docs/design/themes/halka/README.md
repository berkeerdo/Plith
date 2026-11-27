# Halka theme art

Date: 2026-11-26. Spec: `specs/2026-11-26-halka-island-design.md` (sections 3, 5, 6). Sources:
`design/identity/r3/installer/inst/{p2,flat,button}.png`. Script: `make-halka.ps1`
(`pwsh -File make-halka.ps1 -In docs/design/identity/r3/installer -Out <new folder>`, about 5
minutes, prints every number below). Output lives in `crates/plith-core/themes/halka/`.

Scale: 1 DIP = 3 source px (the installer's scale), so the 1x files match `bg-*-1x.png` (y is
scaled by 420 / 1264 like the `bg` files, which puts the source centre at 1x px (320, 33.23)).
2x files are exactly twice the 1x size. DIP rectangles below are relative to the source centre
(source px (960, 100)), x right, y down; on the island the centre sits at (width / 2, 33).

| File | Pixels 1x / 2x | DIP rect (x, y, w, h) | What |
|---|---|---|---|
| `floor-1x/2x.png` | 640x420 / 1280x840 | (-320, -33.23, 640, 420) | The fabric: `flat` with the source disc cloned out, 8 bit palette PNG |
| `ring-1-1x/2x.png` | 217x107 / 434x214 | (-109, -8.23, 217, 107) | Ripple 1 pixels of `p2`, alpha = ring mask |
| `ring-2-1x/2x.png` | 296x125 / 592x250 | (-149, 21.77, 296, 125) | Ripple 2 |
| `ring-3-1x/2x.png` | 352x130 / 704x260 | (-177, 61.77, 352, 130) | Ripple 3 |
| `disc-1x/2x.png` | 137x74 / 274x148 | (-69, -33.23, 137, 74) | The glossy black source, alpha (black body plus soft rim shadow) |
| `pill-1x/2x.png` | 244x32 / 488x64 | own size | `button.png`, lower half of the body, 32 DIP high, top edge flat (three-slice it) |

Ring masks are the code of `make-states.ps1` unchanged (bounds 160 / 310 / 450 px, grow 30,
feather 14). Ring and disc files are straight alpha, cropped to alpha above 2/255 at 1x; the 2x
crop is the 1x box doubled.

## Two decisions the sources forced
- **The floor has no source disc.** `flat` still contains the glossy source, but at rest the
  source is lifted out of the shape (spec 5), so the script clones the fabric from a horizontal
  shift (-578 px, picked by the best weave match around the patch) over the disc and its shadow
  and feathers the edge. The disc comes back only as `disc-*`, from `p2`.
- **The floor is a 256 colour palette PNG.** The weave is noise: 24 bit PNGs of it were 546 KB
  (1x) and 2.33 MB (2x), 4.4 MB for the set against the 3 MB target. The median cut palette (no
  dithering) costs a mean 1.30 / 1.37 levels per channel (1x / 2x) and brings the floors to 146 KB
  and 608 KB. Rings, disc and pill keep 24 bit alpha.

## Measured (script output, 2026-11-26)
- Fabric mean colour (1x floor, whole image): `#CCFA0D`.
- Darkest weave in the text zone: `#ADEE03` at 1x, `#8BDE00` at 2x (the 2x floor keeps more of
  the weave's dark threads). Brightest: `#E9FE2B` (1x), `#F6FF83` (2x).
- **Text zone** (the open page content of the tallest page, DIP relative to the open island's top
  left): x 30 to 596, y 12 to 406 (566 x 394). From `plith-core`: `shapes::PAGE_MAX_HEIGHT` (420,
  the audio page's cap in `pages/audio/layout.rs`: `height = (row_y + BOTTOM).min(PAGE_MAX_HEIGHT)`),
  `pages::PAGE_W` 620 and `MARGIN_LEFT` 30 (left edge), `audio/layout.rs` `VALUE_RIGHT = PAGE_W -
  24` (right edge, 596), `pages/tabbar.rs` `LABEL_Y = 12` (top: the tab labels), and the footer
  line at `height - FOOTER_UP - 10` = 394 plus its 12 DIP label height (bottom, 406). On the
  fabric the zone is shifted by 10 DIP in x (the source centre at 320 DIP of 640 lands at the
  island's 310 of 620): fabric px (40, 12) to (606, 406) at 1x, (80, 24) to (1212, 812) at 2x.
- WCAG relative luminance of the floor inside the zone: 1x min 0.7004, max 0.8838, mean 0.8142;
  2x min 0.5773, max 0.9275, mean 0.8166. Ink `#141A05` has L 0.0090, so the worst contrast is
  (0.5773 + 0.05) / (0.0090 + 0.05) = **10.64:1 at 2x** (12.72:1 at 1x). The 7:1 bar needs a
  fabric luminance of at least 0.363, so no toning of the fabric was needed (spec 11 clause not
  triggered). Muted ink `#3A4512` and faint `#4E5C1A` are not covered by this task.
- Composite check: floor + ring 1..3 + disc at 1x against `bg-3-1x.png`, mean absolute
  difference per channel over the ripple area (50,446 px where any ring or disc alpha is above
  0.02): **1.20 levels** (limit 4); over the whole image 1.44. The script writes
  `composite-check-1x.png` into the output folder for a look; it is not shipped.
- Sizes in bytes: disc 15,626 / 52,209; floor 146,310 / 607,834; pill 4,847 / 17,613; ring-1
  55,890 / 213,830; ring-2 94,014 / 368,436; ring-3 115,457 / 458,924. **Total 2,150,990**
  (under 3 MB).

## Known limits
- The floor is 640x420 DIP (the installer window). The open island is up to 620x420 and the
  spring overshoots by 1.05 (`shapes::OVERSHOOT`), so at the largest page the overshoot is
  about 11 DIP wider and 21 DIP taller than the floor: the consumer must clamp or clip the floor
  there. The 1920x1264 source cannot be cropped any larger, and the design says never scale up.
- The pill's lower half is 244 DIP wide, wider than the 200 DIP collapsed island; it is meant to
  be three-sliced, so only its end caps and a straight stretch are used.

> Note (2026-11-27): the pill raster (`pill-1x/2x.png`, three-sliced) was dropped. The collapsed pill is
> the collapsed palette's gradient and border on the island's own shape, because the raster's corner did
> not match the shape's 16 DIP radius. `make-halka.ps1` still lists the pill; do not ship its output.
