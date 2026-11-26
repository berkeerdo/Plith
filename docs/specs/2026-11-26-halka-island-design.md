# Halka: the installer's material and motion in the island

Date: 2026-11-26. Status: design, approach picked by me under Berke's "önerilerinle devam edelim"
(2026-11-26); he can veto any point. Asked by Berke 2026-11-25 after running the setup: "o temayı
temaları animasyonları normal notch'a island'a koyalım". Paths are under `docs/` unless they
start with `crates/`.

Inputs: identity r3 spec `specs/2026-11-24-identity-r3-design.md` (direction A Halka), installer
material record `design/identity/r3/installer/README.md` (sources `inst/p2.png`, `inst/flat.png`,
`inst/button.png`, `make-states.ps1`), ADR 0007 (themes are palettes), ADR 0017 and 0020 (where
lime may appear), theme switch spec `specs/2026-11-24-theme-switch-design.md`.

## 1. Intent
Berke ran the setup and liked its world: the lime speaker fabric, the glossy black source in the
top edge, the three ripples pressed into the fabric. The island should be able to live in that
world too. The installer itself has **no motion** today (its screens swap at once), so the motion
here is new work, built on the installer's idea: a step is a ripple pressed into the fabric.

## 2. Goals, non-goals, anti-goals
Goals
- A fifth theme **Halka**, picked like the other four (settings Görünüm, `theme=halka`,
  `--theme halka`). The four existing themes do not change.
- **Collapsed:** the island is the source: glossy black, raster, flush with the top edge. The
  idle looks (Karot, Clock, Playing, Ses, Plain, Hidden) draw on it in a light palette.
- **Open:** the floor is the lime fabric, raster, at every page size. Pages draw on it in a dark
  palette.
- **Open motion:** with the island's existing spring morph, the fabric spreads out of the source,
  the three ripples press in from the inside out, one after another, then fade back to flat
  fabric while the page content comes in. The source lifts into the top edge as the fabric
  arrives, because the tab row sits where it would be (`pages/tabbar.rs`, y 0 to 34).
- **Collapse motion:** the reverse, shorter: the ripples rise, the fabric draws back into the
  source.
- Measured on the console against the bar in `CLAUDE.md`: 240 fps at 240 Hz, 0 to 1 hitches,
  for expand and collapse in Halka, and Gece measured in the same session for comparison.

Non-goals
- Changing the installer (it stays lime with instant screens; adding its press motion is a
  separate later job).
- The OSD helper (keeps its fixed colours, as for every theme).
- The settings window in Halka: it shows the Gece palette with the selected station in lime
  (r3 anti-goal: no lime in settings beyond the icon and the station).
- New idle looks, new pages, the Hidden hover strip (separate idea in STATUS).
- A ripple pulse on volume or media events (possible later, once the open motion is measured).

Anti-goals
- Fabric, source or ripples drawn in code (round 2 rejection: "kumaş deseni gibi durmuyor").
  Code draws text, glyphs, meters and the reveal mask only.
- A lime floor while collapsed. The collapsed surface stays black during games; only the small
  flow marks an idle look already draws (as in Gece today) may be lime.
- A motion that costs the bar: if Halka's expand cannot hold 240 fps with 0 to 1 hitches, the
  motion is cut down (fewer layers, shorter fade) before it ships, never shipped slower.
- Text baked into rasters.
- Ripples behind page content at rest (they hurt reading; they are motion only).

## 3. Constraints
- ADR 0007 says a theme is colours only. Halka adds raster layers and two palettes (collapsed
  and open). This needs **ADR 0022**: "a theme may carry a material: raster floor layers and a
  collapsed palette", Halka the only one. ADR 0017 / 0020's lime rule: in Halka the lime is the
  material, so the **flow colour is not lime** there (see section 5).
- Contrast bar of ADR 0007 applies to both Halka palettes, checked by the existing test against
  the fabric's measured mean colour (and its darkest weave level for text).
- The render thread owns every graphics object; the art is decoded once per scale (1x for
  scale < 1.5, 2x from 1.5, like `crates/plith-setup/src/app/art.rs`) and kept as bitmaps; a
  theme switch to or from Halka creates or drops them, no new swap chain.
