# Identity round 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or
> superpowers:executing-plans to implement this plan task-by-task. Boxes are ticked as work
> progresses, only when the check is proven.

**Goal:** The settings window carries the icon (Hakkında header, flow-coloured section station,
class icon), and the installer's A Halka screens exist as approved material assets.

**Architecture:** `plith-core::sheet` lays out an `Node::Image` with a fixed `ArtId`; the sheet
render thread gets artwork through `SheetConfig` like the island's pages do; the app decodes a
256 px tile PNG (made by `build-icon.ps1`, embedded with `include_bytes!`) once on the UI thread
(COM is up there through `OleInitialize`). Installer assets come from the image model.

**Tech Stack:** Rust 2024, `windows` crate (Direct2D, WIC), PowerShell `System.Drawing` for the
tile, Higgsfield `gpt_image_2_6`.

**Spec:** `docs/specs/2026-11-24-identity-r3-design.md`

## Global Constraints
- English code and comments; Turkish only in user-facing strings. No em dash anywhere.
- Structure: 500 per source file, 80 per function, `src/foo.rs` + `src/foo/bar.rs`, no `mod.rs`.
- Pure layout in `plith-core` with unit tests; Windows code stays out of it.
- Builds and full test runs through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- No text baked into any image; no fabric drawn in code; lime only in the icon and the station.
- Five checks before merge: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, tests,
  `node scripts/structure-check.mjs`, release build.
- Work on branch `feature/identity-r3`; ask Berke before every commit to `main` and every push.

## Review Focus
- A window opened at 250 % or 300 %: the 256 px tile is scaled up slightly, never a blank rect.
  Pinned by a render at scale 2.5 in Task 4.
- A tile that fails to decode (corrupt bytes, COM refused): the window still opens, the About
  header draws without the icon. Test in Task 3 (`icon_art` returns `None` on garbage bytes).
- Long translations of the section name next to the flow station: no overlap. The station x does
  not change, so the existing fit tests cover it; Task 1 asserts the station position is unchanged.
- Kağıt and York: the flow colour is dark green or lamp white; the selected station must still
  differ from the unselected text. Checked on the four-theme renders in Task 4.
- The About rows moved down by the header: the `AÇ` button target must move with its row. Test in
  Task 1.

---

### Task 1: About header and flow station in the sheet layout
**Files:** Modify `crates/plith-core/src/sheet/about.rs`, `crates/plith-core/src/sheet/layout.rs:52-58`,
`crates/plith-core/src/sheet.rs` (const). Test: `crates/plith-core/src/sheet/tests.rs`.
**Produces:** `pub const ICON_ART: ArtId = ArtId(0x504C_4954);` in `plith_core::sheet`;
`pub const ICON_DIP: f64 = 96.0;`.

- [x] Step 1: failing tests in `sheet/tests.rs`:
  - `about_opens_with_the_icon`: the About list has exactly one `Node::Image { art: ICON_ART, rect }`
    with `rect == Rect { x: X, y: FIRST_ROW, w: 96.0, h: 96.0 }` (use the layout consts).
  - `about_names_the_app_and_version`: a `Node::Text` with text `"Plith"` and `TextStyle::Hero`
    whose rect starts at `x == X + 96.0 + 20.0`, and a text containing `facts.version` and
    `facts.build_date` in `TextStyle::Value`, `Ink::Muted` under it.
  - `about_rows_follow_the_header`: the first `rule` of the rows is at
    `FIRST_ROW + 96.0 + 24.0`, and the `OpenFolder` target's rect lies inside the file row.
  - `selected_section_station_is_flow`: for every `Section`, the only filled sidebar station
    (x == 32.0) has `ink == Ink::Flow`, at the same y as before (`58 + i * 32 + 14`).
- [x] Step 2: run `cargo test -p plith-core sheet::` and see the four fail.
- [x] Step 3: implement: in `about.rs` draw the image, `Plith` (Hero, `Ink::Ink`, height 36), and
  `"{version}  {build_date}"` (Value, Muted) at y + 44; the version row is removed (its content
  is now in the header), rows start at `FIRST_ROW + 120.0`. In `layout.rs` the active station
  uses `Ink::Flow`. `Plith` is a brand name: no `tr_upper`, no language table entry.
- [x] Step 4: `cargo test -p plith-core` passes; update any existing About test that counted the
  version row.
- [x] Step 5: commit `feat(sheet): icon header in About, flow station`.

### Task 2: Artwork in the sheet renderer
**Files:** Modify `crates/plith-render/src/sheet.rs` (`SheetConfig`, `Stack`, `render_sheet_offscreen`),
`crates/plith-render/src/sheet/paint.rs` (`SheetPainter` holds `art: Vec<Arc<ArtPixels>>`,
passes it to `ListEnv`). Update `crates/plith-render/examples/render_sheet*` callers.
**Produces:** `SheetConfig { .., art: Vec<Arc<ArtPixels>> }`;
`render_sheet_offscreen(list, scale, palette, art: &[Arc<ArtPixels>]) -> Result<Vec<u8>, RenderError>`.

