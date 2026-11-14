# R4 Pages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The tab bar and the audio, media and system pages in the Ray Paneli direction, drawn by
the R1 engine from R2 service data, sized to their content, with the island morphing between them
at 240 fps and 0 to 1 hitches per motion.

**Architecture:** Pure page logic in `plith-core`: view state, layout and hit testing produce a
**display list** (rects, lines, stations, text runs, images) in island DIP. The UI thread owns an
`App` that maps service snapshots to view state, builds the display list and publishes it with
the scene through a latest-wins mailbox that skips unchanged publishes. The render thread draws
each page's display list once into a cached bitmap (text rasterized once), composites the cached
pages with offset and group opacity under the island clip, and draws a small set of live nodes
(meters, traces, seek fill) per frame on an ambient tick that runs only while the island is open.

**Tech Stack:** Rust 2024, `windows` 0.62.2 (Direct2D 1.1, DirectWrite 3 font sets, WIC,
DirectComposition), the R2 crates `plith-audio`, `plith-media`, `plith-system`.

**Spec:** `docs/specs/2026-11-14-rust-rewrite-design.md` (sections 2, 6, 7) and
`docs/specs/2026-11-14-r3-visual-direction.md` (the look); mock `docs/design/r3/ray-paneli.html`,
renders `docs/design/r3/renders-6/`. Engine review items: `docs/quality/2026-11-14-engine-review.md`
L11 to L14.

## Global Constraints

- Files: 500 source, 150 entry (`main.rs`), 800 test lines; 80 lines per function; `src/foo.rs` +
  `src/foo/bar.rs`, never `mod.rs`. `crates/plith-render/src/thread/run.rs` is at 440: split it
  before it grows (Task 5).
- `plith-core` has no Windows code and no dependency on our other crates. Service crates depend on
  nothing of ours. `crates/plith` copies service fields into core input types; every decision
  about those values is a core function with a unit test (`node scripts/check-deps.mjs`).
- The UI thread never draws; the render thread owns every Direct2D, DirectWrite and WIC object
  that draws. Nothing on the render thread or a service thread calls `SendMessage` to the UI
  thread.
- G2: at rest (collapsed, no motion) zero render wake-ups; `--idle-check 60` must still pass.
  Ambient frames (meters, seek) only while expanded on a page that shows them, at `hz / 4`.
- Every motion writes its `motion` line; the bar is 240 fps, 0 to 1 hitches, draw p99 under 1 ms.
- Palette (Gece) and type from the R3 spec: floor `#0B0B0A` to `#151412`, border `#2B2824`, ink
  `#ECE9E2`, muted `#A39E94`, faint `#8A847A`, track `#4A463F`, flow `#CAFF33`. Public Sans for
  names and titles, Martian Mono with tabular numerals for labels and every changing number,
  smallest text 10 px, uppercase mono labels. 1 px track, 2 px lit route, 7 px station squares,
  45 degree turns. No `+` corner marks.
- Surfaces (DIP): pill 200 x 32; pages 620 wide: audio 300, media 232, shelf 214, system 178 high
  for the mock data; audio grows with rows up to `PAGE_MAX_HEIGHT = 420`.
- User-facing strings are Turkish and copied from the mock (`SES`, `MEDYA`, `RAF`, `SİSTEM`,
  `SESSİZ`, `KAPALI`, `KAYNAĞA, SONRA ÇIKIŞA TIKLA`, `Çalan bir şey yok`, ...). Code is English.
- No hardware writes in any run without the master's relayed yes; every run of mine uses the
  fixture services (Task 16). No script moves the mouse. On-screen runs: guarded, at most about
  70 s, after the master relays that Berke is looking.
- Every build and test run: `node ~/.claude/bin/heavy-run.mjs -- <cmd>`. No `rm` on variable or
  glob paths, no `bash -c`/`eval`, multi-line commit messages via `git commit -F <file>`.
- `windows` crate APIs are checked with context7 before first use in a task.

## Review Focus

1. A service publishing at poll rate while the island is collapsed: no render wake-up, no present
   (L12). Test: Task 3 `publish_same_content_does_not_bump`, Task 16 idle check with fixtures
   ticking.
2. Content arriving mid-morph (track change during a page turn): the turn keeps its velocity and
   only the changed page's bitmap is rebuilt. Test: Task 5 `content_change_mid_motion_keeps_motion`.
3. Long or empty names (a 60-character endpoint name, an empty title, an empty Voicemeeter label):
   middle truncation for device names, end truncation for titles, fallback labels, never an empty
   row. Tests in Tasks 12 to 14 (`long_endpoint_name_truncates_in_middle`, `empty_title_*`).
4. A service that is unavailable or vanishes (Voicemeeter quits, no capture device, no media
   session, no DDC/CI): each page shows its drawn degraded state instead of stale data. Tests:
   Task 12 `no_voicemeeter_shows_windows_outputs`, Task 13 `no_session_shows_empty`, Task 14
   `no_mic_hides_mic_row`, `brightness_unsupported`.
5. A click that lands while the page is turning or the layout just changed: the hit test uses the
   layout that is on screen (the incoming page once its opacity passes 0.5), never a stale one.
   Test: Task 17 `click_during_turn_hits_incoming_page`.

---

## P5: the UI layer shape (decision for an ADR the master files)

