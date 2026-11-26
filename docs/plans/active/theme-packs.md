# Theme packs (theme worlds plan 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fixed `Theme` enum with theme packs (a parsed `theme.ini` manifest), a colour solve for user accent and floor picks, the new settings keys and the migration of the retired themes, with Gece as the only built-in pack.

**Architecture:** All new logic is pure and lives in `plith-core::theme` (no Windows code, no new dependency). A pack parses into `Pack`; a `Registry` of packs plus the user's picks resolves to a `ResolvedTheme` (open, collapsed and settings-window palettes, `dark`, notes). The app holds the registry and sends resolved palettes down the existing palette path; the render thread does not change in this plan.

**Tech Stack:** Rust 2024, `plith-core` (zero dependencies), the existing `plith` app and `plith-render` palette path.

**Spec:** `docs/specs/2026-11-26-theme-worlds-design.md` (sections 4, 5.1, 5.2, 5.4, 7, 11), ADR 0022.

## Global Constraints

- Work on branch `feature/theme-packs` in a worktree (`superpowers:using-git-worktrees`). It merges to `main` together with plan 2 (Halka on packs), so `main` never shows a one-theme picker. Ask Berke before the merge.
- `plith-core` stays dependency-free and Windows-free. Hand-written parser; no toml/serde.
- Contrast bar: ink 7:1, muted 4.5:1, faint 3:1, flow 3:1 (WCAG, `theme::contrast`).
- Colours are `#RRGGBB` in files; `Palette` stays `[f32; 4]` straight sRGB.
- Structure (skill `praxvon-structure`, load before creating files): 500 lines source, 800 test, 80 per function; `foo.rs` + `foo/bar.rs`, never `mod.rs`; tests in `foo/tests.rs`.
- English code and comments; only user-facing strings Turkish (and the 8 languages); no em dash anywhere.
- Gece must look exactly as today: the resolved Gece palette with no picks equals today's `Theme::Gece.palette()` bit for bit.
- Builds and full test runs: `node ~/.claude/bin/heavy-run.mjs -- cargo test --workspace`; focused runs (`cargo test -p plith-core theme`) may run plain.
- Before merge: `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`, `node scripts/structure-check.mjs`.
- Commits: Conventional Commits, subject at most 50 chars, bullet body, no AI attribution. Commits on the feature branch are allowed per task; merge and push are asked.

## Review Focus

