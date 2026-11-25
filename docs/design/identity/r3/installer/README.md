# Installer A Halka: material assets

Date: 2026-11-25. Spec: `specs/2026-11-24-identity-r3-design.md`. Review page: Berke's private
page "Plith Kurulum Ekranları" (the three screens at true size with code text).

| File | What |
|---|---|
| `bg-0-*.png` | Ripples flat (start screen) |
| `bg-1-*.png` | Ripple 1 pressed (files done) |
| `bg-2-*.png` | Ripples 1 and 2 pressed (helper trusted) |
| `bg-3-*.png` | All pressed (done; the window is the icon) |
| `button-*.png` | Glossy black pill, transparent, no text (label drawn by code in lime) |
| `inst/p2.png`, `inst/flat.png`, `inst/button.png` | The image-model sources |
| `make-states.ps1` | Builds the four states and the button sizes from the sources |

Sizes: 1x is 640 x 420 (button 200 wide), 2x is 1280 x 840 (button 400 wide). Other scales are
drawn from the 2x files.

## How they were made
- `gpt_image_2_6` through Higgsfield, quality high, 2k, 1920 x 1264, no text in the prompt.
- First set (icon master as reference, ripples down to 66 % of the height) was rejected on the
  page: "bittide yazılar gözükmüyor", the done text sat on the outer ripple. Second set (the
  first `p2` as reference, source 20 % of the width, ripples ending near 50 %) is the one kept;
  of its two passes the second was clean (the first had a faint seam at mid height).
- `flat`: an edit of `p2` with the ripples removed. Alignment measured: the source region differs
  by 3.1/255 per channel on average, the plain fabric by 3 to 5 levels.
- States 1 and 2 are not drawn: they are `flat` with `p2` blended in where `flat` is clearly
  brighter (the groove shadow), sorted into rings by the distance from (960, 100) with bounds
  160, 310, 450 px (`-Bounds`, measured from the groove minima at y 330, 480, 620), grown by
  30 px and feathered by 14 px. Bounds that sit past a groove let the next ring's shadow leak in.
- The button pass went with no reference image (with one the job failed), transparent
  background, cropped to its alpha bounds.
- Credits: about 25 for the whole round (12 comps, 4 + 4 background passes, 2 edits, 1 button).

## Layout used on the page (1x DIP, for the builder)
Ripples end near y 215. Version label 18, 14; close 16 from the right. Step labels right of each
ripple's end, one line each (`white-space: nowrap`): 1 at (430, 38), 2 at (468, 68), 3 at
(496, 108); step 2 is `YARDIMCI`, not `OSD YARDIMCISI`, so `İZİN` fits. Start: lead centred at
y 250, button 200 x 55 at (220, 298). Running: lead at y 262, sub at y 292. Done: title at y 236
(26 px), sub at y 274, button at (220, 304), `WINDOWS İLE BAŞLAT` at y 370, `KAPAT` at y 394.