| | Retained widget tree | Immediate mode in the render thread | **Display list built on the UI thread, drawn retained** |
|---|---|---|---|
| Where layout runs | widget objects on the render thread | render thread each frame | UI thread, pure `plith-core` functions |
| Hit testing | needs the tree on the UI thread or a round trip | needs a round trip or a copy of the layout | same pure layout the list came from, on the UI thread |
| Text cost | cached per widget | re-laid each frame unless cached by hand | rasterized once per content change into a page bitmap |
| Testability | widget code is Windows-bound | Windows-bound | layout and list are unit tested without Windows |
| Cost per frame | tree walk | full draw | one bitmap blit per visible page + a few live nodes |
| Risk | most code (invalidation, events) | text per frame breaks the 1 ms draw bar | live values (meters, seek) need a separate path |

**Recommendation: the third column.** It keeps the render thread dumb (draw a list, blit a
bitmap), puts every decision where it can be tested, makes hit testing a local call on the UI
thread, and turns the spec's "offscreen content layers, text rasterized once" into the default.
The price is the live-node path (Task 5, Task 7) for values that change at frame rate. Task 1
writes this as `docs/plans/active/pages-r4-adr-drafts.md` P5 for the master to file.

---

### Task 1: P5 ADR draft and questions to the master

**Files:**
- Create: `docs/plans/active/pages-r4-adr-drafts.md`

- [x] **Step 1:** Write the P5 draft from the section above (decision, the comparison table,
  consequences: display list types in core, page bitmaps, live nodes, ambient tick at `hz / 4`).
- [x] **Step 2:** Commit `docs(pages): draft the UI layer ADR`; push the branch.

### Task 2: Generic UI wake (L11)

**Files:**
- Modify: `crates/plith-shell/src/poster.rs` (all `WM_APP + n` ids live here), `crates/plith-shell/src/suppress.rs:25` (use the shared id), `crates/plith-shell/src/window.rs` (dispatch), `crates/plith-shell/src/lib.rs`
- Create: `crates/plith-shell/src/poster/tests.rs`

**Interfaces:**
- Produces: `pub struct UiWaker` (`Clone + Send + Sync`), `UiWaker::wake(&self)`;
  `Shell::ui_waker(&self) -> UiWaker`; `ShellEvent::Wake`; `pub(crate) const WM_APP_WAKE: u32 =
  WM_APP + 3`. Coalescing lives in `pub(crate) struct Coalesce(AtomicBool)` with
  `fn arm(&self) -> bool` (true = caller must post) and `fn clear(&self)`, shared by
  `ShapePoster` and `UiWaker`.

- [x] **Step 1: Failing tests** `arm_once_until_cleared` (arm true, arm false, clear, arm true),
  `failed_post_clears` (a post closure returning false leaves the coalescer clear).
- [x] **Step 2:** Run `cargo test -p plith-shell poster`; expect a compile failure.
- [x] **Step 3:** Implement `Coalesce`, `UiWaker` (post `WM_APP_WAKE`, clear on failure),
  dispatch `WM_APP_WAKE` to `clear` + `emit(ShellEvent::Wake)`. Doc comment on `UiWaker`: the
  invariant that render and service threads only post, never send.
- [x] **Step 4:** Five checks green (LANES.md "Done"), commit `feat(shell): add a generic UI wake`.

### Task 3: Mailbox that carries content and skips no-op publishes (L12, L13)

**Files:**
- Modify: `crates/plith-core/src/scene.rs`, `crates/plith-core/src/scene/tests.rs`, `crates/plith-core/src/shapes.rs`, `crates/plith-core/src/layout.rs`, `crates/plith-core/src/demo.rs`, `crates/plith/src/demo.rs`, `crates/plith/src/app/wire.rs`
- Create: `crates/plith-core/src/draw.rs` (display list types only; painting is Task 7)

**Interfaces:**
- Produces in `draw.rs`: `Ink` (`Floor`, `FloorHigh`, `Border`, `Ink`, `Muted`, `Faint`,
  `Track`, `Flow`), `Rect { x, y, w, h: f64 }`, `TextStyle` (`Title`, `Name`, `Sub`, `Label`,
  `Value`, `Clock`), `Trim` (`End`, `Middle`), `Align` (`Start`, `End`, `Center`),
  `StationMark` (`Filled`, `Hollow`), `Node` with variants `Fill { rect, radius, ink }`,
  `Stroke { points: Vec<(f64, f64)>, width, ink, dashed: bool }`, `Station { x, y, mark, ink }`,
  `Text { rect, text: Arc<str>, style, ink, align, trim }`, `Image { rect, art: ArtId }`, and
  `Live(LiveNode)`; `LiveNode` with `Meter { rect, rail: LiveRail, index }`,
  `Trace { rect, rail, index }`, `Seek { rect, timeline: SeekTimeline }`. `pub struct
  DisplayList { pub nodes: Vec<Node>, pub size: (f64, f64) }`, `PartialEq`.
  `pub struct PageContent { pub pages: [Arc<DisplayList>; 4], pub pill: Arc<DisplayList> }`.
