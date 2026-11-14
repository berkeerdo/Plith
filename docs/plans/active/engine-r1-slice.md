# R1 Engine Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An empty dark island on the primary monitor that morphs (width, height, radius and page
turns are springs), driven by a vblank frame clock on a render thread that parks at rest, with
hit testing that follows the shape every frame, the topmost fight, a `motion` log line per
motion, a demo driver and an idle check.

**Architecture:** Pure logic (springs, timeline, motion stats, scene handoff, demo scripts, retry
and fallback rules) lives in `plith-core` and is unit tested first. `plith-render` owns D3D11,
Direct2D and DirectComposition on one render thread whose loop is generic over a frame source
and a painter, so its parking is tested with fakes. `plith-shell` owns one notch window on the UI
thread. `crates/plith` composes them and writes `%LOCALAPPDATA%\Plith\plith.log`.

**Tech Stack:** Rust 1.98.1, edition 2024, `windows` 0.62.2 (signatures checked in the crate
source and context7), std only in `plith-core` (no new dependencies anywhere).

**Spec:** `docs/specs/2026-11-14-rust-rewrite-design.md` (sections 2 G1 and G2, 6, 9 step 2, 10,
11). Baseline numbers: `docs/research/2026-11-14-oyenza-engine.md` sections 1, 2, 4, 5.

## Global Constraints

- Limits: 500 source, 150 entry (`main.rs` composes only), 800 test, 80 per function.
- `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`; inline tests move to `foo/tests.rs`.
- `plith-core` has no `windows` dependency; `plith-render` and `plith-shell` use only
  `plith-core` among ours and never each other (`scripts/check-deps.mjs`).
- The UI thread never draws; the render thread owns every graphics object.
- No API from the render thread that sends a synchronous message to the UI thread
  (`SetWindowRgn`, `SetWindowPos`, `SendMessage`): it posts (`PostMessageW`) instead.
- All units in DIP inside `plith-core`; pixels only at the Win32 edge (`dip * scale`).
- Done = green `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `node scripts/structure-check.mjs`, `node scripts/check-deps.mjs`,
  each through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- No script moves the mouse or types keys. Motions come from `--demo`.
- No `rm` on variable, glob or command-substitution paths; no `bash -c`, `eval` or nested
  one-liners; commit bodies via `git commit -F <file>`. Never run `bash` from PowerShell.
- English everywhere, no em dash, no AI attribution; Conventional Commits, subject max 50 chars.
- Starting `plith.exe` visibly on Berke's screen needs the master's relayed yes (Task 10 batch).

## Review Focus

1. **Turnaround on a shrinking axis** (collapse interrupted by expand, or the reverse, at any
   frame): the shape stays continuous, never drops below the rest size, velocity carries.
   Test: `island::tests::reversal_mid_collapse_is_continuous_and_above_rest` (Task 2).
2. **A long gap between frames** (GPU hitch, sleep and resume, debugger): the timeline resyncs
   after 30 periods and a spring step never takes more than 0.1 s.
   Tests: `timeline::tests::gap_over_30_periods_resyncs`, `spring::tests::dt_is_clamped` (1, 4).
3. **Refresh rate or DPI change while running** (240 to 60 Hz, 100 to 150 percent): the period
   follows the new nominal rate; the surface is relaid at rest only, never mid-morph.
   Tests: `timeline::tests::set_nominal_rebases_period`, `layout::tests::layout_scales_with_dpi`
   (Tasks 4, 2); relayout-at-rest is checked in Task 9's manual list.
4. **Device removed** (driver update, TDR): the renderer recreates its device, and after 3
   failures inside 600 good frames falls back to WARP instead of looping.
   Test: `gpu_health::tests::three_failures_fall_back_to_warp` (Task 6).
5. **Remote desktop session**: the clock is the timer, the motion line says `clock timer`, and
   nothing about motion is claimed from it.
   Test: `clock_choice::tests::remote_session_uses_timer_only` (Task 7).
6. **Display off, session lock, a fullscreen app** (added by the master): each parks the clock
   with zero render wakes, same as at rest, even mid-motion (the motion snaps to its target and
   its report is dropped). On return, the island shows the latest scene with no catch-up motion.
   Tests: `scene::tests::any_suppression_reason_suppresses` (Task 5),
   `thread::tests::suppressed_scene_parks_even_mid_motion` (Task 7); the three signals are
   checked by hand in Task 10.

---

## Shapes for the slice (all DIP, used by tests and the demo)

Placeholders approved by the master 2026-11-14; they live only in `plith-core/src/shapes.rs`.

| Name | Width | Height | Radius |
|---|---|---|---|
| `REST` | 200 | 32 | 16 |
| `PAGES[0]` | 620 | 216 | 40 |
| `PAGES[1]` | 620 | 320 | 40 |
| `PAGES[2]` | 480 | 180 | 40 |

`PAGES[2]` is narrower and shorter than `PAGES[1]`, and `PAGES[1]` is taller than `PAGES[0]` at
the same width, so page turns exercise mixed per-axis directions. Floating style: `TOP_GAP = 8`
below the work-area top, `SURFACE_MARGIN = 20`, `OVERSHOOT = 1.05`. Island fill `#000000`; the
page placeholder is a rounded bar `white(0.16)`, 120 x 12 r6, 24 DIP below the island top,
centred, so page turns are visible without text.

