# Identity round 3: installer screens and settings window from the icon

Date: 2026-11-24. Status: approved by Berke 2026-11-25 ("önerinle devam edelim lütfen"), builder check done.
Paths below are under `docs/`. Inputs: ADR 0019 (icon), `design/identity/icon/README.md` (master
`plith-master-1024.png`, 1024 px), round 1 record `design/identity/comps/README.md` (rejected), settings spec
`specs/2026-11-20-r7-settings-window-design.md`, installer research
`research/2026-11-24-r7-parity.md` section 3. Comps: Berke's private page (12 image-model drafts,
`gpt_image_2_6` with the icon master as reference), picks kept in
`design/identity/r3/`.

## 1. Intent
The icon is settled. The settings window and the installer should look like they come from the
same maker as the icon. Round 1 failed because the metaphor (a lever frame) became the layout and
the drawing was code-made and crude. Round 3 keeps the metaphor in the brand mark only, and takes
real materials from the image model.

## 2. Goals, non-goals, anti-goals
Goals
- Installer screens in direction **A Halka**: the whole window is the icon's lime speaker fabric,
  the glossy black source sits in the top edge, and the icon's three ripples are the three
  install steps (`1 DOSYALAR`, `2 OSD YARDIMCISI`, `3 BAŞLAT`). A finished step's ripple is
  pressed into the fabric; when all three are pressed the window is the icon. Three screens:
  start (`KUR`), running (names the UAC prompt before it comes), done (`PLITH’İ AÇ`, `KAPAT`,
  `WINDOWS İLE BAŞLAT`).
- Settings window variant **S1**: the window stays as it is (Ray Paneli, four themes, sidebar,
  hairline rows). Three brand marks:
  1. Hakkında opens with the icon at 96 DIP, `Plith` in the display size beside it and
     `0.4.0  2026-11-24` in the value style under the name; the existing rows follow.
  2. The selected section's station in the sidebar is drawn in the theme's flow colour (lime in
     Gece, dark green in Kağıt, lamp white in York, amber in Sodyum) instead of ink.
  3. The settings window's own class (`plith-shell/src/sheet_window.rs`) gets the app icon (title
     bar, taskbar, Alt+Tab), loaded at run time from the exe's resource id 1 that `build.rs`
     already embeds (the tray reads the same id).
  Version and date stay live from `SheetFacts` (`version`, `build_date`); the comp's values are
  examples. The flow colours exist per theme in `plith-core/src/theme.rs` (`Ink::Flow`).
- This round delivers: the settings changes built and measured, and the installer's design
  record plus its material assets ready for the builder. The installer program itself is built
  in R7 parity part 2 from this record.

Non-goals
- The installer program, uninstall, trust store, update check (R7 parity part 2).
- New settings rows or sections. Icon changes.
- Animating the ripple press (the builder of part 2 may add a short press; measured then).

Anti-goals
- Anything the image model invented on the comps: fake title bars ("Windows 11 Desktop
  Installer"), sidebar icons (a Windows Settings clone), made-up paths, a progress bar.
- Text baked into images. Every word is drawn by code from the language table (ADR 0015), so
  all eight languages work.
- Fabric drawn in code (round 2: "kumaş deseni gibi durmuyor"). Code draws text, the button
  shape and the step labels; the fabric, source and ripples are raster.
- Lime anywhere else in settings than the icon and the selected station.

## 3. Constraints
- Settings: our engine only. `Node::Image` and `ArtPixels` exist for album art; the sheet painter
  passes `art: &[]` today (`plith-render/src/sheet/paint.rs`), so the icon needs that wiring. The
  pixels come from the icon master (decode once, scaled to the 96 DIP size at the window's scale;
  1x, 1.5x, 2x must stay sharp, so the source is at least 192 px).
- Pure layout stays in `plith-core/src/sheet` with unit tests; structure limits apply.
- Installer materials: per screen state one fabric background without any text, at the window's
  fixed size (640 x 420 DIP) at 1x and 2x: ripples flat, 1 pressed, 1 and 2 pressed, all pressed.
  Same light, same weave, same source position in all four, so they cross-fade cleanly. The
  glossy black button is one separate raster with a transparent background.
- Light theme note: the installer has no theme; it is lime in every theme.

## 4. Approaches (picked in conversation)
1. **Split (picked, Berke: "önerinle devam edelim").** Installer fully in the icon's material,
   settings keep Ray Paneli with brand marks. The installer is a one-time, fixed-size, untheme'd
   window where raster material works; settings are themed and opened often.
2. Brand marks only, everywhere: cheapest, but the installer stays plain, the weak side of round 1.
3. Everything in the material: raster fights four themes and scaling, and puts lime in a window
   opened every day.

Installer directions shown: A Halka (picked, "önerini uygula lütfen"), B Ara Yazı (one fabric
intertitle card per step; pressed text reads poorly), C Tabela (airport sign band over a step
list; most legible, weakest tie to the icon). Settings variants: S1 (picked), S2 (fabric strip
along the top: too loud for a daily window), S3 (paper theme, small icon at the sidebar head).

## 5. Decided
- The settings selected-section mark uses the theme's flow colour. This widens ADR 0017's
  "lime only for sound flows" rule a second time after ADR 0019; recorded as ADR 0020.
- Comps and picks are kept in `design/identity/r3/` (A1 to A3, S1) as the reference the builder
  matches; the other drafts stay on the private page.

## 6. Left to the builder
Settled after the builder check (2026-11-24):
- Installer layers: the source is in every state at the same place; the `KUR` / `PLITH’İ AÇ`
  button is a separate glossy pill raster, not the source. All-pressed repeats the icon's
  composition across the window, not the icon's pixels.
- Assets are made in this session (Higgsfield `gpt_image_2_6`, about 20 credits budgeted). If four
  aligned backgrounds cannot be had, fallback: one flat background plus three ripple-press
  overlays with alpha, cut from one all-pressed pass. Never fabric drawn in code.
- Berke approves the per-screen composites on a page before the assets are final.
- Five checks: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, the test suite,
  `node scripts/structure-check.mjs`, a release build.

Still the builder's call:
- Exact positions and sizes of the step labels next to the ripples, and the text sizes, matched
  to comp A2.
- How the material assets are produced (image-model passes with the master as reference, then
  cropped and aligned), checked side by side at 1x and 2x before use.
- How the icon is decoded for the sheet (WIC from the master PNG or the `.ico` 256 entry).

## 7. Done when
- Settings: rendered at 1x and 2x in all four themes (Hakkında and one other section), the icon
  sharp, the station in the flow colour; title bar and taskbar show the icon on a real run. Tests
  and the five checks green.
- Installer: `design/identity/r3/installer/` holds the four backgrounds and the button at 1x and
  2x, plus a composite per screen (background + code-style text) Berke has seen on a page.