1. A hand-edited `settings.ini` with a bad `accent.<id>` (`#12345`, `red`, `accent.=#000000`, `accent.GECE=...`) must keep the default for that key, log key and length only, and leave every other line applied. Test in Task 5.
2. A user floor that leaves no passing ink (mid grey `#777777`) must still resolve to a palette that passes the bar or fall back to the pack's own floor, never draw failing text. Test in Task 2.
3. A settings file from a future build (`theme=firn` before Firn exists, `accent.firn=#1C5D99`) must round-trip untouched and draw Gece. Test in Task 5.
4. Old files with `theme=sodyum|york|kagit` must keep their colours, and a file that also holds `accent.gece` must not be overwritten by the migration (the user's own value wins). Test in Task 5.
5. A `theme.ini` with CRLF line ends, a BOM, trailing spaces or a duplicated key must parse the same as a clean one (duplicate: refused with a named reason). Test in Task 3.

---

### Task 1: Colour model (`Rgb`, OKLCH, range contrast)

**Files:**
- Create: `crates/plith-core/src/theme/colour.rs`, `crates/plith-core/src/theme/colour/tests.rs`
- Modify: `crates/plith-core/src/theme.rs` (add `pub mod colour;`)

**Interfaces:**
- Produces: `pub struct Rgb(pub [u8; 3])` (`Copy, Eq, Hash, Ord, Debug`) with `Rgb::parse(&str) -> Option<Rgb>` (exactly `#` + 6 hex digits, either case), `Rgb::to_hex(self) -> String` (upper case, `#RRGGBB`), `Rgb::rgba(self) -> [f32; 4]` (same as `hex(r,g,b)`), `Rgb::luminance(self) -> f64` (reuses the WCAG function in `theme.rs`).
- Produces: `pub struct Oklch { pub l: f64, pub c: f64, pub h: f64 }` with `Oklch::from_rgb(Rgb) -> Oklch` and `Oklch::to_rgb(self) -> Rgb` (gamut mapping: keep `l` and `h`, binary-search `c` down until the linear sRGB channels are inside 0..=1, then round).
- Produces: `pub enum Backdrop { Flat { lums: Vec<f64> }, Range { min: f64, max: f64 } }` and `pub fn worst_contrast(colour: Rgb, backdrop: &Backdrop) -> f64`: for `Flat`, the minimum WCAG ratio against each luminance; for `Range`, `1.0` when the colour's luminance lies inside `min..=max`, else the ratio against the nearer end.

- [x] **Step 1: failing tests** in `colour/tests.rs`: `parse_accepts_hex_only` (`#CAFF33`, `#caff33` ok; `CAFF33`, `#CAFF3`, `#CAFF333`, `#GGGGGG`, `` fail); `hex_round_trips` (`Rgb::parse("#caff33").unwrap().to_hex() == "#CAFF33"`); `rgba_matches_hex_fn` (`Rgb([0xCA,0xFF,0x33]).rgba() == hex(0xCA,0xFF,0x33)`); `oklch_round_trips_in_gamut` (for every `r,g,b` in steps of 17, `to_rgb(from_rgb(x))` differs by at most 1 per channel); `oklch_white_and_black` (`l` of `#FFFFFF` within 0.001 of 1, `#000000` within 0.001 of 0, both `c < 0.001`); `out_of_gamut_keeps_hue_and_lightness` (`Oklch{l:0.7,c:0.4,h:150}.to_rgb()` back to OKLCH has `|dl| < 0.01` and `|dh| < 2`); `range_inside_is_one` (`worst_contrast(Rgb([128,128,128]), &Range{min:0.1,max:0.5}) == 1.0`); `range_outside_uses_nearer_end` (black against `Range{min:0.2,max:0.9}` equals `contrast(black, lum 0.2)` i.e. `0.25/0.05 = 5.0` within 1e-9); `flat_takes_the_minimum`.
- [x] **Step 2:** Run `cargo test -p plith-core theme::colour`. Expected: FAIL (module missing).
- [x] **Step 3:** Implement in `colour.rs`. OKLab matrices from Björn Ottosson's reference (linear sRGB to LMS, cube root, LMS to Lab; inverse for `to_rgb`); hue in degrees 0..360.
- [x] **Step 4:** Run `cargo test -p plith-core theme::colour`. Expected: PASS.
- [x] **Step 5:** Commit `feat(core): rgb, oklch and range contrast`.

### Task 2: The solve (accent pick, plain floor ink)

**Files:**
- Create: `crates/plith-core/src/theme/solve.rs`, `crates/plith-core/src/theme/solve/tests.rs`
- Modify: `crates/plith-core/src/theme.rs` (`pub mod solve;`)

**Interfaces:**
- Consumes: Task 1 `Rgb`, `Oklch`, `Backdrop`, `worst_contrast`.
- Produces: `pub enum Shift { None, Lighter, Darker, Fallback }`; `pub struct Solved { pub colour: Rgb, pub shift: Shift }`.
- Produces: `pub fn solve_accent(pick: Rgb, backdrop: &Backdrop, fallback: Rgb) -> Solved`. Keeps `h` and `c` of the pick; walks `l` in steps of 0.005 outwards from the pick's `l` in both directions (each candidate through `to_rgb`), takes the nearest candidate with `worst_contrast >= 3.0` (tie: lighter). If none passes, retries with `c = 0`; if still none, returns `fallback` with `Shift::Fallback`. A pick that already passes returns unchanged with `Shift::None`.
- Produces: `pub struct PlainInk { pub floor: Rgb, pub floor_high: Rgb, pub ink: Rgb, pub muted: Rgb, pub faint: Rgb, pub track: Rgb, pub faint_track: Rgb, pub border: Rgb, pub dark: bool }` and `pub fn derive_plain(floor: Rgb) -> Option<PlainInk>`. `dark` = floor luminance below 0.179. `floor_high` = floor `l` +0.03 (dark) or -0.03 (light), same `h`, `c`. Ink roles are tints at the floor's `h` with `c = 0.012`; each role's `l` is the passing value (against `Flat` of both floor stops) nearest to its target: dark floor targets ink 0.94, muted 0.72, faint 0.62; light floor targets ink 0.20, muted 0.42, faint 0.52; thresholds 7.0, 4.5, 3.0. `track`, `faint_track`, `border` are the floor `l` moved away from the floor by 0.20, 0.09, 0.12 (builder may tune, spec section 12; they carry no contrast bar). Returns `None` when any text role cannot pass.

- [x] **Step 1: failing tests** in `solve/tests.rs`: `passing_pick_is_unchanged` (`#CAFF33` on `Flat` of Gece floors returns `#CAFF33`, `Shift::None`); `dark_pick_on_dark_floor_moves_lighter` (dark blue `#1C2A5A` on Gece floors gives `Shift::Lighter` and `worst_contrast >= 3.0`, hue within 3 degrees); `light_pick_on_light_floor_moves_darker` (`#FFE9A8` on `Flat [lum(#F3F2ED)]` gives `Shift::Darker`); `no_pass_falls_back` (any pick on `Range{min:0.0,max:1.0}` returns `fallback`, `Shift::Fallback`); `sweep_always_passes_or_falls_back` (hues 0..360 step 15, `l` 0.1..0.9 step 0.1, `c` 0.05 and 0.2, on Gece and on `#F3F2ED`: every result has `worst_contrast >= 3.0` or `Shift::Fallback`); `derive_plain_bar_holds` (for floors `#0B0B0A`, `#1C2420`, `#F3F2ED`, `#2A1A14`, `#D1DDEC`: ink >= 7, muted >= 4.5, faint >= 3 against both stops); `derive_plain_dark_flag` (`#1C2420` dark, `#F3F2ED` light); `mid_grey_floor` (`derive_plain(#777777)` is either `None` or passes the bar: assert whichever holds and that it never returns a failing palette).
- [x] **Step 2:** Run `cargo test -p plith-core theme::solve`. Expected: FAIL.
- [x] **Step 3:** Implement `solve.rs` per the Interfaces block.
- [x] **Step 4:** Run `cargo test -p plith-core theme::solve`. Expected: PASS.
- [x] **Step 5:** Commit `feat(core): accent and plain floor solve`.

### Task 3: Pack manifest parser and the Gece pack

**Files:**
- Create: `crates/plith-core/src/theme/pack.rs` (types + validation), `crates/plith-core/src/theme/pack/parse.rs` (line reader and key table), `crates/plith-core/src/theme/pack/tests.rs`
- Create: `crates/plith-core/themes/gece/theme.ini`, `crates/plith-core/src/theme/builtin.rs` (built-in packs live here, one folder per pack; plan 2 adds rasters beside the manifest. Spec 4.1 named `crates/plith/themes/`; moved to `plith-core/themes/` so core tests read the real files, update spec 4.1 in the same commit)
- Modify: `crates/plith-core/src/theme.rs` (`pub mod pack; pub mod builtin;`), `docs/specs/2026-11-26-theme-worlds-design.md` 4.1

**Interfaces:**
- Consumes: Task 1 `Rgb`, `Backdrop`, `worst_contrast`.
- Produces: `pub struct InkSet { pub ink: Rgb, pub muted: Rgb, pub faint: Rgb, pub track: Rgb, pub faint_track: Rgb, pub border: Rgb }`; `pub enum Floor { Colour { top: Rgb, high: Rgb }, Material { lum_min: f64, lum_max: f64 } }`; `pub enum FlowStyle { Fill, Punch }`; `pub enum Signature { Rings, Layers, Tension, Feed, None }`; `pub struct Face { pub floor: Floor, pub ink: InkSet }`; `pub struct Pack { pub id: String, pub name: String, pub blurbs: Vec<(String, String)>, pub open: Face, pub collapsed: Face, pub flow: Rgb, pub flow_style: FlowStyle, pub flow_adjustable: bool, pub floor_adjustable: bool, pub swatches: Vec<Rgb>, pub text_zone: Option<[f32; 4]>, pub signature: Signature, pub overlays: Vec<String>, pub dark: bool }` with `Pack::backdrop(&self, face: &Face) -> Backdrop` and `Pack::blurb(&self, lang_code: &str) -> &str` (falls back to `en`).
- Produces: `pub fn parse_pack(text: &str) -> Result<Pack, PackError>`; `pub struct PackError { pub key: String, pub reason: &'static str }` (`Display`: "`<key>`: <reason>").
- Produces: `pub const BUILTIN: [&str; 1]` in `builtin.rs` = `[include_str!("../../themes/gece/theme.ini")]`.
- Rules: line reader as `settings::parse` (BOM, `#`, CRLF, trim). Keys from the spec 4.2 table; `collapsed.floor.*` and `collapsed.ink.*` default to the open face when absent. Required: `format=1`, `id` (`[a-z0-9-]{1,32}`), `name`, `blurb.en`, `floor.kind`, the floor keys of its kind, all six `ink.*`, `flow.color`. Refusals (`reason` text): unknown format, unknown key, duplicate key, bad colour, bad number, bad list, missing required key, `material.tintable=true` ("tinting is not built"), `floor.adjustable=true` on a material floor, more than 6 swatches, `lum_min > lum_max`, and contrast: ink < 7, muted < 4.5, faint < 3, flow < 3 (skipped for `flow.style=punch`), any swatch < 3, each checked with `worst_contrast` on both faces. `dark` when absent: floor luminance (top stop or `lum_max`) below 0.179.
- Gece manifest values are exactly today's `Theme::Gece` palette (`#0B0B0A`, `#151412`, border `#2B2824`, ink `#ECE9E2`, muted `#A39E94`, faint `#8A847A`, track `#4A463F`, faint track `#2E2B27`, flow `#CAFF33`), `flow.adjustable=true`, `floor.adjustable=true`, `motion.open=none`, swatches `#CAFF33,#FFB224,#FFE9A8,#7FD4FF,#FF7A90,#B9A4FF` (each must pass; replace any that fails with the nearest passing one from `solve_accent` and note it in the commit body), blurbs in all 8 languages: tr "Sade siyah, malzemesiz", en "Plain black, no material", de "Schlichtes Schwarz, ohne Material", es "Negro sencillo, sin material", fr "Noir sobre, sans matière", it "Nero semplice, senza materiale", pl "Zwykła czerń, bez materiału", pt "Preto simples, sem material".

- [x] **Step 1: failing tests** in `pack/tests.rs`: `gece_builtin_parses` (`parse_pack(BUILTIN[0])` ok, `id == "gece"`, ink and floor equal the values above); `crlf_bom_spaces_parse_same` (Gece text with `\r\n`, a leading BOM and trailing spaces equals the clean parse); `duplicate_key_refused`; `unknown_key_refused`; `missing_flow_refused`; `bad_colour_refused` (`flow.color=#CAFF3`); `tintable_refused`; `material_floor_not_adjustable`; `low_contrast_refused` (`ink.faint=#3A3733` on Gece gives reason containing "contrast" and key `ink.faint`); `failing_swatch_refused`; `punch_skips_flow_check` (a Şerit-like light manifest with `flow.style=punch` and `flow.color` equal to `ink.ink` parses); `material_manifest_parses` (an inline Halka-like manifest: `floor.kind=material`, `floor.lum_min=0.78`, `floor.lum_max=0.93`, ink `#141A05`, muted `#3A4512`, faint `#4E5C1A`, flow `#000000`, `text_zone=24,40,572,180`, `motion.open=rings`, `motion.overlays=ring-1,ring-2,ring-3`, collapsed face Gece colours); `blurb_falls_back_to_en`; `collapsed_defaults_to_open`.
- [x] **Step 2:** Run `cargo test -p plith-core theme::pack`. Expected: FAIL.
- [x] **Step 3:** Implement `parse.rs` (line reader to an ordered key list, duplicate detection) and `pack.rs` (typed build and validation). Write `themes/gece/theme.ini`. Update spec 4.1 path.
- [x] **Step 4:** Run `cargo test -p plith-core theme::pack`. Expected: PASS.
- [x] **Step 5:** Commit `feat(core): theme pack manifest and gece pack`.

### Task 4: Registry and resolve

**Files:**
- Create: `crates/plith-core/src/theme/registry.rs`, `crates/plith-core/src/theme/registry/tests.rs`
- Modify: `crates/plith-core/src/theme.rs` (`pub mod registry;`, keep `Palette`, `hex`, `contrast`, `Palette::setup`, `Palette::resolve`; the `Theme` enum stays until Task 6)

**Interfaces:**
- Consumes: Tasks 1 to 3.
- Produces: `pub struct ThemeId(String)` (`Clone, Eq, Ord, Hash, Debug`) with `ThemeId::parse(&str) -> Option<ThemeId>` (`[a-z0-9-]{1,32}`), `as_str()`, `ThemeId::gece()`, `Default` = gece.
- Produces: `pub enum AccentPick { Colour(Rgb), Windows }`; `pub struct Picks<'a> { pub accent: Option<&'a AccentPick>, pub floor: Option<Rgb>, pub windows_accent: Option<Rgb> }`.
- Produces: `pub struct ResolvedTheme { pub id: ThemeId, pub open: Palette, pub collapsed: Palette, pub sheet: Palette, pub dark: bool, pub accent_shift: Shift, pub floor_refused: bool }`.
- Produces: `pub struct Registry { packs: Vec<Pack> }` with `Registry::builtin() -> Registry` (parses `BUILTIN`; a refusal panics with the `PackError`, a built-in that refuses is a bug the tests catch), `Registry::from_packs(Vec<Pack>) -> Registry`, `packs(&self) -> &[Pack]`, `get(&self, id: &ThemeId) -> Option<&Pack>`, `resolve(&self, id: &ThemeId, picks: Picks) -> ResolvedTheme`.
- Resolve rules: unknown id resolves as Gece but keeps `id` as given. Floor pick applies only when `floor_adjustable`: `derive_plain(pick)`; `None` from it keeps the pack floor and sets `floor_refused`. The accent is `AccentPick::Colour(c)` or, for `Windows`, `windows_accent` (absent or near-neutral, `c < 0.03`, means the pack's `flow`); then `solve_accent` against the resolved open face's backdrop with the pack `flow` as fallback; ignored when `!flow_adjustable`. `collapsed` uses the collapsed face (plain packs: the derived floor too) with the same solved accent re-checked against it. `sheet` = Gece's open palette with `flow` = the accent solved against Gece's floors, or Gece lime when the accent's `c < 0.03` (spec 6).

- [x] **Step 1: failing tests** in `registry/tests.rs`: `gece_no_picks_is_todays_palette` (`resolve(gece, no picks).open` equals a test-local `TODAYS_GECE: Palette` literal copied from today's `Theme::Gece` arm, so the test survives the enum's removal in Task 6; also `collapsed` and `sheet`); `unknown_id_draws_gece_keeps_id` (`firn` resolves to Gece palettes, `id.as_str() == "firn"`); `sodyum_colours_through_gece` (accent `#FFB224` gives `open.flow == hex(0xFF,0xB2,0x24)`, `Shift::None`); `kagit_colours_through_gece` (floor `#F3F2ED`, accent `#3F6E00`: `open.floor == hex(0xF3,0xF2,0xED)`, `dark == false`, bar holds); `york_colours_through_gece` (floor `#1C2420`, accent `#FFE9A8`); `windows_accent_followed` (`Windows` with `windows_accent = #0078D4` gives a passing flow with the hue of `#0078D4`); `neutral_windows_accent_uses_pack_flow` (`#808080`); `sheet_keeps_gece_floor` (with floor `#F3F2ED` the `sheet.floor` is still `#0B0B0A`); `black_accent_sheet_station_is_lime` (an inline material pack with flow `#000000`: `sheet.flow == Gece lime`); `material_ignores_floor_pick`; `bad_floor_sets_refused` (floor `#777777` when `derive_plain` returns `None`).
- [x] **Step 2:** Run `cargo test -p plith-core theme::registry`. Expected: FAIL.
- [x] **Step 3:** Implement `registry.rs`.
- [x] **Step 4:** Run `cargo test -p plith-core theme`. Expected: PASS (old `theme/tests.rs` still green).
- [x] **Step 5:** Commit `feat(core): theme registry and resolve`.

### Task 5: Settings keys, migration, WPF import, launch, sheet row

**Files:**
- Modify: `crates/plith-core/src/settings.rs` (fields, parse, serialize), `crates/plith-core/src/settings/change.rs`, `crates/plith-core/src/settings/wpf.rs:57`, `crates/plith-core/src/launch.rs:78,252`, `crates/plith-core/src/lang/sheet.rs:309`, `crates/plith-core/src/lang/sheet/fit.rs:60`, `crates/plith-core/src/sheet.rs` (`SheetFacts`), `crates/plith-core/src/sheet/look.rs`
- Create: `crates/plith-core/src/settings/migrate.rs` (keeps `settings.rs` under 500 lines), tests in `settings/tests.rs`, `settings/change/tests.rs`, `settings/wpf/tests.rs`, `launch/tests.rs`, `sheet/look/tests.rs`, `lang/sheet/tests.rs`

**Interfaces:**
- Consumes: Task 4 `ThemeId`, `AccentPick`, `Rgb`.
- Produces: `Settings { theme: ThemeId, accents: BTreeMap<ThemeId, AccentPick>, floors: BTreeMap<ThemeId, Rgb>, .. }`. Keys: `theme=<id>`; `accent.<id>=#RRGGBB|windows`; `floor.<id>=#RRGGBB`. `parse` accepts the two prefixed key families beside `KEYS` (bad id or value: default kept, problem logged with key and length, as today). `serialize` writes `theme=` then `accent.*` then `floor.*` in id order right after it.
- Produces: `settings::migrate::retired(s: &mut Settings, raw_theme: &str) -> bool`: `sodyum` sets `theme=gece` and `accent.gece=#FFB224`; `york` sets `floor.gece=#1C2420`, `accent.gece=#FFE9A8`; `kagit` sets `floor.gece=#F3F2ED`, `accent.gece=#3F6E00`; existing `accent.gece`/`floor.gece` in the file are never overwritten (the migration runs after all lines are read). Returns whether it changed anything; `parse` adds the info line `theme <old> moved to gece` to the problem list.
- Produces: `SettingChange::Theme(ThemeId)`, `SettingChange::Accent(ThemeId, Option<AccentPick>)`, `SettingChange::Floor(ThemeId, Option<Rgb>)` (`None` = Sıfırla, removes the key); all three set `effects.theme`.
- Produces: WPF `Light` maps to `theme=gece` plus the Kağıt floor and accent; `Dark` and `Auto` to gece.
- Produces: `LaunchOptions.theme: Option<ThemeId>` (syntax checked here; the app checks it against the registry, Task 6).
- Produces: `SheetFacts.themes: Vec<ThemeCard>` with `pub struct ThemeCard { pub id: ThemeId, pub name: String }` (name already upper-cased by `lang::upper(name, Lang::Tr)`, Turkish rules for every UI language, spec 4.4); `look::build` lists `facts.themes` as today's choice words. `lang::sheet::theme_name` is removed; `fit.rs` measures the five planned names `GECE, HALKA, FİRN, KABLO, ŞERİT` (constant in `fit.rs`) so the wrap rule is proven before plan 3.

- [x] **Step 1: failing tests:** `settings/tests.rs`: `accent_and_floor_round_trip` (`theme=gece`, `accent.gece=#FFB224`, `accent.halka=windows`, `floor.gece=#1C2420` survive `parse` then `serialize` then `parse`); `bad_accent_lines_logged_and_skipped` (`accent.gece=#12345`, `accent.gece=red`, `accent.=#000000`, `accent.GECE=#000000`, `floor.gece=windows`: four problems, each naming the key and a length, no value text; the other keys applied); `future_theme_kept` (`theme=firn`, `accent.firn=#1C5D99` serialize back unchanged); `sodyum_migrates`, `york_migrates`, `kagit_migrates` (exact keys above); `migration_keeps_user_accent` (`theme=sodyum` plus `accent.gece=#7FD4FF` keeps `#7FD4FF`). `change/tests.rs`: `accent_change_sets_theme_effect`, `accent_reset_removes_key`. `wpf/tests.rs`: `light_imports_as_kagit_colours`. `launch/tests.rs`: `theme_flag_takes_any_valid_id`, `theme_flag_rejects_bad_syntax` (`--theme GECE!`). `look/tests.rs`: rows built from `facts.themes`. `lang/sheet/tests.rs` or `fit` tests: the five names fit in every language (wrap allowed, never shortened).
- [x] **Step 2:** Run `cargo test -p plith-core`. Expected: FAIL (compile errors on the new fields are the failure).
- [x] **Step 3:** Implement. Old `Theme` uses in core move to `ThemeId`; `theme/tests.rs` drops the enum tests that Task 4's registry tests replace and keeps the contrast tests for `Palette::setup()`.
- [x] **Step 4:** Run `cargo test -p plith-core`. Expected: PASS.
- [x] **Step 5:** Commit `feat(core): theme pack settings and migration`.

### Task 6: App and render on the registry; the enum goes

**Files:**
- Modify: `crates/plith-core/src/theme.rs` (remove `Theme`), `crates/plith/src/app.rs:174,322`, `crates/plith/src/app/idle_wiring.rs:42,58`, `crates/plith/src/app/state.rs:99`, `crates/plith/src/app/state/live.rs:88,111,178`, `crates/plith/src/app/state/sheet.rs:160,175,195,261`, `crates/plith/src/app/sheet_side.rs:178,194,221,243,292`, `crates/plith/src/main.rs:28`, their tests (`settings_file/tests.rs`, `app/state/live/tests.rs`), `crates/plith-render/src/thread/tests/palette.rs`, `crates/plith-render/tests/{marks,paint,sheet}.rs`, `crates/plith-render/examples/{render_pages,render_sheet}.rs`
- Modify: `docs/plans/active/halka-island.md` (Task 2 banner: "replaced by `theme-packs.md`; Halka becomes a pack in plan 2"), `docs/STATUS.md`

**Interfaces:**
- Consumes: Task 4 `Registry::builtin`, `resolve`, `ResolvedTheme`; Task 5 settings fields.
- Produces: the app owns one `Registry` (built at start) and a `fn resolved(&self) -> ResolvedTheme` on its state, built from `settings.theme`, `settings.accents`, `settings.floors` and `windows_accent: None` (plan 3 wires the live Windows accent). Island gets `resolved.open` through `relayout.set_palette` exactly as today; the settings window gets `resolved.sheet`, and since the window stays Gece its caption is always dark (`sheet_side.rs:221` drops `is_dark`). `--theme <id>` not in the registry exits with the existing launch error text style ("unknown theme `<id>`"). Usage string: `[--theme <id>]`. Render tests and examples use `Palette::default()` and, where they looped over the four themes, the three migration palettes from `Registry::builtin().resolve(...)` with the Task 5 picks, so the same visual coverage stays.

- [x] **Step 1: failing test** in `app/state/live/tests.rs`: `accent_change_sends_solved_palette` (applying `SettingChange::Accent(gece, Some(Colour(#FFB224)))` sends one palette whose `flow` is `#FFB224`); `sheet_palette_stays_gece_floor` (after `Floor(gece, Some(#F3F2ED))`, the sheet side receives a palette with Gece's floor).
- [x] **Step 2:** Run `cargo test -p plith`. Expected: FAIL.
- [x] **Step 3:** Implement; delete `Theme`; fix every consumer listed under Files. Halka island plan banner; STATUS "Now" line for this branch.
- [x] **Step 4:** Run `node ~/.claude/bin/heavy-run.mjs -- cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`, `node scripts/structure-check.mjs`. Expected: all pass; report the test count from the run's own output.
- [x] **Step 5: Gece unchanged, measured.** Render `cargo run -p plith-render --example render_pages` and `render_sheet` on this branch and on `main` into two fresh folders under the scratchpad; compare the Gece PNGs byte for byte (or per pixel with the examples' WIC helper). Expected: identical island renders; the settings window may differ only if `main` drew it in a non-Gece theme. Then render the three migrated looks and look at them.
- [x] **Step 6:** Commit `refactor: themes resolve from packs`.

---

Progress log (crash recovery, written while the plan runs):
- 2026-11-26 Task 1 `2c07b7e`, Task 2 `2105c8d`, Task 3 `4f7fd61`, Task 4 `1ea56cf` + fix `c80e041`
  (explicit neutral accent picks were dropped), Task 5 `5001758`, Task 6 `7c713ce`, final-review
  fix wave `81e314b` and `b63706f` (low-chroma floors keep their tint). Each task passed its own review.
- Task 6 Step 4: 1665 passed, 0 failed, 1 ignored; clippy, fmt, structure clean. The run skipped
  the heavy-run lock at Berke's request (nm-app held it), capped at `-j 4 --test-threads=4`.
- Task 6 Step 5: 93 renders on `main` and the branch, 86 byte-identical (every Gece island,
  idle, quick, shelf and settings page); 7 differ as expected (3 migrated looks, 4 Look pages
  that now list only GECE).
- Open for plan 2 (final review I1): Halka's black accent solves to grey `#626262` on its Gece
  pill; decide lime-like rule or a `collapsed.flow.color` key. Remove the `grey()` stand-in for
  material floors. Open for plan 3: no Sıfırla until the Vurgu/Zemin rows, so no release before
  plan 3 (I2); cache the resolved theme before the live Windows accent (M3).