## Tunings (stiffness, damping), unit mass, from the research doc section 4 (floating table)

| Const | k | c | zeta |
|---|---|---|---|
| `GROW_W` | 500 | 32.5 | 0.73 |
| `GROW_H` | 250 | 23 | 0.73 |
| `SHRINK_W` | 400 | 36 | 0.90 |
| `SHRINK_H` | 560 | 43 | 0.91 |
| `RADIUS_GROW` | 1200 | 69.3 | 1.0 |
| `RADIUS_SHRINK` | 150 | 24.5 | 1.0 |
| `PAGE_OFFSET` | 380 | 35.1 | 0.90 (new: Oyenza tweens pages) |
| `OPACITY` | 700 | 52.9 | 1.0 |

Page turn: incoming page slides from `dir * PAGE_SLIDE` (20 DIP) to 0 and fades 0 to 1; the
outgoing page fades to 0 and slides to `-dir * PAGE_SLIDE`.

---

### Task 1: Spring and tunings (`plith-core`)

**Files:**
- Create: `crates/plith-core/src/spring.rs`, `crates/plith-core/src/spring/tests.rs`,
  `crates/plith-core/src/tuning.rs`, `crates/plith-core/src/tuning/tests.rs`
- Modify: `crates/plith-core/src/lib.rs` (add `pub mod spring; pub mod tuning;`)

**Interfaces:**
- Produces: `Tuning { pub stiffness: f64, pub damping: f64 }`, `Tuning::new(k, c) -> Tuning`
  (`const`), `Tuning::zeta(&self) -> f64`; the eight constants above in `tuning`.
  `Spring::new(value: f64, tuning: Tuning) -> Spring` (eps value 0.05, velocity 0.2),
  `Spring::with_eps(self, value_eps: f64, velocity_eps: f64) -> Spring`,
  `retarget(&mut self, target: f64, tuning: Tuning)` (keeps value and velocity),
  `step(&mut self, dt: f64) -> bool` (true when settled; snaps to target when settled),
  `clamp_min(&mut self, floor: f64)` (value below floor becomes floor, negative velocity becomes
  0), `value()`, `velocity()`, `target()`, `is_settled()`.

- [x] **Step 1: Write the failing tests**

```rust
// tuning/tests.rs
#[test] fn zetas_match_the_table() {
    for (t, z) in [(GROW_W, 0.73), (GROW_H, 0.73), (SHRINK_W, 0.90), (SHRINK_H, 0.91),
                   (RADIUS_GROW, 1.0), (RADIUS_SHRINK, 1.0), (PAGE_OFFSET, 0.90), (OPACITY, 1.0)] {
        assert!((t.zeta() - z).abs() < 0.01, "{t:?} zeta {}", t.zeta());
    }
}
// spring/tests.rs (h = 1/240 fixed sub-step, semi-implicit Euler)
#[test] fn grow_overshoots_between_1_and_6_percent_and_settles_under_1_2_s()  // 0 -> 100 with GROW_W
#[test] fn critical_spring_never_overshoots()                                // 0 -> 100, RADIUS_GROW
#[test] fn retarget_keeps_value_and_velocity()     // step 0.05 s, retarget, value/velocity equal before and after
#[test] fn settled_snaps_to_target()               // after settle, value() == target() exactly
#[test] fn dt_is_clamped()                         // step(5.0) moves no further than ten step(0.01) calls
#[test] fn clamp_min_stops_negative_velocity()     // value 5, velocity -50, clamp_min(10): value 10, velocity 0
#[test] fn opacity_eps_settles_tighter()           // with_eps(0.002, 0.01): 0 -> 1 ends within 0.002
```

- [x] **Step 2: Run, expect FAIL** `cargo test -p plith-core` (unresolved `spring`, `tuning`).
- [x] **Step 3: Implement.** Force `-k (x - target) - c v`, fixed sub-step `1/240` s, `dt`
  clamped to `0..=0.1`, leftover below one sub-step carried to the next call (an accumulator),
  settled when `|x - target| < eps_value && |v| < eps_velocity`.
- [x] **Step 4: Run, expect PASS**, then the five checks through heavy-run.
- [x] **Step 5: Commit** `feat(core): add springs and tunings`, push, tick this box.

### Task 2: Island motion, geometry and surface layout (`plith-core`)