- Produces in `scene.rs`: `Scene` gains `shapes: [IslandShape; 4]` (still `Copy`; default from
  `shapes::PAGES`); `Scene::target()` reads `shapes[page]`. `SceneMailbox::new(scene,
  content: Arc<PageContent>)`; `update(&self, change) -> Option<u64>` and `publish(scene) ->
  Option<u64>` return `None` and do not notify when the scene is unchanged;
  `publish_content(&self, content: Arc<PageContent>) -> Option<u64>` skips when
  `*old == *new`; `latest()` returns `Snapshot { scene, content: Arc<PageContent>, generation }`.
- Produces in `shapes.rs`: `PAGES: [IslandShape; 4]` = audio 620 x 300, media 620 x 232, shelf
  620 x 214, system 620 x 178, page radius from the master's answer to open item 1 (24 proposed);
  `PAGE_MAX_HEIGHT: f64 = 420.0`; `SurfaceLayout` sizes for 620 x `PAGE_MAX_HEIGHT`.

- [ ] **Step 1: Failing tests** `update_same_scene_returns_none` (no generation bump),
  `publish_same_content_does_not_bump`, `publish_new_content_bumps_and_wakes` (a waiter thread
  returns), `target_reads_scene_shapes`, `surface_fits_max_page`.
- [ ] **Step 2:** Run `cargo test -p plith-core scene layout`; expect failures.
- [ ] **Step 3:** Implement. The demo `page` step accepts 0..=3. Callers that ignored the
  returned generation keep compiling with `let _ =` only where the value is truly unused.
- [ ] **Step 4:** Five checks green, commit `feat(core): carry page content in the mailbox`.

### Task 4: UI-thread `App` and an STA UI thread (L14)

**Files:**
- Create: `crates/plith/src/app/state.rs` (`App`), `crates/plith/src/app/state/tests.rs`
- Modify: `crates/plith/src/app.rs`, `crates/plith/src/app/wire.rs`, `crates/plith-shell/src/process.rs`

**Interfaces:**
- Produces: `pub struct App` owned through `Rc<RefCell<App>>` by the shell handler, with
  `fn on_event(&mut self, event: ShellEvent)`, `fn attach_render(&mut self, stop: StopHandle)`
  (replaces the `OnceLock`), and fields for the mailbox, log, closing flag and later the model
  (Task 16). `plith_shell::init_ui_thread() -> windows::core::Result<()>` calls `OleInitialize`
  (STA, needed by R5) before the window exists.
- Consumes: `ShellEvent::Wake` (Task 2), `SceneMailbox` (Task 3).

- [ ] **Step 1: Failing tests** in `state/tests.rs` with a fake stopper:
  `closing_before_render_attached_is_safe`, `click_toggles_expanded`,
  `suppression_republishes`.
- [ ] **Step 2:** Run `cargo test -p plith app::state`; expect failures.
- [ ] **Step 3:** Move the handler body from `wire::event_handler` into `App::on_event`;
  `run_shell` builds `App`, creates the shell with a closure over the `Rc`, then attaches the
  render stop handle. The `Closing` path still stops and joins before `DefWindowProc` destroys
  the window.
- [ ] **Step 4:** `cargo run --release -p plith -- --idle-check 20` through heavy-run: exit 0 and
  `idle` passes in the log (no screen look needed; this is not motion).
- [ ] **Step 5:** Five checks green, commit `refactor(app): own UI state in one App`.

### Task 5: Render loop: split, content repaint, ambient tick

**Files:**
- Split: `crates/plith-render/src/thread/run.rs` into `run.rs` (loop entry, `run`, `animate`),
  `run/engine.rs` (`Engine` state, `aim`, `snap`, `paint_frame`), `run/ambient.rs` (new)
- Modify: `crates/plith-render/src/thread.rs` (`RenderSinks`), `crates/plith-render/src/thread/tests.rs`, `crates/plith-render/src/thread/tests/harness.rs`, `crates/plith-render/src/paint.rs` (`PaintFrame`)
- Create: `crates/plith-core/src/ambient.rs` + `ambient/tests.rs`

