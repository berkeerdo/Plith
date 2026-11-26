# Theme worlds and settings r2

Date: 2026-11-26. Status: approved by Berke 2026-11-26. ADR 0022 (supersedes 0007).
Comps: artifact `PPgArGSRbE82QB6q3ZdytY`, files in `docs/design/themes/r2/`; settings r2 mock:
artifact `UQpu8w5MWVvn9diUWpZUjs`.

## 1. Intent

Berke, 2026-11-26: "temalarımız kötü". The four palettes (Gece, Kağıt, York, Sodyum) are colour
swaps over one floor; none of them feels like a place. The installer's lime fabric did, and
became the Halka idea. This round makes every theme a **material world**: a surface with its
own ink, its own marks and its own way of opening. It also lets a user fit Plith to their own
desk: "kendi kurulumuma uydurmak için, vurgu rengini seçebileyim", and in some themes the
background too.

Themes become **data packs**, so a later theme designer and a theme marketplace sit on the same
format instead of a rewrite.

## 2. Goals, non-goals, anti-goals

Goals
- Five built-in worlds: **Halka** (installer lime fabric), **Firn** (compacted glacier snow),
  **Kablo** (1866 Atlantic cable: gutta-percha face, steel armour wire and copper core at the
  edge), **Şerit** (clean cream telegraph tape, levels punched as hole rows), **Gece** (plain
  black, no material). York, Sodyum and Kağıt retire as themes (section 7 keeps their colours).
- Text reads on every pixel it sits on: primary ink 7:1, muted 4.5:1, faint and flow 3:1,
  measured per pixel inside the pack's text zone, not against the mean.
- The accent (flow) colour is user-adjustable where the pack allows it; the floor colour is
  user-adjustable on plain (material-free) packs. Ink is derived so the bar still holds.
- Settings r2: the Görünüm section returns as a visual theme picker (cards with a mini island),
  plus the r2 control refresh (section 6).
- One pack format, built-in packs embedded in the exe and read by the same loader that will
  later read a user folder.

Non-goals (this round)
- Loading packs from disk, a theme designer, a marketplace. The format is ready; the folder is
  not read yet.
- Recolouring a material (approach A in the conversation: a lime fabric turned blue). The
  manifest reserves `material.tintable`; v1 packs set it `false` and the loader rejects `true`.
- Custom motion code in packs: a pack picks a motion signature from a fixed library.
- Themes in the OSD helper (`plith-osd` keeps its hard-coded Gece colours) and in high contrast
  (still the Windows scheme).

Anti-goals (failure even if it works)
- A user colour that makes text or the flow mark unreadable on any pixel.
- A theme switch or a material floor that costs frames: the bar stays 240 fps at 240 Hz with
  0 to 1 hitches per motion (Oyenza on this PC).
- Materials that look like stock wallpaper behind UI. Each world keeps a quiet text zone; its
  character lives at the edges and in the motion.
- Retired themes silently resetting a user's look to default.

## 3. Approaches

1. **Rust constants per theme.** Simplest; nothing can be added without a rebuild, and the
   designer and marketplace would need a second format. Rejected.
2. **File packs, built-ins embedded (picked).** A pack is a folder: a manifest plus rasters.
   Built-in packs are embedded with `include_bytes!` and parsed at start by the same
   `plith-core` loader that a later user folder uses. Cost: a hand-written parser and its
   tests (the workspace has no toml or serde, and `plith-core` has no dependencies; keeping it
   that way is worth more than the parser saves).
3. **Procedural materials (shaders).** Packs carry parameters, the GPU makes the surface.
   Most flexible, but a large render project, and the approved comps are raster materials.
   Kept as a possible later layer type, not this round.

For colour adjustment Berke picked **B**: material packs fix their floor, only the accent is
adjustable there; plain packs adjust floor and accent. A (material recolouring) stays possible
through the reserved field.

## 4. The pack

### 4.1 Files
```
themes/<id>/
  theme.ini            manifest
  floor-1x.png  floor-2x.png      open island body (material packs)
  edge-1x.png   edge-2x.png       optional strip that follows the bottom edge
  pill-1x.png   pill-2x.png       collapsed island
  <overlay>-1x.png / -2x.png      motion overlays named by the manifest
```
Built-ins live in the repo at `crates/plith-core/themes/<id>/` (moved from `crates/plith/themes/` so core tests read the real files;
one folder per pack, one place). Raster formats: PNG or JPG through WIC, sRGB.