- [x] Step 1: failing test in `crates/plith-render/src/sheet/tests.rs`
  `offscreen_draws_sheet_art`: a 840 x 640 list with one `Node::Image { art: ArtId(7), rect 10,10,20,20 }`
  and a 2 x 2 opaque red `ArtPixels` with id 7; at scale 1 the pixel at (20, 20) is red
  (B=0, G=0, R=255, A=255) and (5, 5) is the floor.
- [x] Step 2: run it (`cargo test -p plith-render sheet::`), see it fail to compile.
- [x] Step 3: implement; the artwork is fixed for the window's life, so no inbox field.
- [x] Step 4: test passes; `cargo build -p plith-render --examples` builds.
- [x] Step 5: commit `feat(render): artwork in the settings window`.

### Task 3: Tile asset, decode and class icon in the app
**Files:** Modify `scripts/build-icon.ps1` (also writes `crates/plith/assets/plith-tile-256.png`,
the cropped tile at 256 px with the same margin rule); `crates/plith-media/src/art.rs` (make
`decode` `pub`, documented: needs COM on the thread); create `crates/plith/src/app/sheet_side/icon.rs`;
modify `crates/plith/src/app/sheet_side.rs` (`open` fills `SheetConfig.art`);
`crates/plith-shell/src/sheet_window.rs` (`hIcon`, `hIconSm` in `register_class`, loaded like
`tray/notify.rs::icon` from resource 1 at `SM_CXICON` and `SM_CXSMICON`).
**Consumes:** `ICON_ART` (Task 1), `SheetConfig.art` (Task 2).
**Produces:** `pub(super) fn icon_art(encoded: &[u8]) -> Option<Arc<ArtPixels>>` in `icon.rs`,
`ArtPixels { id: ICON_ART, .. }` from `plith_media::art::decode`.

- [x] Step 1: run `pwsh -File scripts/build-icon.ps1`; check the PNG is 256 x 256 with alpha.
- [x] Step 2: failing tests in `sheet_side/tests.rs`: `icon_art_decodes_the_tile` (on the
  embedded bytes, after `CoInitializeEx` in the test: id `ICON_ART`, 256 x 256, 262 144 bytes)
  and `icon_art_is_none_for_garbage` (`b"not a png"`).
- [x] Step 3: implement; a `None` leaves `art` empty and logs one `sheet: icon decode failed` line.
- [x] Step 4: tests pass; `cargo clippy -p plith -p plith-shell -p plith-media -- -D warnings`.
- [x] Step 5: commit `feat(app): icon in the settings window`.

### Task 4: Renders and a real run
**Files:** `crates/plith-render/examples/render_sheet.rs` (loads the tile through WIC with the
same decode, passes art); output to a fresh folder `docs/design/identity/r3/renders/`.
- [x] Step 1: render About and General in Gece, Kağıt, York, Sodyum at 1x and 2x, About in Gece at
  2.5x. Look at every PNG: icon sharp, station in the flow colour and distinct from the text.
- [ ] Step 2: release build, start `plith.exe`, open the settings window; read the log for the
  Progress 2026-11-25 07:30: release rebuilt and running in the RDP session, log clean; the window open with a real click and the title bar and taskbar icon wait for Berke.
  open line and no decode failure; screenshot shows the icon in the title bar and the taskbar.
  Say which half was measured (static look over RDP is fine; no motion here).
- [x] Step 3: commit `docs: identity r3 settings renders`.

### Task 5: Installer A Halka material assets
**Files:** create `docs/design/identity/r3/installer/` (`bg-0.png` to `bg-3.png` for 0 to 3 pressed
ripples, `button.png` transparent, each at 1x 640 x 420 and 2x 1280 x 840; `composite-*.png`;
`README.md` with prompts, model, credits, alignment notes).
- [x] Step 1: generate with `gpt_image_2_6`, the icon master and comp `a3-done.png` as references,
  no text in the prompt; pick the best all-pressed pass.
- [x] Step 2: derive the other three states from it (image-model edits of the same frame, or the
  spec's fallback: flat base plus three ripple-press overlays with alpha). Diff the four at the
  source and the edges: the source must not move by more than 1 px.
- [x] Step 3: compose start, running, done with code-style text (any HTML render is fine) and show
  them on a private page; Berke approves. Done 2026-11-25 on the second background set ("evet şimdi oldu").
- [x] Step 4: commit `docs: installer material assets`.

### Task 6: Record
- [x] ADR 0020 (settings selected-section station takes the theme's flow colour; widens ADR 0017).
- [x] STATUS rewritten (next job), journal line, tick this plan, five checks on the branch, then ask
  Berke about the merge.
