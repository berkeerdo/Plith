# Identity comps, round 1 (Kol Sırası)

Date: 2026-11-24. Concept A "Kol Sırası" with C's one line per lever
(`../2026-11-24-directions.md`). HTML mocks at 1:1 DIP, rendered headless at 1x and 2x.
No engine code. Each page has a full-page PNG (`<page>-1x.png`, `-2x.png`) and one PNG per
window or variant.

## Files
| File | Shows |
|---|---|
| `icon-sheet.html` | Three icon constructions at 256, 48, 32, 24, 16 on light and dark Explorer, light and dark taskbar (24 px button, 16 px tray among neighbours), true 16 and 24 px rasters enlarged without smoothing. PNGs `icon-ekran-*`, `icon-plaka-*`, `icon-kesit-*` |
| `settings-gece.html` | OSD locked (13 normal, 14 locked), OSD free (13 pulled), Genel (keyboard focus on lever 2), Boşta top and scrolled to the end. PNGs `settings-gece-osd`, `-osd-serbest`, `-genel`, `-bosta`, `-bosta-sonu` |
| `settings-kagit.html` | OSD locked in Kağıt. PNG `settings-kagit-osd` |
| `installer-1/2/3.html` | Start, running (UAC asked on lever 3), done. PNGs `installer-N-window` |
| `kit.js`, `settings.js`, `installer.js`, `icons.js` | Shared drawing code: lever, plate, frame, caption; icon geometry per size grid |

## Decisions
- **Icon: recommend B Plaka.** The night tile is the screen, the island hangs from its top edge
  in ink, the lever has a lime handle. At 16 px it is the only one whose island and handle stay
  separate shapes on both bar colours. A Ekran loses its bezel on a dark bar; C Kesit's hatch
  is gone below 32 px and its top rule fuses with the island. Every size is drawn on its own
  pixel grid (16, 24, 32), not scaled.
- **Lever rule:** toggle normal = upright and hollow, pulled = leaning and filled ink. A choice
  lever is always filled and sits in a notch of its quadrant; the readable positions are on the
  plate (station square filled = set). Locked = faint, a hatched lock bar across the stem, the
  plate's foot says why (`KİLİTLİ: 13 ÇEKİLİ DEĞİL`), and a rod in the locking bed joins the two
  levers. A locking table (Loft's table of offsets) spells the rule out in words.
- **Numbering:** Genel 1-3, Görünüm 4, Boşta 5-12, OSD 13-14, Hakkında is the maker's plate
  (`LEVHA`, no levers). This keeps the concept's 13/14 interlock. Şehir became its own lever 11
  so it is the second real interlock (locked unless Konum is Şehir).
- **Sidebar = box diagram:** each section shows its levers in miniature with their real state,
  so the whole frame reads at a glance; a key at the foot explains normal, pulled, locked.
- **No lime in settings or installer.** The Ses look preview in Boşta draws its route in ink.
  Lime appears only in the icon handle.
- **Boşta scrolls.** Eight levers with plates do not fit 640 DIP; the content scrolls under the
  fixed sidebar with a thin overlay thumb. Other sections fit.
- **Installer:** levers 2 and 3 can be skipped (`ATLA`) before `KUR`; the running step names the
  UAC prompt and what a refusal means (Plith runs without the OSD).

## Open questions for Berke
1. Icon handle in lime: lime means "sound flows now" on the island. Keep it as the brand spot,
   or ink only?
2. Plaka on the dark taskbar needs its lighter edge to stay a plate (same lesson as the
   file-explorer icon). Paper version of B instead?
3. Boşta scrolling: accept, or split it into two sections (Ada 5-8, Hava 9-12)?
4. `PLITH’İ AÇ` keeps the brand as `PLITH`; ADR 0012's `tr_upper` would give `PLİTH`. Brand
   names stay invariant?
5. The quadrant ticks next to each lever are small at 1x; worth keeping, or let the lean alone
   carry the state?
6. Installer copy about the per-user install and the Program Files helper follows the R6 plan;
   confirm the final install paths before the builder writes them.