- Island sizes: collapsed 200 x 32 (radius 16, square top), pages up to 620 x 420
  (`plith-core/src/shapes.rs`). The fabric raster must cover 620 x 420 plus the 1.05 overshoot
  at 2x without stretching: crop from the 1920 x 1264 `flat` source, anchored top-centre at the
  source position, never scaled up.
- Hidden look: the 3 DIP sliver stays black glossy (the source cropped), same as collapsed.
- Pure logic (timeline of the motion, palette selection by morph progress, crop rectangles) in
  `plith-core` with unit tests; Windows and D2D code in `plith-render`. Structure limits apply.
- Assets must stay small in the exe: target under 3 MB for all Halka rasters at 1x and 2x
  (JPEG-quality PNG or WebP via WIC is the builder's call, measured).

## 4. Approaches
1. **Halka as a fifth theme, source when collapsed, fabric when open (picked).** Brings both the
   material and the step motion; the collapsed island stays black, so nothing lime glows at the
   top of the screen in a game. Costs a theme that is more than a palette (ADR 0022) and a
   floor that changes during the morph.
2. Lime all the time, collapsed too. Most faithful to the installer, but a lime bar over every
   game and video, and every idle look must read on lime at 32 DIP. Rejected.
3. Motion only: a ripple-press open motion in all four themes, drawn in each theme's flow colour,
   no raster. Cheapest and keeps ADR 0007, but drops the material Berke liked and needs code-drawn
   ripples, the kind of drawing rejected in round 2. Rejected.

## 5. Decided
- Name and key: `Halka` / `halka`, listed last (`GECE / KAĞIT / YORK / SODYUM / HALKA`), proper
  name in every language.
- **Scale and anchor:** the art is used at the installer's scale, 1 DIP = 3 px of the 1920 x
  1264 sources (so the island's 1x and 2x match the installer's `bg-*` files exactly). The source
  centre (960, 100) px maps to the island's top centre, (width / 2, 33) DIP. The ripples then
  reach about 150 DIP from it and end near y 215 DIP, which fits the 620 wide pages; on a page
  shorter than that they are clipped by the shape like everything else.
- Layers at rest in the open island, bottom to top: fabric (`flat` crop), page content. Motion
  layers between them only while a motion runs: ripple 1, 2, 3 overlays (alpha, cut from `p2`
  over `flat` with the ring masks of `make-states.ps1`, bounds 160 / 310 / 450 px from the source
  centre) and the source disc (cut from `p2`, alpha). At rest the source is fully lifted out of
  the shape and not drawn; the tab row has the fabric alone behind it.
- Collapsed surface: not a crop of the disc (the disc is about 128 x 70 DIP and would not read at
  200 x 32). It is the glossy pill material of `inst/button.png`, stretched horizontally by its
  straight middle (three-slice), its lower 32 DIP used so the top is square and flush with the
  screen edge. During expand the pill fades into the disc while the width grows. Idle looks use
  Gece's ink, muted and faint on it; their flow marks stay lime.
- **Content keeps its own palette, no palette cross-fade.** An idle look is always drawn in the
  collapsed palette and a page always in the open palette; the existing content handover of the
  expand (idle look out, page in) is unchanged. Only the floor changes during the morph (pill to
  disc to fabric reveal), so cached page bitmaps are recorded once per palette as today.
- **What drives the motion:** the reveal circle, the source lift and the pill-to-disc fade follow
  the morph's progress (height between its start and target, so they follow the spring and its
  retargets). The ripple press, hold and fade run on time since the expand began. Interrupts:
  a collapse during an expand starts the collapse sequence from the layers' current opacities (no
  jump); a reopen during a collapse follows the spring again and replays the ripples only if the
  reveal had shrunk below 10 %. A page switch or resize while open replays nothing: the fabric
  is only re-cropped by the shape.
- **Where the data lives:** `Theme::palette()` stays the open palette; new
  `Theme::collapsed_palette()` returns the same palette for the four existing themes and the
  collapsed one for Halka; new `Theme::material() -> Option<Material>` (`plith-core`, pure data:
  art ids, crop rectangles, timeline constants) is `Some` only for Halka. `Theme::ALL` grows to
  five and the tests and look cycling sized to four follow. The contrast test checks against
  constants for the fabric's mean and darkest weave colour, measured by the cutting script and
  written with their measurement date (`plith-core` has no image decoder).