### 4.2 Manifest
Same line rules as `settings.ini` (`key=value`, `#` comments, BOM tolerated, no sections), so
one tokenizer serves both. Keys (v1):

| Key | Meaning |
|---|---|
| `format=1` | Pack format version; an unknown version refuses the pack |
| `id` | Lower-case ASCII, matches the folder, used as the `theme=` value |
| `name` | Product name, shown upper-cased by the rule in 4.4 (`HALKA`, `ŞERİT`) |
| `blurb.<lang>` | One card line per language (`tr`, `en`, `de`, `es`, `fr`, `it`, `pl`, `pt`); `en` required, fallback `en` |
| `floor.kind` | `color` or `material` |
| `floor.color`, `floor.color_high` | Two gradient stops (`color` packs) |
| `floor.lum_min`, `floor.lum_max` | Relative luminance range of the text zone (material packs), measured, checked by test (5.3) |
| `floor.adjustable` | `true` only on `color` packs |
| `material.tintable` | Reserved, must be `false` in v1 |
| `text_zone` | Rect in DIP relative to the open island, where text and marks may sit |
| `ink.primary`, `ink.muted`, `ink.faint`, `ink.track`, `ink.faint_track`, `ink.border` | Pack ink for the open island |
| `flow.color` | Default accent |
| `flow.style` | `fill` (bars, dots) or `punch` (Şerit: levels drawn as hole rows in ink) |
| `flow.adjustable` | Whether the user may pick the accent |
| `flow.swatches` | Up to 6 suggested accents, each already passing on this floor |
| `collapsed.*` | Same `floor.*` and `ink.*` keys for the collapsed pill (Halka: glossy black pill, Gece ink) |
| `motion.open` | One of `rings`, `layers`, `tension`, `feed`, `none` |
| `motion.overlays` | Overlay raster names the signature uses, in order |
| `dark` | Optional; when absent it is derived from the floor luminance |

Colours are `#RRGGBB`. Any parse error, missing required key, failed contrast check or
oversized raster refuses the whole pack with one log line (pack id, key, reason); the theme
falls back to Gece. A built-in pack that refuses is a test failure, never a runtime surprise.

### 4.3 The five built-ins (from comps r2)

| Pack | Floor | Accent | Accent adjustable | Floor adjustable | Motion |
|---|---|---|---|---|---|
| Halka | lime fabric, mean `#C7F910` | `#000000` fill | yes, swatches dark | no | `rings` |
| Firn | snow, mean `#D1DDEC`, layers only at the bottom edge | `#1C5D99` glacier blue | yes | no | `layers` |
| Kablo | gutta-percha `#2A1A14`, armour wire and copper core on the bottom edge | `#E0874A` copper | yes | no | `tension` |
| Şerit | cream tape `#F8E9CC` | punched holes in ink | no (the hole is the mark) | no | `feed` (from the left) |
| Gece | `#0B0B0A` to `#151412` | `#CAFF33` lime | yes | yes | `none` (today's motion) |

Ink values per pack are the comps r2 values (`docs/design/themes/r2/contrast.mjs`), rechecked
per pixel in 5.3. Default theme stays **Gece** (ADR 0007's default; change if Berke says).

### 4.4 Names
`name` is a product name like the old variants. Upper-casing follows the label rule (Berke
2026-11-16): Turkish UI words use Turkish rules; theme names are Plith's own Turkish words, so
`Şerit` upper-cases as `ŞERİT` in every UI language. The builder adds a fit test for five names.

## 5. Colour: the user's accent and floor

### 5.1 What the user picks
For the active theme, Görünüm shows **Vurgu** when `flow.adjustable`, and **Zemin** when
`floor.adjustable`. Each row is: the pack default, the pack's swatches, **Windows** (follows the
Windows accent colour live; my call, it is the cheapest way to match a desk whose RGB already
syncs to Windows), and **Özel** (a custom colour). Özel opens an in-window colour field drawn by
our engine, like the r2 language list: a hue strip, a saturation and lightness square, a hex box.
No Windows colour dialog. **Sıfırla** returns to the pack default. Changes apply live, no save
button, like every other setting.

### 5.2 The solve (pure, `plith-core`)
The user's choice is stored as chosen; what is drawn is solved from it on every load, so a pack
update that changes a floor still yields a readable colour.
- **Accent:** keep the hue (OKLCH), move lightness to the nearest value where the worst-case
  contrast against the floor is at least 3:1. Worst case for a material floor is the colour
  against both `lum_min` and `lum_max`; for a colour floor, against both stops. Chroma is
  reduced only if no lightness passes. If the drawn colour differs from the picked one, the
  row says so in one muted line ("Okunsun diye biraz açıldı" / "koyulaştı").
- **Floor (plain packs):** the user's floor sets both stops (the high stop a fixed small step
  lighter in OKLCH). Ink is derived: primary, muted, faint as near-neutral tints toward the
  floor hue, each solved for 7, 4.5 and 3; track, faint track and border from fixed OKLCH
  offsets. Then the accent is re-solved against the new floor. `dark` follows the floor.
