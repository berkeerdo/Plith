# Halka Island Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a fifth theme Halka: the collapsed island is the icon's glossy black source, the open island is the installer's lime fabric, and the open and collapse motions press and lift the three ripples.

**Architecture:** Halka is a theme pack (ADR 0022, plan 1 `theme-packs.md`): its manifest and rasters live in `crates/plith-core/themes/halka/`, are embedded by `plith-core::theme::builtin` as bytes, and are decoded by the app through `plith-media`. The `rings` motion signature's timeline lives in `plith-core::material`; the render thread gets a `ThemePaint` (open palette, collapsed palette, optional decoded material) instead of a bare `Palette`, and the painter draws the floor with a bitmap brush and the motion layers from `MaterialLayers` in `PaintFrame`.

**Plan 2 of theme worlds** (spec `docs/specs/2026-11-26-theme-worlds-design.md` sections 4, 5.3, 8, 11; plan 1 is done on this branch). Rewritten 2026-11-26 from the pre-pack version: Task 1 cuts into the pack folder, Task 2 makes Halka a pack, Task 4 adds the per-pixel contrast test. The branch `feature/theme-packs` merges to main after Task 6 Step 4 (Berke's look), asked first.

**Tech Stack:** Rust 2024, `windows` crate (Direct2D bitmap brush, layers), PowerShell + System.Drawing for the cutting script.

**Spec:** `docs/specs/2026-11-26-halka-island-design.md` (read it with this plan; it is binding).

## Global Constraints
- Fabric, source and ripples are raster, never drawn in code; code draws text, glyphs, meters and the reveal circle only.
- Art scale: 1 DIP = 3 px of the 1920 x 1264 sources; source centre (960, 100) px maps to the island's (width / 2, 33) DIP.
- The collapsed surface is never lime; idle looks use Gece's ink, muted, faint and lime flow on it.
- The open palette's flow is not lime (proposal `#000000`, filled marks only).
- Content keeps its own palette: the pill slot always in the collapsed palette, page slots always in the open palette. No palette cross-fade.
- Structure limits: 500 source, 80 per function, 800 test; `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`.
- Bar: 240 fps at 240 Hz, 0 to 1 hitches, console session only, never move the mouse from a script; builds and full suites through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- No em dash anywhere. Code and docs in English; theme name `HALKA` is a proper name in every language.

## Review Focus
1. Theme switched to or from Halka while the island is open: floor, pill and pages must change on the next frame with no stale fabric or stale black (Task 5 test `theme_switch_mid_open_replaces_floor`).
2. A collapse during an expand (and a reopen during a collapse) must continue from the current layer opacities, no jump (Task 3 tests `collapse_mid_expand_starts_from_current` and `reopen_replays_only_below_ten_percent`).
3. A page switch or resize while open must not replay the ripples (Task 3 test `page_switch_does_not_replay`).
4. Spring overshoot past the art size (651 x 441 DIP) must not show an edge: the fabric brush mirrors (Task 5 test `fabric_brush_extends_by_mirror`).
5. The Hidden look's 3 DIP sliver in Halka stays black glossy and the settings window in Halka shows the Gece palette (Task 2 test `halka_open_flow_black_pill_lime`, Task 6 render check).

---

### Task 1: Cut the Halka art

**Files:**
- Create: `docs/design/themes/halka/make-halka.ps1`, `docs/design/themes/halka/README.md`
- Create (script output): `floor-1x.png`, `floor-2x.png` (the fabric), `ring-1-1x.png` to `ring-3-2x.png`, `disc-1x.png`, `disc-2x.png`, `pill-1x.png`, `pill-2x.png` in `crates/plith-core/themes/halka/`

**Interfaces:**
- Produces: the files above; `README.md` records each file's pixel size, its DIP rectangle relative to the source centre, the fabric's mean colour and darkest weave colour, the **text zone** (the open page content rectangle of the tallest page, from `plith-core` layout, in DIP relative to the open island's top left) and the minimum and maximum WCAG relative luminance of the fabric inside it at 1x and 2x (all measured by the script), and the total size in bytes. If the darkest weave in the text zone fails ink 7:1 against `#141A05`, tone the fabric inside the zone in the script (spec 11: the art gives way, never the ink) and record how.

- [x] **Step 1:** Write `make-halka.ps1 -In docs/design/identity/r3/installer -Out <new folder>`, reusing the mask code of `make-states.ps1` (bounds 160 / 310 / 450, grow 30, feather 14). Outputs: fabric = whole `flat` scaled by 1/3 (1x) and 2/3 (2x); ring k = `p2` pixels with alpha = ring k mask, cropped to the ring's bounding box; disc = the source cut from `p2` with alpha (threshold on the black body plus its rim shadow), cropped; pill = `inst/button.png` scaled to 32 DIP high keeping its lower half, at 1x and 2x.
- [x] **Step 2:** Run it into a fresh folder, then copy into `crates/plith-core/themes/halka/`. Expected: every file exists; total under 3 MB (convert to smaller PNG settings if not, and note it).
- [x] **Step 3:** Composite check: fabric + ring 1..3 + disc at 1x equals `bg-3-1x.png` within 4 levels mean per channel over the ripple area (script prints the number). Record it in `README.md`.
- [x] **Step 4:** Commit `feat(design): cut the halka art`.

### Task 2: Halka as a pack

**Files:**
- Create: `crates/plith-core/themes/halka/theme.ini`
- Modify: `crates/plith-core/src/theme/builtin.rs` (embed manifest and files), `crates/plith-core/src/theme/pack.rs` and `pack/parse.rs` (two keys), `crates/plith-core/src/theme/registry.rs` (collapsed flow, `drawn`, material floor colour), their tests, `docs/specs/2026-11-26-theme-worlds-design.md` 4.2 (the two keys)

**Interfaces:**
- Consumes: Task 1 files and README numbers; plan 1's `Pack`, `Registry`, `ResolvedTheme`.
- Produces: `pub struct BuiltinPack { pub manifest: &'static str, pub files: &'static [(&'static str, &'static [u8])] }` and `pub const BUILTIN: [BuiltinPack; 2]` (Gece with no files, Halka with its 12 PNGs as `include_bytes!`, names without extension and scale, e.g. `("floor-1x", ..)`). Two manifest keys: `floor.mean=#RRGGBB` (required for `floor.kind=material`; `Palette.floor` and `floor_high` for material faces use it, replacing plan 1's `grey()` stand-in) and `collapsed.flow.color=#RRGGBB` (optional; when present the collapsed accent is exactly this colour and the user's accent does not apply to the pill; checked 3:1 on the collapsed face at load). `ResolvedTheme` gains `pub drawn: ThemeId` (the pack actually drawn: `gece` for an unknown id) and `pub material: bool`.
- Halka manifest: `id=halka`, `name=Halka`, blurbs in the 8 languages (tr "Limon kumaş, halka halka açılır", en "Lime fabric, opens ring by ring", de "Limettenstoff, öffnet sich Ring um Ring", es "Tela lima, se abre anillo a anillo", fr "Tissu citron vert, s'ouvre anneau par anneau", it "Tessuto lime, si apre anello dopo anello", pl "Limonkowa tkanina, otwiera się pierścień po pierścieniu", pt "Tecido lima, abre anel a anel"), `floor.kind=material`, `floor.mean`, `floor.lum_min`, `floor.lum_max` and `text_zone` from the Task 1 README, ink `#141A05`, muted `#3A4512`, faint `#4E5C1A`, track/faint track/border as `Palette::setup()`, `flow.color=#000000`, `flow.style=fill`, `flow.adjustable=true`, swatches `#000000,#141A05,#2B1F00,#0B2A4A,#3A0F2E`, collapsed face = Gece's colours, `collapsed.flow.color=#CAFF33`, `motion.open=rings`, `motion.overlays=ring-1,ring-2,ring-3,disc`, `dark=false`. Ruling (final review I1): the pill keeps Gece lime because the idle looks are Gece's on a black pill (halka spec); option (a), a lime rule in code, was rejected because it hides a pack decision in the registry.

- [x] **Step 1: failing tests:** in `pack/tests.rs`: `halka_builtin_parses` (id, signature `Rings`, overlays in order, `floor.mean` equals the README value); `material_needs_mean`; `collapsed_flow_checked_on_collapsed_face` (a pack with `collapsed.flow.color` failing 3:1 on its pill is refused). In `registry/tests.rs`: `halka_open_flow_black_pill_lime` (`open.flow` black, `collapsed.flow` `#CAFF33`, `collapsed.floor` Gece's, `sheet` Gece with lime station); `halka_user_accent_open_only` (`Colour(#0B2A4A)` changes `open.flow`, not `collapsed.flow`); `material_floor_uses_mean`; `drawn_is_gece_for_unknown`; `chroma_zero_retry_passes` (a pick that fails at every lightness with its own chroma but passes at `c = 0`: returns a grey, not `Fallback`; final review asked for it). In `builtin` tests: `builtin_ids_match_folders` (each `BUILTIN` manifest's id equals the folder its `include_str!` path names; final review M7) and `halka_files_present` (all 12 names present, each non-empty).
- [x] **Step 2:** Run `cargo test -p plith-core theme`. Expected: FAIL.
- [x] **Step 3:** Implement; write the manifest; add both keys to spec 4.2's table. The Look row lists GECE and HALKA from the registry (plan 1 code, no change).
- [x] **Step 4:** Run `cargo test -p plith-core`, then `node ~/.claude/bin/heavy-run.mjs -- cargo test --workspace` (the fit test now measures the row with two names). Expected: PASS. Commit `feat(core): halka as a theme pack`.

### Task 3: The material timeline in `plith-core::material`

**Files:**
- Create: `crates/plith-core/src/material.rs`, `crates/plith-core/src/material/tests.rs`; register in `crates/plith-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct MaterialLayers { pub reveal: f64 /* 0..1 of the far-corner radius */, pub ripples: [f32; 3], pub disc: f32, pub disc_lift: f64 /* DIP up */, pub pill: f32 }` with `MaterialLayers::COLLAPSED` (pill 1, rest 0) and `MaterialLayers::OPEN` (reveal 1, rest 0).
  - `pub fn morph_progress(height: f64, rest: f64, open: f64) -> f64` (clamped 0..1; 1 when `open <= rest`).
  - `pub struct MaterialMotion` with `new() -> Self` (at `COLLAPSED`), `expand(&mut self, now_ms: f64)`, `collapse(&mut self, now_ms: f64)`, `layers(&self, now_ms: f64, progress: f64) -> MaterialLayers`, `is_settled(&self, now_ms: f64, progress: f64) -> bool`.
  - Constants from spec section 5: reveal full at progress 0.55; ripple press starts `RIPPLE_START_MS: [f64; 3]` (25 / 40 / 55 % of `EXPAND_SETTLE_MS`, a constant set from the measured `brisk` expand full time in the latest console record), `PRESS_MS = 90.0`, `HOLD_MS = 120.0`, `FADE_MS = 180.0`; disc lift between progress 0.30 and 0.60 by 40 DIP; collapse ripple steps `RISE_STEP_MS = 60.0` in order 3, 2, 1; pill below progress 0.30; `REPLAY_BELOW = 0.10`.

- [ ] **Step 1: failing tests:** `layers_at_rest_collapsed_and_open`; `ripples_press_in_order_at_listed_times` (at `RIPPLE_START_MS[k] + PRESS_MS` ripple k is 1, ripple k+1 still 0); `ripples_gone_after_hold_and_fade`; `reveal_full_at_progress_055`; `disc_lift_between_030_and_060`; `collapse_mid_expand_starts_from_current` (layers right after `collapse` equal layers right before); `reopen_replays_only_below_ten_percent`; `page_switch_does_not_replay` (no `expand` call, progress jumping 1 to 0.8 to 1 leaves ripples at 0); `settled_only_when_ripples_done_and_progress_at_end`; `morph_progress_clamps`.
- [ ] **Step 2:** `cargo test -p plith-core material`. Expected: FAIL.
- [ ] **Step 3:** Implement. Opacity curves are linear ramps; the collapse sequence starts each ripple from its current value.
- [ ] **Step 4:** Tests PASS; `node scripts/structure-check.mjs` passes. Commit `feat(core): add the material timeline`.

### Task 4: `ThemePaint` from the app to the render thread

**Files:**
- Modify: `crates/plith-render/src/thread/relayout_handle.rs:25`, `crates/plith/src/app/state.rs:45-55`, `crates/plith/src/app/state/live.rs:178`, `crates/plith/src/app/state/live/tests.rs:253`, `crates/plith-render/src/thread/tests/harness.rs:138`, `crates/plith-render/src/gpu_paint.rs:96`, `crates/plith-render/src/offscreen.rs:130`
- Create: `crates/plith-core/src/material/art.rs` (pure types), `crates/plith/src/app/material_art.rs` (embedding + decode)

**Interfaces:**
- Produces: `plith_core::material::art::MaterialArt { fabric, rings: [Arc<ArtPixels>; 3], disc, pill: Arc<ArtPixels> }` (each `ArtPixels` with its own pixel size) and `ThemePaint { open: Palette, collapsed: Palette, material: Option<Arc<MaterialArt>> }` with `ThemePaint::plain(Palette)`, built from `ResolvedTheme` (`open`, `collapsed`) plus the art of `resolved.drawn` when `resolved.material`. `Relayouter::set_theme(&self, paint: ThemePaint)` replaces `set_palette` for the island; `plith::app::material_art::load(pack: &BuiltinPack, scale: f64) -> Result<Arc<MaterialArt>, String>` picks the `-1x` files below scale 1.5 and `-2x` from 1.5 (as `plith-setup/src/app/art.rs`), decoding with `plith_media::art::decode_image(bytes, u32::MAX)`; the bytes come from Task 2's `BuiltinPack.files`, never a second `include_bytes!`.
- Consumes: Task 1 files, Task 2 `BuiltinPack`, `ResolvedTheme.drawn` and `.material`.

- [ ] **Step 1: failing tests:** in `live/tests.rs`, `halka_sends_material_and_two_palettes` (the fake `Relayouter` records a `ThemePaint` with `material.is_some()`, `collapsed.floor` equal to Gece's floor and `collapsed.flow` lime); `gece_sends_plain_paint` (`material.is_none()`, open == collapsed). In `crates/plith-render/tests/pack_contrast.rs` (spec 5.3, WIC is a dev dependency there): `builtin_text_zones_pass_per_pixel` (for every built-in material pack, decode `floor-1x` and `floor-2x`, and for every pixel inside `text_zone` scaled to the file: ink >= 7, muted >= 4.5, faint >= 3, flow >= 3 unless `punch`, against that pixel's luminance; report the worst pixel and its ratio on failure); `lum_bounds_are_honest` (every text-zone pixel's luminance lies in `floor.lum_min..=floor.lum_max`).
- [ ] **Step 2:** `cargo test -p plith live` and `cargo test -p plith-render --test pack_contrast`. Expected: FAIL.
- [ ] **Step 3:** Implement; decode only when the theme has a material, once per scale, and keep the `Arc` across palette-only switches. A decode error logs `theme art: <pack id> <file>` and falls back to `ThemePaint::plain(Palette::default())`. If `pack_contrast` fails, fix the art in Task 1's script (spec 11), never the ink or the bar.
- [ ] **Step 4:** `cargo test -p plith` and `cargo test -p plith-render` PASS. Commit `feat(app): send the theme paint to the island`.

### Task 5: Paint the material

**Files:**
- Create: `crates/plith-render/src/paint/material.rs` (bitmaps, fabric brush, layer drawing)
- Modify: `crates/plith-render/src/paint.rs` (`Painter::set_theme(ThemePaint)` replaces `set_palette`; pill slot uses `collapsed`; floor via `material.rs` when present), `crates/plith-render/src/paint/island.rs` (nothing if the gradient path stays for plain themes)
- Modify: `PaintFrame` gains `pub material: MaterialLayers` (default `MaterialLayers::OPEN` for plain themes, ignored there)
- Test: `crates/plith-render/src/thread/tests/` (offscreen harness)

**Interfaces:**
- Consumes: `ThemePaint`, `MaterialLayers`, `MaterialArt`.
- Produces: draw order with a material: clear; fill shape with the pill bitmap brush (three-slice stretched, at `layers.pill`); push clip of the shape; fill the reveal circle (centre = source point, radius = `reveal` x distance to the far corner) with the fabric bitmap brush (`D2D1_EXTEND_MODE_MIRROR`, transform anchoring the source point); ring k bitmaps at `ripples[k]`; disc at `disc` shifted up by `disc_lift`; then the page and pill bitmaps as today; border.

- [ ] **Step 1: failing tests** (offscreen, read back pixels): `halka_open_floor_is_fabric` (a pixel at (20, 200) DIP of the open audio page within 12 levels of the Halka pack's `floor.mean`); `halka_collapsed_is_black` (centre pixel of the 200 x 32 island has luminance below 0.1); `fabric_brush_extends_by_mirror` (shape 651 x 441: the corner pixel is fabric, not transparent or clamped stripe: its colour within 12 levels of the mean); `theme_switch_mid_open_replaces_floor` (paint open Halka, `set_theme(plain Gece)`, paint: the (20, 200) pixel within 6 levels of Gece's floor); `plain_theme_paints_as_before` (Gece frame pixels equal a frame painted before this task, via the existing reference render used by the tests).
- [ ] **Step 2:** Run them via `node ~/.claude/bin/heavy-run.mjs -- cargo test -p plith-render`. Expected: FAIL.
- [ ] **Step 3:** Implement. Bitmaps are created once per `ThemePaint` change on the render thread; no allocation per frame.
- [ ] **Step 4:** Tests PASS; `paint.rs` stays under 500 lines. Commit `feat(render): paint the halka material`.

### Task 6: Drive the motion in the engine, log the theme, render the review set

**Files:**
- Modify: `crates/plith-render/src/thread/run/engine.rs` (own a `MaterialMotion`; `aim` calls `expand` / `collapse` when `scene.expanded` flips; `paint_frame` fills `PaintFrame.material` from `layers(now_ms, morph_progress(shape.height, REST.height, scene.target().height))`, using the last open height while collapsing; `is_settled` includes `MaterialMotion::is_settled`)
- Modify: `crates/plith-core/src/motion.rs` (`MotionReport` line ends with ` theme <id>`, the drawn pack id), `crates/plith-core/src/motion/tests.rs`
- Modify: `crates/plith-render/examples/render_pages.rs` (`--halka <new folder>`)

**Interfaces:**
- Consumes: Tasks 3 to 5.

- [ ] **Step 1: failing tests:** `motion_line_names_the_theme` (line ends with ` theme halka`); engine harness `halka_expand_is_not_settled_until_ripples_end` and `gece_settles_as_before` (same frame count as before for a fixed demo expand).
- [ ] **Step 2:** FAIL, then implement, then PASS (`heavy-run` for the render crate). Check any script under `scripts/` that parses `motion` lines still reads them.
- [ ] **Step 3:** `--halka <new folder>` writes, at 1x and 2x: collapsed with Karot, Clock and Playing; Hidden; audio, media and welcome open; the expand strip at progress 0 / 0.25 / 0.40 / 0.55 / 0.70 / 1.0 on the welcome page; the settings window in Halka. Run it into a fresh folder and inspect every PNG yourself before showing anyone.
- [ ] **Step 4:** Publish the set on Berke's private review page (same style as "Plith Kurulum Ekranları"). **Stop here until Berke has looked**; apply palette or layout changes he asks for, rerun Step 3.
- [ ] **Step 5:** Five checks green (`cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, full suite on the office VM per `server-ofis`, `node scripts/structure-check.mjs`, release build via `heavy-run`). Commit `feat(render): run the halka motion`.

### Task 7: Console measurement with Berke

**Files:**
- Create: `docs/quality/2026-11-XX-halka-motion.md`
- Modify: `docs/STATUS.md`, `docs/ROADMAP.md`, this plan (banner, move to `plans/done/`)

- [ ] **Step 1:** Berke at the PC; `query session` shows console, log line `remote false`.
- [ ] **Step 2:** Release build; `--theme halka` with the demo driver: expand and collapse audio, media, welcome ten times each; then the same with `--theme gece`. Live switch Halka to Gece and back while open.
- [ ] **Step 3:** Record every `motion` line's fps, hitches, worst gap, draw and present ms per theme in the quality file; 0 render wakes at rest afterwards.
- [ ] **Step 4:** If Halka misses the bar, apply the spec's cut order one step at a time (shorter ripple fade, no ripples, no reveal) and remeasure each; record each run.
- [ ] **Step 5:** Tune `EXPAND_SETTLE_MS` and the ripple times only from what Berke saw; rerun Step 2 once after tuning. Update STATUS (rewrite), ROADMAP, banner; commit `docs: halka measured`.