- **If the bar fails**, cut in this order and remeasure each time: the ripple fade-out shortened,
  then ripples dropped (reveal only), then the reveal dropped (fabric appears with the content
  fade). Fabric without motion still ships as Halka: the material is the core of the request.
  The bar is this PC at 240 Hz; other refresh rates are not measured in this round.
- Open palette (proposal, final values set on the render review): ink near black olive
  `#141A05`, muted `#3A4512`, faint `#4E5C1A`, tracks as darker lime; **flow black `#000000`
  drawn as filled marks**, ink text never filled, so flow and ink differ by shape as well as
  value. If the review shows flow and ink cannot be told apart, the flow becomes the glossy
  black raster tone with the source's highlight (a 1 px light line on top), still not lime.
- Motion timeline (first proposal; its press times, hold and fade are superseded by the two
  dated picks at the end of this bullet): expand follows the existing spring; the
  fabric reveal is a circle growing from the source centre, reaching the far corner at 55 % of
  the morph's progress; ripples press at 25 / 40 / 55 % of the spring's typical settle time
  (converted to ms once, from the measured `brisk` expand), each a 90 ms press (fade in to the
  `p2` depth), held 120 ms, then all fade over 180 ms; the source lifts (slides up out of the
  shape) between 30 and 60 % progress; page content keeps its existing handover. Collapse:
  ripples rise in reverse order in 60 ms steps, the reveal circle shrinks into the source, the
  disc fades back into the pill below 30 % progress. Berke's pick on the render review
  (2026-11-26), replacing the time-based press: the ripples follow the morph's progress, ripple
  k pressing linearly over progress 0.15 / 0.30 / 0.45 plus 0.15 (all pressed at 0.60), latched
  so a dip in progress never un-presses or replays them, then (superseded by the next sentence)
  held 120 ms and faded together over 180 ms from the last press. Halka exception to "page content keeps its
  existing handover" (Berke, 2026-11-26): the ripples fade together right after the last press
  (`HOLD_MS` 0, `FADE_MS` 60, the collapse keeps its 180 ms as `RISE_MS`), the page comes in
  only once they are 0 (about 160 ms after Gece's in a brisk expand), the idle look leaves with
  the black pill on its own schedule (never above the pill's opacity), so no frame shows ripples
  with page text or the idle look over the fabric, and the review strip is sampled by time since
  the expand (0 / 50 / 100 / 130 / 230 / 400 ms, progress printed beside each); Gece and the
  plain themes keep the handover unchanged.
- `motion` log lines name the theme, so Halka and Gece runs read side by side.
- Assets are cut from the existing sources by a script next to `make-states.ps1`, no new
  image-model passes unless a cut fails (then about 10 credits, asked first).
- A render review page (Berke's private page) shows: collapsed in three idle looks, the audio,
  media and welcome pages open, and a frame strip of the expand (by time since 2026-11-26, see
  the timeline above), at 1x and 2x. Berke sees it before the timeline and palette are final.

## 6. Left to the builder
- The exact crop rectangles and how the ring overlays are cut and feathered, checked at 1x and
  2x side by side with the installer's `bg-*` files.
- How the reveal mask is drawn (geometry clip or opacity mask), picked by measured cost.
- How page bitmaps (cached per page) combine with a raster floor: drawn over the fabric each
  frame, or the floor baked into the cached bitmap; picked by measured present time.
- Asset format and compression inside the exe, and the cutting script's tool (PowerShell with
  System.Drawing like `make-states.ps1`, or Node), as long as it is rerunnable from the sources.
- Order of work (my call): assets, offscreen renders and the review page first; the live motion
  after Berke has looked at the page (palette and layout changes are cheap until then); the
  console measurement last, with Berke at the PC.

## 7. Done when
- Offscreen renders at 1x and 2x of the review set in section 5, seen by Berke on the page.
- Unit tests: `halka` key round trip, Halka in `Theme::ALL` last, both Halka palettes pass the
  contrast test, the timeline gives the right layer opacities at the listed points, crop
  rectangles stay inside the source at every page size.
- On the console with Berke present: switch to Halka live and back; expand and collapse each page
  ten times; `motion` lines for Halka and Gece meet the bar (240 fps, 0 to 1 hitches); 0 render
  wakes at rest afterwards.
- ADR 0022 written. Five checks green: `cargo fmt --check`, `cargo clippy --workspace -- -D
  warnings`, the test suite, `node scripts/structure-check.mjs`, a release build.