**Interfaces:**
- Produces in core: `pub fn ambient_divisor(hz: f64) -> u32` (`hz / 60` rounded, at least 1; 4 at
  240 Hz); `pub fn wants_ambient(scene: &Scene, content: &PageContent) -> bool` (expanded, not
  suppressed, and the visible page's list has a `Live` node); `AmbientReport` with `frames`,
  `hitches`, `worst_gap_ms`, `draw_p99_ms` and `Display` as
  `ambient: <page> <secs> s, <frames> frames, <hitches> hitches, worst <ms> ms, draw p99 <ms>`.
- Produces in render: `PaintFrame` gains `content: Arc<PageContent>`, `content_gen: u64`,
  `live: LiveValues`; `RenderSinks` gains `sample: Box<dyn FnMut() -> LiveValues + Send>` (levels
  and traces, called once per ambient frame) and `on_ambient: Box<dyn Fn(AmbientReport) + Send>`.
  `LiveValues` (core, `ambient.rs`): `strips: Vec<[f32; 2]>`, `buses: Vec<[f32; 2]>`, `traces:
  Vec<TraceSamples>`, `now: SystemTime`.
- Behaviour: a content-only change at rest paints exactly one frame. While `wants_ambient`, the
  settled loop waits on the clock and paints every `ambient_divisor` ticks, with no `motion`
  line; it writes one `ambient` line when it stops. A scene change during ambient starts a normal
  motion (motion line as today).

- [ ] **Step 1: Failing tests** (fake clock and painter in `harness.rs`):
  `content_change_at_rest_paints_once`, `content_change_mid_motion_keeps_motion` (velocity kept,
  one motion line), `ambient_runs_only_when_expanded_with_live_nodes`,
  `ambient_paints_every_fourth_tick_at_240`, `collapse_stops_ambient_and_reports`,
  `idle_after_collapse_has_no_wakes`; core `divisor_at_60_120_144_240`.
- [ ] **Step 2:** Run `cargo test -p plith-render thread` and `-p plith-core ambient`; expect
  failures.
- [ ] **Step 3:** Split first (no behaviour change, tests still green), then implement.
- [ ] **Step 4:** Five checks green, commit `feat(render): repaint content and tick ambient`.

### Task 6: Text with DirectWrite and the bundled fonts

**Files:**
- Create: `crates/plith-render/src/text.rs` (fonts, formats, layout cache), `crates/plith-render/src/text/trim.rs`, `crates/plith-render/assets/fonts/` (font files + `OFL.txt`, only after the master's yes on question 7), `crates/plith-render/tests/text.rs`
- Modify: `crates/plith-render/Cargo.toml` (`Win32_Graphics_DirectWrite`, `Win32_Graphics_Imaging`), `crates/plith-core/src/draw.rs` (style table)

**Interfaces:**
- Produces in core: `pub fn style_spec(style: TextStyle) -> StyleSpec` with `family: Family
  (Sans | Mono)`, `size_dip`, `weight`, `uppercase`, `tracking`: Title Sans 17 semibold, Name
  Sans 14, Sub Mono 10 upper, Label Mono 10 upper tracking 0.08 em, Value Mono 12, Clock Mono 14.
- Produces in render: `pub struct TextSystem` (render thread only) with
  `TextSystem::new() -> Result<Self>` (font set from the embedded files through
  `IDWriteFactory5::CreateInMemoryFontFileLoader` and `IDWriteFontSetBuilder2::AddFontFile`;
  falls back to Segoe UI Variable / Cascadia Mono when the set cannot be built and logs one
  `note`), `fn layout(&mut self, text: &str, style: TextStyle, max_w: f32, trim: Trim) ->
  Result<IDWriteTextLayout>` cached by `(text, style, max_w bits, trim)` with at most 512 entries
  (clear-all when full), tabular figures through `IDWriteTypography` (`DWRITE_FONT_FEATURE_TAG_
  TABULAR_FIGURES`) on every Mono format. Middle trim: keep head and tail, insert `…`, binary
  search on measured width.

- [ ] **Step 1: Failing tests** (WARP, `tests/text.rs`): `mono_digits_share_one_advance`
  ("1111" and "8888" measure equal), `middle_trim_keeps_both_ends`
  ("Speakers (Realtek(R) High Definition Audio Device)" at 120 DIP starts with "Spea" and ends
  with "ice)"), `cache_reuses_layouts`, `missing_font_falls_back`.
- [ ] **Step 2:** Run through heavy-run `cargo test -p plith-render --test text`; expect failures.
- [ ] **Step 3:** Implement (context7 check on `IDWriteFactory5`, `IDWriteFontSetBuilder2`,
  `IDWriteTypography` first).
- [ ] **Step 4:** Five checks green, commit `feat(render): add DirectWrite text and fonts`.

### Task 7: Painting the display list: page bitmaps, clip, live nodes

**Files:**
- Create: `crates/plith-render/src/list.rs` (draws `DisplayList` nodes), `crates/plith-render/src/pages.rs` (per-page bitmap cache), `crates/plith-render/src/live.rs` (meters, traces, seek)
- Modify: `crates/plith-render/src/paint.rs` (placeholder bars removed; composite cached pages), `crates/plith-render/src/offscreen.rs`, `crates/plith-render/tests/paint.rs`
- Create: `crates/plith-core/src/theme.rs` + `theme/tests.rs`

**Interfaces:**
- Produces in core: `pub struct Palette { floor, floor_high, border, ink, muted, faint, track,
  faint_track, flow: [f32; 4] }`, `pub enum Theme { Gece, Kagit, York, Sodyum }`,
  `Theme::palette()`, `Palette::resolve(Ink) -> [f32; 4]`, `pub fn contrast(a, b) -> f64`
  (WCAG). Values from the spec and ADR 0007 (faint per theme: Gece `#8A847A`, Kağıt `#6B665E`,
  York `#86958E`, Sodyum `#86868D`; the other Kağıt, York, Sodyum values copied from the mock's
  `#themes` CSS).
- Produces in render: `PageCache::draw(&mut self, ctx, page: usize, list: &Arc<DisplayList>,
  generation) -> Result<&ID2D1Bitmap1>` rebuilds only when `Arc::ptr_eq` fails;
  `Painter::paint` composites incoming and outgoing page bitmaps with `DrawBitmap` at the layer
  offset and `layer.opacity * content_opacity`, inside the island clip, then draws live nodes of
  the incoming page on top. The pill list draws when collapsed (content opacity 0 for pages,
  1 - content for the pill).

- [ ] **Step 1: Failing tests:** core `every_theme_passes_contrast` (text inks 4.5:1, flow and
  marks 3:1 against both floor stops); render (`render_offscreen`, WARP)
  `station_filled_and_hollow_pixels`, `lit_route_is_two_px_flow`, `page_bitmap_reused_across_frames`
  (a counting hook in `PageCache`), `outgoing_page_fades_with_opacity`, `live_meter_draws_level`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement. Group opacity comes from drawing the cached bitmap with an opacity,
  never by fading each node.
- [ ] **Step 4:** Five checks green, commit `feat(render): paint display lists from page cache`.

### Task 8: Artwork decode on the media worker

**Files:**
- Create: `crates/plith-media/src/art.rs` + `art/tests.rs`
- Modify: `crates/plith-media/src/snapshot.rs`, `crates/plith-media/src/service.rs`, `crates/plith-media/Cargo.toml` (`Win32_Graphics_Imaging`)

**Interfaces:**
- Produces: `pub struct Artwork { pub width: u32, pub height: u32, pub bgra: Arc<[u8]>, pub
  id: u64 }` (premultiplied, longest side 256 px, `id` a hash of the encoded bytes);
  `MediaSnapshot::artwork: Option<Arc<Artwork>>` decoded once per new thumbnail on the media
  worker thread with WIC. `thumbnail` stays for the probe.
- Consumes in render (Task 7): `Node::Image { art: ArtId(u64) }`, bitmaps created from the pixels
  passed in `PageContent.art: Vec<Arc<Artwork>>` (field added here; core gets a mirror type
  `ArtPixels` so it stays service-free).

- [ ] **Step 1: Failing tests:** `decodes_png_to_premultiplied_bgra` (a 2 x 2 PNG fixture in
  `art/tests.rs` as bytes), `scales_longest_side_to_256`, `garbage_bytes_give_none`.
- [ ] **Step 2:** Run `cargo test -p plith-media art`; expect failures.
- [ ] **Step 3:** Implement with `IWICImagingFactory`, `IWICBitmapScaler`,
  `IWICFormatConverter` to `GUID_WICPixelFormat32bppPBGRA` (context7 check first).
- [ ] **Step 4:** Five checks green, commit `feat(media): decode artwork on the worker`.

### Task 9: Voicemeeter routes in the audio service (read)

**Files:**
- Modify: `crates/plith-audio/src/voicemeeter.rs`, `crates/plith-audio/src/voicemeeter/poll.rs`, `crates/plith-audio/src/voicemeeter/service.rs`, `crates/plith-audio/src/voicemeeter/layout.rs`, `crates/plith-audio/src/voicemeeter/tests.rs`

**Interfaces:**
- Produces: `ChannelSnapshot::routes: Vec<bool>` for strips (one per bus, read from
  `Strip[i].A1..A5` and `Strip[i].B1..B3` as the edition has them), empty for buses;
  `pub fn route_param(kind: VmKind, bus: usize) -> &'static str` (`A1`..`A5`, `B1`..`B3`, same
  split as `fallback_label`); `VoicemeeterService::toggle_route(strip, bus) -> Option<bool>`
  (written, not exercised on hardware without a yes).

- [ ] **Step 1: Failing tests:** `route_params_banana` (`[A1, A2, A3, B1, B2]`),
  `route_params_potato`, `route_params_standard`, `poll_reads_routes_into_snapshot` (with the
  existing fake API in `tests.rs`).
- [ ] **Step 2:** Run `cargo test -p plith-audio voicemeeter`; expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Five checks green, commit `feat(audio): read Voicemeeter strip routes`.

### Task 10: Tab bar and clock

**Files:**
- Create: `crates/plith-core/src/pages.rs` (page ids, shared layout constants), `crates/plith-core/src/pages/tabbar.rs` + `tabbar/tests.rs`

**Interfaces:**
- Produces: `pub enum PageId { Audio, Media, Shelf, System }` with `index()`, `from_index`,
  `label()` (`SES`, `MEDYA`, `RAF`, `SİSTEM`); `pub fn tabbar(active: PageId, clock: &str) ->
  (Vec<Node>, Vec<HitTarget>)`: four stations on one hairline that runs only between the first
  and last station, active = filled ink square + ink label, others hollow + faint label, clock
  right-aligned (Mono). `pub enum Action` (grows in Tasks 12 to 14) with `Tab(PageId)`;
  `pub struct HitTarget { rect: Rect, action: Action }`; `pub fn hit(targets: &[HitTarget], x,
  y) -> Option<Action>` (first match, 4 DIP slack). `pub fn clock_text(hour: u8, minute: u8)
  -> String` (`21:47`).

- [ ] **Step 1: Failing tests:** `four_stations_one_line_between_ends`, `active_is_filled_ink`,
  `tab_hit_targets_cover_labels`, `clock_pads_zero` (`09:05`), `hit_slack_four_dip`.
- [ ] **Step 2:** Run `cargo test -p plith-core pages::tabbar`; expect failures.
- [ ] **Step 3:** Implement with positions measured off the mock at 1:1 DIP (stations at x 34,
  161, 288, 415 from the page's left edge; line y 38; labels y 14; clock right edge 586).
- [ ] **Step 4:** Five checks green, commit `feat(core): add the tab bar layout`.

### Task 11: Audio view model

**Files:**
- Create: `crates/plith-core/src/pages/audio.rs` + `audio/model.rs` + `audio/tests.rs`

**Interfaces:**
- Produces inputs (plain copies of service data): `AudioFacts { vm: Option<VmFacts>, endpoints:
  Vec<EndpointFacts>, default_id: Option<String>, attached: Option<(f32, bool)>, mic:
  Option<MicFacts> }`, `VmFacts { kind_strips, kind_buses, strips: Vec<ChannelFacts>, buses:
  Vec<ChannelFacts> }`, `ChannelFacts { label, gain_db, muted, routes: Vec<bool> }`.
- Produces: `pub struct AudioView { sources: Vec<SourceRow>, outputs: Vec<OutputRow>, routes:
  Vec<Route>, mode: PickMode, footer: Footer }`; `Route { source, output, state: RouteState }`
  with `RouteState { Live, Idle, Cut, Candidate }`; `pub enum PickMode { None, Picked(usize) }`;
  `pub fn audio_view(facts: &AudioFacts, activity: &Activity, mode: PickMode) -> AudioView`.
  `Activity` holds per-strip "signal in the last 2 s" flags, updated by
  `Activity::observe(levels, now)` with a 2 s hold (pure, fed by the UI timer in Task 16).
- Rules: lit (`Live`) = routed, source and output unmuted, signal held; `Idle` = routed, no
  signal; muted source = `Cut` with `SESSİZ`; in pick mode the picked source's unrouted outputs
  are `Candidate`. No Voicemeeter: one source row "Windows / VARSAYILAN ÇIKIŞ", outputs are the
  active endpoints, the default is `Live`. Density (open item 2): draw `Live` and `Candidate`
  routes plus `Idle` only for the picked source; rows above 5 shrink pitch from 57 to 40 DIP.
  Short names: Mono labels upper-cased by the style, device names middle-trimmed by the renderer.

- [ ] **Step 1: Failing tests:** `routed_with_signal_is_live`, `routed_without_signal_is_idle`,
  `muted_source_is_cut`, `pick_mode_marks_unrouted_outputs_as_candidates`,
  `no_voicemeeter_shows_windows_outputs`, `potato_rows_shrink_pitch`, `activity_holds_two_seconds`,
  `empty_label_uses_fallback`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Five checks green, commit `feat(core): map audio facts to the audio view`.

### Task 12: Audio page layout

**Files:**
- Create: `crates/plith-core/src/pages/audio/layout.rs` + `layout/tests.rs`

**Interfaces:**
- Produces: `pub fn audio_page(view: &AudioView, clock: &str) -> PageBuild`;
  `pub struct PageBuild { list: DisplayList, targets: Vec<HitTarget>, shape: IslandShape }`
  (shared by Tasks 13 to 15). Actions added: `PickSource(usize)`, `PickOutput(usize)`,
  `CancelPick`, `SetDefaultEndpoint(String)`, `ToggleSourceMute(usize)`.
- Geometry: source column x 34 (name Sans, sub Mono), trace slot x 190 to 262, source station x
  287, output station x 512, output label x 528, value right edge 586; rows from y 82; a route is
  a polyline: horizontal from the source station, a 45 degree run, horizontal into the output
  station (`fn route_path(from_y, to_y) -> Vec<(f64, f64)>`). Footer label at the bottom
  (`KAYNAĞA, SONRA ÇIKIŞA TIKLA`, pick mode `ÇIKIŞA TIKLA: YOL KURULUR` + the cancel text from
  question 11, no Voicemeeter `VOICEMEETER ÇALIŞMIYOR · ÇIKIŞA TIKLA: VARSAYILAN OLUR`).
  Page height = rows bottom + footer, clamped to `PAGE_MAX_HEIGHT`.

- [ ] **Step 1: Failing tests:** `route_turns_at_45_degrees`, `straight_route_has_two_points`,
  `banana_page_is_300_high` (the mock data), `potato_page_fits_max_height`,
  `long_endpoint_name_truncates_in_middle` (the node's trim is `Middle`), `live_route_is_two_px_flow`,
  `meters_and_traces_are_live_nodes`, `hit_source_then_output_actions`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Five checks green, commit `feat(core): lay out the audio page`.

### Task 13: Media page

**Files:**
- Create: `crates/plith-core/src/pages/media.rs` + `media/tests.rs`

**Interfaces:**
- Produces inputs `MediaFacts { has_session, source_name, title, artist, art: Option<ArtId>,
  playing, can_seek, can_next, can_prev, timeline: Option<SeekTimeline>, output: Option<String> }`
  and `pub fn media_page(facts: &MediaFacts, clock: &str) -> PageBuild`. Actions:
  `PlayPause`, `Next`, `Previous`, `Seek(f64)` (fraction 0..1 from the click x on the ruler).
- Layout per mock: art 108 x 108 at (34, 70); title (Title) and artist line (Sub) at x 166;
  ruler from x 166 to 586 at y 140 with ticks every 1/12 and taller ticks at quarters; times
  under its ends (Mono tabular, `1:42`, `4:03`); transport centred under the ruler; output route
  row `SPOTIFY > A1 · G733` at y 216 (read-only per question 8). No session: `Çalan bir şey yok`
  + `Spotify, tarayıcı ya da bir oyun ses verdiğinde burada görünür.` + hollow route. No
  timeline: ruler and times hidden, transport stays. Disabled transport buttons are faint and
  have no hit target. The played part of the ruler and the position text are `Live::Seek`.
  `pub fn format_time(secs: u64) -> String` (`m:ss`, `h:mm:ss` from an hour).

- [ ] **Step 1: Failing tests:** `no_session_shows_empty`, `no_timeline_hides_ruler`,
  `seek_fraction_from_click_x`, `disabled_next_has_no_target`, `format_time_cases` (0 → `0:00`,
  102 → `1:42`, 3725 → `1:02:05`), `empty_title_shows_source_name`, `page_is_232_high`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Five checks green, commit `feat(core): add the media page`.

### Task 14: System page, shelf placeholder, pill seal

**Files:**
- Create: `crates/plith-core/src/pages/system.rs` + `system/tests.rs`, `crates/plith-core/src/pages/pill.rs` + `pill/tests.rs`, `crates/plith-core/src/pages/shelf.rs`

**Interfaces:**
- Produces `SystemFacts { brightness: Option<(String, u8)>, mic: Option<MicFacts>, mic_bus:
  Option<String> }`, `pub fn system_page(facts, clock) -> PageBuild`; actions
  `SetBrightness(u8)`, `ToggleMic`. Brightness ruler in ink (never flow), value right (`70`);
  unsupported: `DESTEKLENMİYOR · MONİTÖRDE DDC/CI KAPALI`; mic route flow when open, cut with
  `KAPALI` centred when muted; no capture device: the mic row is absent. Height 178.
- `pub fn shelf_page(clock) -> PageBuild`: the empty siding (`Dosyaları buraya sürükle`,
  `RAFTA BEKLER, İSTEDİĞİN YERE GERİ SÜRÜKLERSİN`), height 214 (drop is R5).
- `pub fn pill(facts: &PillFacts) -> DisplayList` with `PillFacts { output: String, value:
  Option<u8>, state: PillState (Live | Muted | Idle) }`: three fixed slots (name left, route
  segment middle, value right); muted shows the cut segment and `KAPALI` (question 3).

- [ ] **Step 1: Failing tests:** `brightness_ruler_is_ink_not_flow`, `brightness_unsupported`,
  `muted_mic_is_cut_with_kapali`, `no_mic_hides_mic_row`, `system_page_is_178_high`,
  `pill_slots_do_not_move_between_states`, `shelf_empty_is_214_high`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Five checks green, commit `feat(core): add system, shelf and pill`.

### Task 15: Offscreen renders of every page against the mock

**Files:**
- Create: `crates/plith-render/examples/render_pages.rs` (writes PNGs with WIC into a new
  uniquely named folder under `%TEMP%`), `crates/plith-core/src/pages/fixtures.rs` (the mock's
  data: Banana with G733, Edifier, Discord; Potato; no Voicemeeter; nothing playing; DDC/CI off)

- [ ] **Step 1:** Render all states in Gece plus one audio page per theme at scale 2.
- [ ] **Step 2:** Compare each PNG with `docs/design/r3/renders-6/sheet.png` by eye; list
  differences; fix the layout constants in Tasks 10 to 14's files; re-render to a new folder.
- [ ] **Step 3:** Quality pass with the `impeccable` skill on the renders (contrast, alignment,
  rhythm); fix what it finds that the spec agrees with.
- [ ] **Step 4:** Copy the final set to `docs/design/r4/renders-1/` only after asking the master
  (that path is outside my owned paths); five checks green, commit
  `feat(render): render pages offscreen for review`.

### Task 16: Services and fixtures in the app

**Files:**
- Create: `crates/plith/src/services.rs` (start, subscribe, drain), `crates/plith/src/services/fixture.rs`, `crates/plith/src/model.rs` + `model/tests.rs` (service types to core facts), `crates/plith/src/pages.rs` (builds `PageContent` and scene shapes)
- Modify: `crates/plith/src/app/state.rs`, `crates/plith-core/src/launch.rs` (`--fixture banana|potato|bare|none`)

**Interfaces:**
- Produces: `pub trait Services` with `drain(&mut self) -> Changes` (bit set: audio, media,
  system), `facts(&self) -> (AudioFacts, MediaFacts, SystemFacts, PillFacts)`, `levels(&self) ->
  LiveValues`, and the write calls behind `Action`; `RealServices` (R2 crates) and
  `FixtureServices` (fixtures from Task 15; writes change the fixture only).
- Startup: one `plith-services` thread per service calls `subscribe()` first, then `start()`,
  then `snapshot()`, then `set_waker(ui_waker.wake)`; the UI thread never blocks on `start()`.
  `Wake` drains every receiver with `try_recv` (latest wins), rebuilds only the changed pages and
  publishes content (no publish when equal, Task 3). A UI timer at 4 Hz runs only while expanded
  on the audio page to feed `Activity`; it is killed on collapse.
- Clock: a one-shot timer to the next minute boundary while expanded.

- [ ] **Step 1: Failing tests:** `model_copies_vm_snapshot`, `wake_with_no_change_publishes_nothing`
  (fixture + mailbox generation), `activity_timer_only_on_audio_page`, `fixture_writes_stay_in_fixture`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `--idle-check 60 --fixture banana` through heavy-run: 0 render wakes while
  collapsed with the fixture ticking.
- [ ] **Step 5:** Five checks green, commit `feat(app): feed pages from the services`.

### Task 17: Input on pages

**Files:**
- Modify: `crates/plith-shell/src/window.rs` (`ShellEvent::Click { x, y }` in island DIP;
  `WM_MOUSELEAVE` through `TrackMouseEvent` as `ShellEvent::Leave` if question 10 says so),
  `crates/plith/src/app/state.rs`
- Create: `crates/plith-core/src/input.rs` + `input/tests.rs`

**Interfaces:**
- Produces: `pub fn route_click(scene: &Scene, layers: &PageLayers, targets: &[Vec<HitTarget>;
  4], x, y) -> ClickResult` (`Expand`, `Action(Action)`, `Nothing`); the page on screen is the
  incoming layer once its opacity is past 0.5, else the outgoing one. The UI thread keeps the
  last published targets and reads the last drawn page layers from the shape cell (the render
  thread adds `pages` to `ShapeFrame`). `pub struct CollapseTimer` (pure: `leave(now)`,
  `enter()`, `due(now) -> bool`, delay 400 ms) if question 10 is answered that way.

- [ ] **Step 1: Failing tests:** `click_on_pill_expands`, `click_during_turn_hits_incoming_page`,
  `click_on_empty_space_cancels_pick`, `leave_then_return_cancels_collapse`.
- [ ] **Step 2:** Run; expect failures.
- [ ] **Step 3:** Implement; every `Action` that writes goes through `Services`, so with
  `--fixture` nothing touches hardware.
- [ ] **Step 4:** Five checks green, commit `feat(app): handle clicks on pages`.

### Task 18: Demo and the console measurement

**Files:**
- Modify: `crates/plith-core/src/demo.rs` (named demo `pages`: expand; page 1, 2, 3, 0 with
  waits; a content swap mid-turn via a new `DemoStep::Content(u8)` that switches fixture
  variants; collapse; repeat 3), `crates/plith/src/demo.rs`

- [ ] **Step 1: Failing tests:** `pages_demo_parses`, `content_step_out_of_range_rejected`.
- [ ] **Step 2:** Implement; five checks green; commit `feat(demo): add the pages demo`.
- [ ] **Step 3:** Ask the master (batched with any other on-screen ask) for a guarded console run:
  `plith --demo pages --exit-after-demo --fixture banana` (about 60 s), release build, then the
  same with the island open on the audio page for 20 s of ambient. Acceptance from the log:
  every `motion` line 238+ fps, 0 to 1 hitches, draw p99 under 1 ms; the `ambient` line at
  60 fps with 0 hitches; Berke's words on the look recorded verbatim.
- [ ] **Step 4:** Record the numbers in the plan's progress log; fix and re-run if the bar is
  missed (systematic-debugging first).

### Task 19: Handoff

- [ ] **Step 1:** project-docs handoff on my branch: progress log in this plan, honest banner,
  ADR drafts ready, tree clean, branch pushed.
- [ ] **Step 2:** Message the master: what is done, commits, the record path, the next job
  (R5 shelf, live hardware write check, themes switch in settings R7).

## Questions for the master (batched, with recommendations)

1. Spec open item 1, bottom radius: **24** as in the mock (40 reads soft next to square stations).
2. Open item 2, Potato density: **draw Live and Candidate routes, Idle only for the picked
   source; rows above 5 shrink pitch to 40 DIP**, page capped at 420 high.
3. Open item 3, pill mute: **keep `KAPALI` in the value slot** (it fits at 10 px Mono in the mock);
   revisit after the Task 15 render.
4. Open item 4, levels: the API exists (`VoicemeeterService::levels()`, pulled per ambient frame);
   traces sampled at 30 Hz into an 8 s ring. No question unless you disagree.
5. Open item 5, route writes: **built, exercised only on fixtures**; one guarded write session with
   Berke later (toggle one route on Banana and back).
6. Open item 6, high contrast: **out of R4**.
7. Fonts: **bundle Public Sans and Martian Mono (both SIL OFL 1.1) in
   `crates/plith-render/assets/fonts/` with `OFL.txt`**, fetched from their upstream GitHub
   releases; fallback Segoe UI Variable and Cascadia Mono. Yes to the download and the asset?
8. Media output picker: per-app output routing needs the undocumented `IAudioPolicyConfig`.
   **R4 shows the route read-only** (source to the default endpoint, or to the Voicemeeter bus
   when Voicemeeter runs); the picker becomes a ROADMAP item.
9. RAF tab in R4: **station present, page shows the drawn empty siding**; drag and drop stays R5.
10. Expand and collapse: today a click toggles. **Click on the pill expands; leaving the island
    for 400 ms collapses; a click on empty page space does not collapse.** No hover-to-expand
    in R4.
11. The island never takes activation, so `ESC VAZGEÇER` cannot work. **Cancel by clicking the
    picked source again or empty diagram space; footer `ÇIKIŞA TIKLA: YOL KURULUR · KAYNAĞA
    TEKRAR TIKLA: VAZGEÇ`.** Copy change needs your or Berke's yes.
12. Value changes: **click on a ruler sets the value (volume, brightness, seek); no drag or wheel
    in R4.**
13. `docs/design/r4/` for the Task 15 renders is outside my paths: may I write there, or do you
    want them attached to my handoff only?

## Progress log

- 2026-11-15: plan written; the master approved Tasks 1 to 8 and answered Q4 to Q9, Q12, Q13;
  Q1, Q2, Q3, Q10, Q11 go to Berke and gate Task 9 onward. Page radius stays 40 until Q1.
- 2026-11-15: Task 1 done (P5 draft in `pages-r4-adr-drafts.md`).