- Tests: every swatch of every built-in pack passes unchanged; a property-style sweep over hues
  and lightness shows every solved output passes; black, white and greys stay valid.

### 5.3 Per-pixel contrast (test, `plith-render/tests`)
A test decodes every built-in raster at 1x and 2x with WIC and checks every pixel inside
`text_zone` against the pack's ink and accent, and that `floor.lum_min`/`lum_max` bound the zone
(so the runtime solve in 5.2 is honest). `contrast.mjs` (mean only) stays as a design-time aid.

### 5.4 Settings file
```
theme=halka
accent.halka=#1A1A1A
accent.gece=windows
floor.gece=#1C2420
```
`accent.<id>` holds `#RRGGBB` or `windows`; absent means the pack default. `floor.<id>` only for
adjustable packs. Per theme, so a colour that suits Kablo does not ruin Firn. Unknown ids and
values are kept-default and logged, as the parser does today.

## 6. Settings r2

Sections stay General, Görünüm (Look), Idle, Osd, About (my call: the comps r2 sidebar was a
sketch; the existing five map one to one). The window itself keeps the plain Gece palette in
every theme (comps note); its selected-section station (ADR 0020) uses the active accent solved
against Gece's floor, and Gece lime when the accent is near-neutral (Halka black).

Görünüm, top to bottom:
- **Tema:** five cards in one row, each a mini collapsed island in its material (the pack's
  `pill` raster) and the name and blurb under it. Selected card: solid ink border and the floor
  one tone lighter (r2 Boşta look cards). Hover plays a small preview of the pack's opening
  motion once. Click switches the island now.
- **Vurgu** and **Zemin** rows as 5.1, only when the pack allows them.

The r2 refresh from the settings mock, kept: choices as boxed segments (selected filled), real
switches, the language row as an in-window list, window 760 x 560, rows 56, Idle look previews
at 84% size in two rows of three, weather sub-rows hidden while weather is off, About with a 56
icon and a one-row update line. Its "Tema in Genel" change is overruled by Görünüm returning.

## 7. Migration

`theme=` values of retired themes become Gece with their colours, so nobody's look resets:
- `sodyum` -> `theme=gece`, `accent.gece=#FFB224`
- `york` -> `theme=gece`, `floor.gece=#1C2420`, `accent.gece=#FFE9A8`
- `kagit` -> `theme=gece`, `floor.gece=#F3F2ED`, `accent.gece=#3F6E00`
The file is rewritten once with the new keys. WPF import (`Dark` -> Gece, `Light` -> Kağıt)
maps `Light` to the Kağıt-coloured Gece above. `--theme` accepts pack ids only.

## 8. Rendering

- `Palette` stays the role table the painters read; a pack plus the user's choices resolves to
  two palettes (open and collapsed) and an optional material. The render thread receives
  `ThemePaint` (palettes, decoded rasters for the current scale, motion signature) through the
  existing latest-wins palette path; `apply_palette` grows into `apply_theme`.
- Only the active pack's rasters are decoded, once per scale, off the UI thread; the cards use
  `pill` rasters only. Budget per pack: 1 MB embedded, 3 MB decoded at 2x (Halka island spec).
- The material body is cropped from art cut at the largest island size, anchored top centre; the
  `edge` strip is drawn at the island's current bottom, so Kablo's wire and Firn's layers stay
  on the bottom edge while the island grows.