**Files:**
- Create: `crates/plith-core/src/island.rs` (+ `island/tests.rs`),
  `crates/plith-core/src/geometry.rs` (+ `geometry/tests.rs`),
  `crates/plith-core/src/shapes.rs` (the one constants table: every number in "Shapes for the
  slice" lives here and nowhere else, so R3 is a one-file change),
  `crates/plith-core/src/layout.rs` (+ `layout/tests.rs`)

**Interfaces:**
- Consumes: `Spring`, `Tuning`, tuning constants (Task 1).
- Produces:
  - `geometry`: `IslandShape { width, height, radius: f64 }` (`Copy`, `PartialEq`),
    `IslandShape::area()`, `IslandShape::contains(&self, x: f64, y: f64) -> bool` (point relative
    to the island's top-left, rounded-rect test with the drawn radius).
  - `shapes`: consts `REST`, `PAGES: [IslandShape; 3]`, `TOP_GAP`, `SURFACE_MARGIN`,
    `OVERSHOOT`, `FILL` and `PLACEHOLDER` (colour, size, radius, top offset as plain numbers).
  - `island`: `IslandMotion::new(rest: IslandShape) -> IslandMotion`,
    `retarget(&mut self, target: IslandShape)`, `step(&mut self, dt: f64) -> bool`,
    `shape(&self) -> IslandShape` (radius clamped to `min(radius, height/2, width/2)`),
    `is_settled(&self) -> bool`.
  - `layout`: `RectPx { x, y, width, height: i32 }`,
    `SurfaceLayout::new(monitor: RectPx, work_top_px: i32, scale: f64) -> SurfaceLayout` with
    `surface: RectPx`, `scale: f64`, `island_origin(&self, shape: IslandShape) -> (f64, f64)`
    (DIP, relative to the surface: centred, `TOP_GAP` from the top).

- [x] **Step 1: Write the failing tests**

```rust
// island/tests.rs, helper run(m, secs) steps 1/240 and collects shapes
#[test] fn per_axis_tuning_follows_each_axis_direction()
    // REST -> PAGES[1], then retarget PAGES[2] mid-flight: width shrinks (SHRINK_W), height
    // grows (GROW_H): width never undershoots 480, height overshoots 180 by > 0.
#[test] fn retarget_keeps_velocity_on_every_axis()
#[test] fn shrinking_axis_never_goes_below_target()          // PAGES[0] -> REST, all samples >= REST
#[test] fn reversal_mid_collapse_is_continuous_and_above_rest()
    // PAGES[0] -> REST for 0.1 s, then -> PAGES[0]: no frame-to-frame jump > 15 DIP at 240 Hz,
    // never below REST on any axis.
#[test] fn expand_settles_between_0_6_and_0_9_s()             // REST -> PAGES[0] (spec ~730 ms)
#[test] fn radius_never_exceeds_half_height()
// geometry/tests.rs
#[test] fn contains_rejects_the_corner_outside_the_arc()      // (1, 1) on a 620x216 r40 is out
#[test] fn contains_accepts_edges_and_centre()
// layout/tests.rs
#[test] fn surface_is_even_width_and_centred_on_the_monitor_centre_pixel()
#[test] fn surface_fits_the_largest_page_with_overshoot_and_margin() // width >= 620*1.05 + 40 DIP
#[test] fn layout_scales_with_dpi()                            // scale 1.5: surface px = DIP * 1.5, rounded up to even
```

- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement.** Retune per axis at retarget time from that axis's direction
  (`target < current value` is shrinking). Floor landing: a shrinking axis sets
  `floor = target`; a growing axis keeps its previous floor; after every step each axis calls
  `clamp_min(floor)`. Radius uses `RADIUS_GROW` when its target grows, else `RADIUS_SHRINK`.
- [x] **Step 4: Run, expect PASS**, five checks.
- [x] **Step 5: Commit** `feat(core): add island motion and layout`, push, tick.

### Task 3: Page-turn springs (`plith-core`)

**Files:** Create `crates/plith-core/src/page.rs` (+ `page/tests.rs`).

**Interfaces:**
- Consumes: `Spring`, `PAGE_OFFSET`, `OPACITY`.
- Produces: `PageLayer { pub page: usize, pub offset: f64, pub opacity: f64 }`,
  `PageTurn::new(page: usize) -> PageTurn`, `turn_to(&mut self, page: usize)`,
  `step(&mut self, dt: f64) -> bool`, `layers(&self) -> Vec<PageLayer>` (outgoing first),
  `current(&self) -> usize`, const `PAGE_SLIDE = 20.0`.

- [x] **Step 1: Write the failing tests**

```rust
#[test] fn turn_forward_slides_in_from_the_right_and_fades_in()  // first step: offset > 0, opacity < 0.2
#[test] fn outgoing_layer_is_dropped_once_invisible()             // after settle, layers().len() == 1
#[test] fn reversal_mid_turn_keeps_velocity()
    // 0 -> 1 for 0.1 s, then back to 0: layer for page 0 resumes from its current offset and
    // opacity (no jump), page 1 becomes outgoing with its velocity kept.
#[test] fn turn_settles_under_0_6_s()                             // spec page ~450 ms
#[test] fn turn_to_current_page_is_a_no_op()
```

- [x] **Step 2: Run, expect FAIL.**  **Step 3: Implement.** Opacity springs use
  `with_eps(0.002, 0.01)`. A turn to the page that is outgoing swaps the two layers and retargets
  both, keeping their springs.
- [x] **Step 4: Run, expect PASS**, five checks.  **Step 5: Commit**
  `feat(core): add page-turn springs`, push, tick.

### Task 4: Vsync timeline, frame stamping and motion stats (`plith-core`)

**Files:** Create `crates/plith-core/src/timeline.rs` (+ tests), `crates/plith-core/src/motion.rs`
(+ tests).

**Interfaces:**
- Produces:
  - `Tick { pub vblank: f64, pub period: f64, pub periods: u32 }` (seconds).
  - `VsyncTimeline::new(nominal_hz: f64)`, `lock(&mut self, raw: f64) -> Option<Tick>`
    (`None` for a spurious wake-up, `n == 0`), `reset(&mut self)`,
    `set_nominal(&mut self, hz: f64)`, `needs_rate_reread(&self) -> bool`.
  - `FrameStamper::default()`, `stamp(&mut self, tick: Tick) -> (f64, f64)` returning
    `(anim_t, dt)`, `reset(&mut self)`.
  - `MotionKind { Expand, Collapse, Page, Resize }` with `as_str()`;
    `MotionKind::classify(from: IslandShape, to: IslandShape, page_changed: bool)`
    (`to == REST` Collapse, `from == REST` Expand, `page_changed` Page, else Resize).
  - `MotionStats::new(kind: MotionKind, hz: f64, clock: &'static str)`,
    `record_frame(&mut self, anim_t: f64, draw_ms: f64, present_ms: f64)`,
    `note_retarget(&mut self)`, `finish(&self) -> MotionReport`; `MotionReport` implements
    `Display` with the exact line below.

Line format (pinned; the log writer prefixes a local timestamp):
`motion expand: 175 frames in 731 ms (239.4 fps at 240 Hz), 0 hitches, worst gap 4.4 ms; draw p50 0.31 p99 0.62 ms; present p50 0.10 p99 0.22 ms; clock dcomp; retargets 0`

- [x] **Step 1: Write the failing tests**

```rust
// timeline/tests.rs
#[test] fn jittered_wakeups_snap_to_the_period_grid()   // raw = k/240 + jitter(0..3 ms): vblank deltas are whole periods
#[test] fn a_wakeup_up_to_three_quarters_late_belongs_to_its_vblank()
#[test] fn spurious_second_wakeup_returns_none()
#[test] fn gap_over_30_periods_resyncs()                 // next tick has periods == 1 and vblank == raw
#[test] fn period_stays_within_3_percent_of_nominal()
#[test] fn set_nominal_rebases_period()                  // 240 -> 60: next tick period == 1/60 (within 3 %)
// stamper
#[test] fn first_step_after_rest_is_exactly_one_period()
#[test] fn every_step_is_a_whole_number_of_periods()
// motion/tests.rs
#[test] fn report_line_matches_the_pinned_format()       // 240 frames at 1/240: "240 frames in 1000 ms (240.0 fps at 240 Hz), 0 hitches"
#[test] fn a_gap_over_1_5_periods_is_a_hitch()
#[test] fn percentiles_are_nearest_rank()
#[test] fn classify_covers_the_four_kinds()
```

- [x] **Step 2: Run, expect FAIL.**
- [x] **Step 3: Implement** the research doc section 1 algorithm: `n = floor(elapsed/period +
  0.25)`; resync when the gap exceeds 30 periods. **Amended 2026-11-14 (Task 4 finding):** a
  vblank wait only returns late, never early, so the grid tracks the earliest wake-up, not the
  mean: phase error `e < 0` snaps the grid to `raw`, `e >= 0` moves it by `0.005 * e`; the
  period stays at the nominal rate (no period estimator; small nominal error is absorbed by the
  snaps). Symmetric gain 0.1 parked the grid at the mean lateness and dropped valid early
  wake-ups as spurious. Late gain is 0.005. Off-rhythm score +1 when `e > 0.5 period` or
  `n > 1` (a missed vblank), -1 (floor 0) otherwise, `needs_rate_reread` when it passes 20.
  Stamp: the first stamp after `reset` anchors `anim_t = vblank + period` with `dt` one period;
  after that `dt = periods * period` and `anim_t = last_anim + dt` (never a vblank difference). Duration = `last_anim - first_anim + period`; hitch = gap > 1.5 periods.
- [x] **Step 4: Run, expect PASS**, five checks.  **Step 5: Commit**
  `feat(core): add vsync timeline and motion stats`, push, tick.

### Task 5: Scene handoff, launch args and demo scripts (`plith-core`)

**Files:** Create `crates/plith-core/src/scene.rs`, `launch.rs`, `demo.rs` (each + tests).

**Interfaces:**
- Produces:
  - `Suppression { pub display_off: bool, pub locked: bool, pub fullscreen: bool }` (`Copy`,
    `Default` all false), `Suppression::any(&self) -> bool`.
  - `Scene { pub expanded: bool, pub page: usize, pub suppression: Suppression }` (`Copy`,
    `Default` = collapsed, page 0, nothing suppressed),
    `Scene::target(&self) -> IslandShape` (`REST` or `PAGES[page]`).
  - `SceneMailbox` (latest wins; `Mutex<(Scene, u64)>` + `Condvar`): `new(Scene)`,
    `publish(&self, scene: Scene) -> u64`, `latest(&self) -> (Scene, u64)`,
    `wait_newer(&self, seen: u64) -> Option<(Scene, u64)>` (blocks; `None` after `stop`),
    `stop(&self)`.
  - `ShapeFrame { pub shape: IslandShape, pub generation: u64 }`, `ShapeCell` with
    `publish(&self, ShapeFrame)` and `latest(&self) -> Option<ShapeFrame>`.
  - `Launch { pub demo: Option<DemoScript>, pub idle_check_secs: Option<u64>, pub clock:
    ClockRequest, pub exit_after_demo: bool }`, `ClockRequest { Auto, Dcomp, Dxgi, Timer }`,
    `Launch::parse<I: IntoIterator<Item = String>>(args: I) -> Result<Launch, LaunchError>`
    for `--demo <name|script>`, `--idle-check <secs>`, `--clock <auto|dcomp|dxgi|timer>`,
    `--exit-after-demo`.
  - `DemoStep { Expand, Collapse, Page(usize), Wait(u64 /* ms */) }`,
    `DemoScript { pub steps: Vec<DemoStep>, pub repeat: u32 }`,
    `DemoScript::parse(&str) -> Result<DemoScript, DemoError>` (grammar: steps separated by
    `;`, `repeat N` allowed only last), `DemoScript::builtin(name: &str) -> Option<DemoScript>`.

Built-ins (pinned):
- `basic`: `expand; wait 1500; page 1; wait 1200; page 2; wait 1200; page 0; wait 1200; collapse; wait 1500; repeat 3`
- `interrupt`: `expand; wait 200; collapse; wait 150; expand; wait 1500; page 1; wait 120; page 0; wait 1500; collapse; wait 100; expand; wait 1500; collapse; wait 1500; repeat 3`

- [x] **Step 1: Write the failing tests**

```rust
#[test] fn any_suppression_reason_suppresses()             // each of the three flags alone makes any() true
#[test] fn wait_newer_wakes_on_publish_from_another_thread()
#[test] fn latest_wins_when_two_scenes_are_published_before_a_read()
#[test] fn stop_releases_a_blocked_waiter()
#[test] fn parse_reads_all_flags()
#[test] fn unknown_flag_is_an_error()
#[test] fn builtins_parse_and_page_indices_are_in_range()
#[test] fn repeat_must_be_last_and_page_index_must_exist() // "repeat 2; expand" and "page 3" are errors
```

- [x] **Step 2: FAIL. Step 3: Implement. Step 4: PASS**, five checks.
- [x] **Step 5: Commit** `feat(core): add scene handoff and demo scripts`, push, tick.

### Task 6: Device, composition target and painter (`plith-render`)

**Files:**
- Create: `crates/plith-render/src/device.rs`, `target.rs`, `paint.rs`,
  `crates/plith-core/src/gpu_health.rs` (+ tests), `crates/plith-render/tests/paint.rs`
- Modify: `crates/plith-render/Cargo.toml` (`windows` features), `crates/plith-render/src/lib.rs`

**Interfaces:**
- Consumes: `IslandShape`, `PageLayer`, `SurfaceLayout`.
- Produces:
  - `Gpu::new(prefer_warp: bool) -> windows::core::Result<Gpu>`: D3D11 hardware device with
    `D3D11_CREATE_DEVICE_BGRA_SUPPORT`, WARP fallback, `ID2D1Factory1` single-threaded,
    `ID2D1DeviceContext`, `IDXGIFactory2`, `IDCompositionDevice` (`DCompositionCreateDevice`).
  - `CompositionTarget::new(gpu: &Gpu, hwnd: HWND, layout: &SurfaceLayout) -> Result<_>`:
    `CreateSwapChainForComposition`, `B8G8R8A8_UNORM`, `FLIP_SEQUENTIAL`, premultiplied alpha,
    buffer count and latency per ADR proposal P3, DComp target on the HWND, one visual, one
    commit. `present(&mut self) -> windows::core::Result<()>`.
  - `Painter::paint(&mut self, ctx: &ID2D1DeviceContext, frame: &PaintFrame)` where
    `PaintFrame { pub shape: IslandShape, pub origin: (f64, f64), pub pages: Vec<PageLayer> }`;
    clears to transparent, fills the island `#000000`, draws each page placeholder with its
    offset and opacity, clipped to the island (a D2D layer with the island geometry, pushed only
    while the shape is unsettled).
  - `render_offscreen(width_px: u32, height_px: u32, scale: f64, frame: &PaintFrame) ->
    windows::core::Result<Vec<u8>>` (WARP, CPU-readable bitmap, BGRA premultiplied): the
    test hook.
  - `plith-core::gpu_health::GpuHealth`: `record(&mut self, ok: bool) -> GpuAction` with
    `GpuAction { None, Recreate, FallBackToWarp }`; failure count resets after 600 good frames.

- [x] **Step 1: Write the failing tests**

```rust
// crates/plith-render/tests/paint.rs (runs on WARP, CI-safe)
#[test] fn island_pixels_are_opaque_black_and_outside_is_transparent()
    // 400x300 px, scale 1.0, PAGES[2]-sized shape: centre pixel a == 255 and rgb == 0;
    // (0, 0) a == 0; the pixel 1 px inside the top-left corner arc's outside a == 0.
#[test] fn scale_1_5_draws_the_same_shape_larger()
#[test] fn page_placeholder_is_lighter_than_the_fill()
// plith-core gpu_health/tests.rs
#[test] fn one_failure_recreates()
#[test] fn three_failures_fall_back_to_warp()
#[test] fn failures_reset_after_600_good_frames()
```

- [x] **Step 2: FAIL.** **Step 3: Implement**, checking each `windows` call in the 0.62.2
  source (`~/.cargo/registry/src/*/windows-0.62.2/src/Windows/Win32/...`) and context7
  `/microsoft/windows-rs` before use. Every `unsafe` block has a `// SAFETY:` line.
- [x] **Step 4: PASS**, five checks.  **Step 5: Commit** `feat(render): add device and painter`,
  push, tick.

### Task 7: Frame clock and render thread (`plith-render`, opus)

**Files:**
- Create: `crates/plith-render/src/clock.rs`, `clock/dcomp.rs`, `clock/dxgi.rs`,
  `clock/timer.rs`, `crates/plith-render/src/thread.rs` (+ `thread/tests.rs`),
  `crates/plith-core/src/clock_choice.rs` (+ tests)

**Interfaces:**
- Consumes: Tasks 1 to 6.
- Produces:
  - core: `ClockKind { Dcomp, Dxgi, Timer }` with `as_str()` (`dcomp`, `dxgi`, `timer`);
    `clock_order(request: ClockRequest, remote: bool) -> Vec<ClockKind>` (Auto =
    `[Dcomp, Dxgi, Timer]`, remote = `[Timer]`, a named request puts it first then the rest);
    `ClockHealth::observe(&mut self, waited: f64, period: f64) -> bool` (true = demote after 3
    consecutive waits under 0.15 period).
  - render: `trait FrameSource { fn wait(&mut self) -> Result<f64, ClockError>; fn kind(&self)
    -> ClockKind; }` (returns raw seconds from a monotonic origin). Impls:
    `DcompClock` (`DCompositionWaitForCompositorClock(None, timeout)`), `DxgiClock`
    (`IDXGIOutput::WaitForVBlank` on the output for the notch monitor), `TimerClock`
    (high-resolution waitable timer, own `next_due`).
  - render: `trait Paint { fn paint(&mut self, frame: &PaintFrame) -> (f64, f64) /* draw ms,
    present ms */ }` implemented by the real painter + target.
  - render: `RenderSinks { pub on_shape: Box<dyn Fn(ShapeFrame) + Send>, pub on_motion:
    Box<dyn Fn(MotionReport) + Send> }`;
    `spawn(config: RenderConfig, mailbox: Arc<SceneMailbox>, sinks: RenderSinks) ->
    Result<RenderHandle, RenderError>` where `RenderConfig { pub hwnd: isize, pub layout:
    SurfaceLayout, pub clock: ClockRequest, pub remote: bool, pub hz: f64 }`;
    `RenderHandle::wake_count(&self) -> u64`, `RenderHandle::shutdown(self)`.
  - render: generic loop `run_loop<F: FrameSource, P: Paint>(...)` used by `spawn` and by tests.

Loop (the part the tests pin):
1. At rest the thread blocks only in `mailbox.wait_newer(seen)`; it never waits on the clock.
2. On a new scene: retarget `IslandMotion` / `PageTurn`, start `MotionStats` (kind from
   `MotionKind::classify`) or `note_retarget` if already moving, `timeline.reset()` and
   `stamper.reset()` when leaving rest.
3. Each iteration while moving: `source.wait()`, `timeline.lock()`, skip `None`, stamp, check
   `mailbox.latest()` for a newer scene, step springs by `dt`, paint, `on_shape` with the drawn
   shape, `record_frame`.
4. When everything settles: one last frame, `on_motion(stats.finish())`, back to step 1.
5. Each return from any wait increments the wake counter. When `ClockHealth` demotes, the loop
   switches to the next `clock_order` entry; the motion line's `clock` field shows the source
   that drew the motion (no separate clock line in R1).
6. The thread joins MMCSS `"Games"` (`AvSetMmThreadCharacteristicsW`); recorded in ADR P1.

- [x] **Step 1: Write the failing tests** (fakes: a `FakeClock` returning `k/240` with a
  counter, a `FakePaint` recording frames)

```rust
#[test] fn parks_at_rest_with_no_clock_waits()           // no scene published: 200 ms later clock waits == 0, wakes == 0
#[test] fn suppressed_scene_parks_even_mid_motion()      // expand, 20 frames, publish suppressed: no more clock waits, no report
#[test] fn one_expand_produces_one_motion_report()       // kind Expand, frames > 100, then parks
#[test] fn retarget_mid_motion_is_one_report_with_retargets_1()
#[test] fn first_frame_after_rest_advances_one_period()
#[test] fn on_shape_is_called_every_painted_frame()
#[test] fn three_fast_returns_demote_the_clock()         // FakeClock returning instantly: kind changes to the next
// core clock_choice/tests.rs
#[test] fn auto_prefers_dcomp_then_dxgi_then_timer()
#[test] fn remote_session_uses_timer_only()
#[test] fn named_request_goes_first()
```

- [x] **Step 2: FAIL.** **Step 3: Implement.** Thread name `plith-render`. All graphics objects
  are created inside the thread; only `Arc`s and the HWND value cross.
- [x] **Step 4: PASS**, five checks.  **Step 5: Commit**
  `feat(render): add frame clock and render thread`, push, tick.

### Task 8: Notch window, hit testing and the topmost fight (`plith-shell`)

**Files:**
- Create: `crates/plith-shell/src/process.rs`, `monitor.rs`, `window.rs`, `hit.rs`,
  `topmost.rs`, `crates/plith-core/src/topmost.rs` (+ tests), `docs/plans/active/engine-r1-spikes.md`
- Modify: `crates/plith-shell/Cargo.toml`, `crates/plith-shell/src/lib.rs`

**Interfaces:**
- Consumes: `SurfaceLayout`, `ShapeCell`, `ShapeFrame`, `IslandShape::contains`.
- Produces:
  - `init_process()`: `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`.
  - `MonitorInfo { pub rect: RectPx, pub work_top_px: i32, pub scale: f64, pub hz: f64 }`,
    `primary_monitor() -> MonitorInfo` (`EnumDisplaySettingsW` nominal rate, `GetDpiForMonitor`),
    `is_remote_session() -> bool` (`GetSystemMetrics(SM_REMOTESESSION)`).
  - `Shell::create(layout: SurfaceLayout, shapes: Arc<ShapeCell>, on_event: Box<dyn
    FnMut(ShellEvent)>) -> windows::core::Result<Shell>`, `Shell::hwnd(&self) -> isize`,
    `Shell::shape_poster(&self) -> ShapePoster` (`Send + Clone`; `post(ShapeFrame)` stores into
    the cell and `PostMessageW(WM_APP_SHAPE)`, coalesced by generation),
    `run_message_loop() -> i32`, `ShellEvent { Click, DisplayChanged, Suppression(Suppression) }`.
  - Suppression signals, all event driven (no polling): display state via
    `RegisterPowerSettingNotification(GUID_CONSOLE_DISPLAY_STATE)`, lock via
    `WTSRegisterSessionNotification` (`WTS_SESSION_LOCK` / `UNLOCK`), fullscreen via an AppBar
    registration that never calls `ABM_SETPOS` (`ABN_FULLSCREENAPP`). The app republishes the
    scene with the new `Suppression`.
  - Window: `WS_POPUP`, `WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE |
    WS_EX_NOREDIRECTIONBITMAP`, placed once at `layout.surface`, never moved during a morph.
  - core `TopmostFight`: `on_foreground(&mut self) -> TopmostAction`, `on_timer(&mut self) ->
    TopmostAction`, `TopmostAction { Raise { retry_in_ms: Option<u32> }, Idle }`, consts
    `RETRIES = 4`, `RETRY_MS = 500`. Shell: `SetWinEventHook(EVENT_SYSTEM_FOREGROUND,
    WINEVENT_OUTOFCONTEXT)` plus `SetTimer`, `SetWindowPos(HWND_TOPMOST, NOMOVE | NOSIZE |
    NOACTIVATE)`; the first raise per foreground change logs the class of the window above.

- [ ] **Step 1: Hit-testing spike** (ADR proposal P2). Build three candidates behind a
  `--hit <region|mask|split>` debug flag, each fed by `WM_APP_SHAPE` on the UI thread:
  - `region`: `SetWindowRgn` on the single DComp window, rounded region outset by 1 px so the
    anti-aliased edge is not clipped.
  - `split`: a second, input-only window under the visual one, `SetWindowRgn` per shape; the
    visual window is `WS_EX_TRANSPARENT`.
  - `mask`: Oyenza's layered 1/255 alpha mask window, updated every frame (not 30 Hz).
  Record per candidate in `engine-r1-spikes.md`: UI-thread cost per update (ms), whether the
  edge is clipped, and the Task 10 click results. Default to `region` until Task 10 decides.
- [x] **Step 2: Write the failing core tests**

```rust
#[test] fn foreground_change_raises_now_and_schedules_four_retries()
#[test] fn retries_stop_after_four()
#[test] fn a_new_foreground_change_restarts_the_count()
```

- [x] **Step 3: FAIL. Step 4: Implement**; `WM_DPICHANGED` and `WM_DISPLAYCHANGE` raise
  `DisplayChanged` and the app relays the surface at rest only; `WM_LBUTTONUP` inside the shape
  raises `Click`; `WM_MOUSEACTIVATE` returns `MA_NOACTIVATE`.
- [x] **Step 5: PASS**, five checks.  **Step 6: Commit**
  `feat(shell): add notch window and hit testing`, push, tick.

### Task 9: App composition, log, demo driver and idle check (`crates/plith`)

**Files:**
- Create: `crates/plith/src/app.rs`, `log.rs`, `demo.rs`, `idle.rs`
- Modify: `crates/plith/src/main.rs` (parse `Launch`, call `app::run`, exit code; under 150
  lines), `crates/plith/Cargo.toml`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `log::LogFile::open() -> io::Result<LogFile>` at `%LOCALAPPDATA%\Plith\plith.log`
    (`SHGetKnownFolderPath(FOLDERID_LocalAppData)`), `line(&self, text: &str)` prefixing
    `YYYY-MM-DD HH:MM:SS.mmm` local time; appends, never truncates.
  - `demo::spawn(script: DemoScript, mailbox: Arc<SceneMailbox>, done: Sender<()>)`: a thread
    that publishes scenes and sleeps for `Wait`; never synthesises input.
  - `idle::run(secs: u64, render: &RenderHandle, log: &LogFile) -> bool`: waits `secs` at rest,
    logs `idle 60 s: 0 render wake-ups, 0.0 ms process CPU` (CPU from `GetProcessTimes`), true
    when wakes == 0.
  - `app::run(launch: Launch) -> i32`: `init_process`, monitor, layout, `Shell`, render
    `spawn` with sinks wired to `ShapePoster` and `LogFile`, click toggles `expanded`, demo or
    idle mode, message loop, clean shutdown (`mailbox.stop()`, join).

- [ ] **Step 1: Write the failing test** `log::tests::line_has_timestamp_and_appends` (writes to
  a temp dir via `LogFile::open_at(path)`).
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS**, five checks.
- [ ] **Step 5: Smoke run. Approved by Berke via the master 2026-11-14, on conditions: run only
  when `SHQueryUserNotificationState` is not `QUNS_RUNNING_D3D_FULL_SCREEN`, `QUNS_BUSY` or
  `QUNS_PRESENTATION_MODE` (else skip and retry later); each run at most about 1 minute; no
  mouse or keys; afterwards confirm no `plith.exe` is left running:**
  `cargo run -p plith --release -- --demo basic --exit-after-demo` then
  `cargo run -p plith --release -- --idle-check 60`. Expected: 15 motion lines (`basic` has 5
  motions, `repeat 3`) and `idle 60 s: 0 render wake-ups`. If this session is not a console
  session, every motion number is labelled "not measured (RDP or background)".
- [ ] **Step 6: Commit** `feat(app): compose the engine slice`, push, tick.

### Task 10: Console measurement with Berke (master arranges)

Asked through the master in one batch. Nothing here moves the mouse; Berke clicks by hand.

- [ ] Clock comparison: `--demo basic --clock dcomp`, then `dxgi`, then `timer`; pick the
  default by motion lines (ADR P1).
- [ ] G1: every motion line at 238+ fps at 240 Hz, 0 to 1 hitches, draw p99 under 1 ms;
  expand 600 to 900 ms, collapse 500 to 800 ms, page 300 to 600 ms. Paste the lines here.
- [ ] G2: `--idle-check 60` prints 0 render wake-ups.
- [ ] Review Focus 6: during `--demo basic`, Berke locks the session (Win+L), turns the display
  off, and starts a fullscreen app; the log shows the motion cut short with no report and no
  wakes until return.
- [ ] Hit testing per candidate (`--hit region|split|mask`): Berke clicks just outside the
  corner arc and outside the island during `--demo interrupt` (reaches the window beneath), and
  inside (toggles); pen and touch if this PC has them, else recorded as untested (ADR P2).
- [ ] Topmost over two borderless games Berke names: island stays above for 2 minutes each;
  the log shows the raise lines (spec section 10).
- [ ] Interrupted motion feel: Berke's verdict on `--demo interrupt` recorded verbatim.
- [ ] Handoff: `project-docs` handoff on this branch, plan banner, message the master.

---

## Proposed ADRs (the master files them; each is decided by Task 10 numbers)

| # | Builder call (spec section 11) | Proposal | Decided by |
|---|---|---|---|
| P1 | Frame clock source | `DCompositionWaitForCompositorClock` first, `WaitForVBlank` second, waitable timer last; demote after 3 waits under 0.15 period; MMCSS `Games` on the render thread; timer only over RDP; phase lock tracks the earliest wake-up with a nominal period (deviates from Oyenza's symmetric phase and period gains, Task 4 finding) | Task 10 clock comparison |
| P2 | Hit-testing mechanism | One DComp window with a 1 px outset `SetWindowRgn` applied on the UI thread from a posted shape, every painted frame (Oyenza: 30 Hz, two windows) | Task 8 spike + Task 10 clicks |
| P3 | Swap chain buffers and latency | 2 buffers, `SetMaximumFrameLatency(1)`, `Present(1, 0)` after the clock wait; 3 buffers if draw ever waits on DWM | Task 10 present p99 |
| P4 | Scene handoff | Latest-wins mailbox (`Mutex` + `Condvar` + generation), not a channel: intermediate targets are meaningless and the render thread parks on the condvar | Task 7 tests |
| P5 | UI layer shape | Deferred to R4 (R1 has no widgets); recorded so R4 opens with it | n/a |

Service polling rates belong to lane `services`.

## Not in this slice (and where it goes)

Offscreen content layers with group opacity (R4, when text exists), hover dwell and hover lift
(R4), a named-pipe demo command (R4 if `--demo` is not enough), multi-monitor choice and
settings (R7), fullscreen hiding of the window itself (R4; R1 only parks the clock), OLE drop
target (R5).