- Motion signatures generalize the Halka island plan's material timeline: `rings` (Halka),
  `layers` (Firn settles layer by layer), `tension` (Kablo's wires pull taut), `feed` (Şerit
  feeds in from the left). Each writes the usual `motion` log line and is measured on the
  console against the 240 fps bar.
- A theme switch with the island open may cost one frame; it is not a motion and is not logged
  as one.

## 9. What changes in existing work

- ADR 0022 is this round's ADR. The Halka island plan named its own ADR 0022 ("a theme may carry
  a material"); that ADR is not written and folds into this one.
- `plans/active/halka-island.md`: Task 2 (Halka as an enum variant) is replaced by the pack
  loader; Tasks 1 and 3 to 7 stay and become the first world built on packs.
- `Theme` enum: replaced by a pack registry (`id`, resolved palettes, metadata). Consumers listed
  in the code map (settings, change, wpf import, launch `--theme`, lang sheet names, look sheet,
  live state, sheet side, render examples and the per-theme test loops) move to pack ids.

## 10. Order of work (plans to write)

1. Pack format, loader, registry, colour solve, settings keys and migration (core, all unit
   tested); Gece as a pack, the retired themes as migrations.
2. Material rendering from packs: Halka island plan Tasks 1, 3 to 7 on the pack path.
3. Settings r2: the Görünüm cards, Vurgu and Zemin rows, colour field, and the r2 refresh.
4. Firn, Kablo, Şerit: art cut, packs, motion signatures, per-pixel test, console measurement.

One session with subagents (master decision): plans 2 and 3 both depend on 1, and 4 depends
on 2, so no two streams are independent enough for lanes.

## 11. Rules settled after the builder check (master's calls, Berke may overrule)

- **Art.** Halka uses the installer fabric (Halka island plan Task 1). Firn, Kablo and Şerit art
  is regenerated at production size (largest island at 2x, plus `edge` strips and overlays) by
  the same image model that made comps r2, then cut and scaled to 1x by a script in
  `docs/design/themes/<id>/`. Comp rasters are reference, not shipping art. Berke sees the
  renders before a pack merges.
- **Contrast fails per pixel: the art gives way.** The text zone is flattened or toned in the
  art; ink values and the bar never move. A built-in that fails is fixed in the art.
- **Encodings.** Lists are comma-separated (`flow.swatches=#111111,#2B1F00`,
  `motion.overlays=ring-1,ring-2,ring-3`); `text_zone=x,y,w,h` in DIP; `blurb.<lang>` one key
  per language. `lum_min`/`lum_max` are written by the art script and checked by the 5.3 test.
- **Windows accent.** Source is `UISettings::GetColorValue(UIColorType::Accent)`, live through
  `ColorValuesChanged`, which posts to the UI thread and goes the normal theme path. It is
  solved like any pick; a near-neutral Windows accent falls back to the pack default.
- **No passing lightness.** If no lightness passes even with chroma 0, the pick resolves to the
  pack's `flow.color`. The Özel field greys out the unreachable part of its square live, using
  the same solve, so most picks never get there.
- **Collapsed pill on plain packs** follows `floor.<id>` with the same ink derivation; material
  packs keep their own `collapsed.*`.
- **Settings type.** `Settings.theme` holds the pack id as a string; an unknown id (a future
  user pack, a typo) is kept in the file and drawn as Gece, never rewritten away. Retired ids
  are the only ones migrated.
- **Migration** happens in memory on read and is written on the next save; a read-only or
  unwritable file just migrates again next start. WPF import keeps running only on first run,
  as today.
- **Theme switch.** The old theme stays painted until the new `ThemePaint` is decoded, then
  swaps in one frame. Scale rule: 2x art for scale 1.5 and up, 1x below, resampled by the
  existing high-quality path.
- **Card hover preview** runs the real signature in the settings window's own render thread on
  a mini island. Plan 3 ships cards with the static `pill`; the hover preview lands after plan 4
  has the signatures (Gece and Halka first).

## 12. Left to the builder

Exact OKLCH offsets for derived ink; the colour field's layout inside the 760 x 560 window (to
be mocked before build); overlay counts per signature; the card hover preview's size and length;
where the built-in pack folder sits if `crates/plith/themes/` conflicts with the structure check.
