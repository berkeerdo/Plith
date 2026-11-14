# R1b Engine Under Load Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The island keeps 0 to 1 hitches per motion with a borderless game behind, its motion
durations are honest and tuned by Berke's eye, the topmost fight is explained and quiet, the
dropped hit modes are gone and a render-thread panic or close can no longer freeze or race.

**Architecture:** Measure first, then pull levers behind launch flags so one console session with
Deadlock compares them side by side (as the R1 clock comparison did); only the winners become
defaults. Pure parts (lever parsing, hitch detail, visible settling, tuning presets, raise
debounce) live in `plith-core` with unit tests; Windows calls stay in `plith-render` and
`plith-shell`.

**Tech Stack:** Rust 1.98.1, edition 2024, `windows` 0.62.2 (check every new API with context7
and the crate source before use), std only in `plith-core`.

**Spec:** `docs/specs/2026-11-14-rust-rewrite-design.md` (G1, section 6 frame clock and surface).
Inputs: `docs/plans/done/engine-r1-slice.md` (Task 10, Handoff), ADR 0003 to 0006,
`docs/quality/2026-11-14-engine-review.md` (L1, L2, L4, L5, L8, L10).

## Global Constraints

- Limits: 500 source, 150 entry (`main.rs` composes only), 800 test, 80 per function.
- `src/foo.rs` + `src/foo/bar.rs`, never `mod.rs`; tests in `foo/tests.rs`.
- `plith-core` has no `windows` dependency; `plith-render` and `plith-shell` use only
  `plith-core` among ours and never each other (`scripts/check-deps.mjs`).
- The UI thread never draws; the render thread never `SendMessage`s the UI thread.
- Done per task = green `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace`, `node scripts/structure-check.mjs`,
  `node scripts/check-deps.mjs`, each through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- No script moves the mouse or types keys; motions come from `--demo`. No `rm` on variable, glob
  or command-substitution paths; no `bash -c`, `eval` or nested one-liners; never `bash` from
  PowerShell; commit bodies via `git commit -F <file>`.
- English, no em dash, no AI attribution; Conventional Commits, subject max 50 chars (count it).
- Anything on Berke's screen: guarded runs of at most about 70 s, started only after the master
  relays that he is looking; game runs need him to alt-tab into the game.
- New `windows` features go in the owning crate's `Cargo.toml` only; the root file is add-only.
- **No lever may win by taking from the game** (master, 2026-11-14): Berke games on this PC. The
  process priority class stays normal by default (never HIGH or REALTIME as a default); Task 7
  records the game's own smoothness per lever, and a lever that costs the game is rejected even
  if it fixes our hitches.

## Review Focus

1. **A lever the OS refuses** (`SetGPUThreadPriority(7)` or a high GPU scheduling class without
   the privilege): the app still starts, logs `lever <name>: refused <hresult>` and runs with the
   default. Test: `levers::tests::refused_lever_is_logged_not_fatal` (Task 2, fake applier).
2. **A hitch line flood** (a stalled GPU gives dozens of hitches in one motion): at most
   `MAX_HITCH_DETAILS = 4` detail lines per motion, the count stays exact.
   Test: `motion::tests::hitch_details_are_capped_and_count_is_exact` (Task 1).
3. **The visible duration must not end before the last visible change**: a turnaround
   mid-collapse moves the visible end to after the turnaround; a sub-threshold drift that adds up
   to 0.5 DIP from the final shape still counts (compare with the final state, not the previous
   frame). Test: `visible::tests::turnaround_moves_the_visible_end` (Task 3).
4. **A panic inside the render loop or a window callback**: the process logs the payload and
   quits with exit 1, never a frozen island or a silent abort.
   Tests: `thread::tests::panic_in_paint_reaches_on_fatal` (Task 5); the window-proc guard is
   checked by `window::tests::guard_turns_panic_into_logged_error` (Task 5).
5. **Close while a motion runs** (Alt+F4 or the watchdog mid-morph): the render thread is joined
   inside `WM_CLOSE` before `DefWindowProc` destroys the window; no "render stopped while
   closing" line, no device rebuild at exit. Test: `thread::tests::stop_and_join_is_idempotent`
   (Task 5); the log check is in Task 7's list.

---

## What the R1 log already says (measured 2026-11-14, before this plan)

- With Deadlock behind, every hitch is one missed vblank (worst gap 8.3 ms) while draw p99 stays
  at or under 0.41 ms, but **present p99 rises from 0.28 ms to 2.1 to 4.3 ms**. Present blocks
  (`SetMaximumFrameLatency(1)` + `Present(1, 0)`) while the GPU still runs the game's work, and a
  long present pushes the next clock wait past its vblank. GPU back pressure is the lead suspect;
  CPU preemption and DWM's own misses are not ruled out. Task 1 tells them apart.
- A spring simulation of the R1 tunings reproduces the logged durations to the frame (800, 813,
  775, 421, 563 ms). **Collapse's last 400 ms is the radius spring (`RADIUS_SHRINK`, k 150)
  settling behind `min(radius, height / 2)`: invisible.** Visible change ends by about 333 ms
  (0.5 DIP); the content fade ends at about 417 ms. Expand and
  the 775 ms page turn are the `GROW_H` (k 250, zeta 0.73) tail.
- Topmost: every alt-tab logs 5 raises within about 140 ms, each "above: IME". Raises fire once
  per `EVENT_SYSTEM_FOREGROUND`; an alt-tab produces several foreground changes (switcher, then
  the game). The IME window is the hidden default IME window, found because the culprit search
  does not skip invisible windows.

---

### Task 1: Frame cleanup and hitch instrumentation

**Files:**
- Modify: `crates/plith-core/src/page.rs` (layers without a `Vec`), `crates/plith-core/src/timeline.rs`
  (`Tick.late`), `crates/plith-core/src/motion.rs` (phases, hitch details, DWM delta)
- Create: `crates/plith-core/src/dwm.rs` (`DwmSample`, `DwmDelta`), `crates/plith-render/src/dwm.rs`
  (`DwmGetCompositionTimingInfo` reader)
- Modify: `crates/plith-render/src/paint.rs`, `crates/plith-render/src/thread/run.rs`,
  `crates/plith-render/src/thread.rs` (`LoopParts.dwm`), `crates/plith-render/Cargo.toml`
  (`Win32_Graphics_Dwm`)
- Test: `crates/plith-core/src/{page,timeline,motion,dwm}/tests.rs`, `crates/plith-render/src/thread/tests.rs`

**Interfaces:**
- Produces: `PageTurn::layers(&self) -> PageLayers` where `PageLayers` is `Copy`, holds at most two
  `PageLayer`s, outgoing first, and has `iter()`; `PaintFrame.pages: PageLayers`.
- Produces: `Tick { vblank, period, periods, late: f64 }`, `late = raw - vblank` in seconds (>= 0
  except on a snap, where it is 0).
- Produces: `FramePhases { wait_ms: f64, late_ms: f64, draw_ms: f64, present_ms: f64 }`;
  `MotionStats::record_frame(&mut self, anim_t: f64, phases: FramePhases)` replaces the
  `(anim_t, draw_ms, present_ms)` form. `MotionReport.hitch_details: Vec<HitchDetail>` with
  `HitchDetail { frame: usize, gap_ms: f64, before: FramePhases, at: FramePhases }`;
  `pub const MAX_HITCH_DETAILS: usize = 4`. `MotionReport::detail_lines(&self) -> Vec<String>`.
- Produces: `DwmSample { refreshes: u64, missed: u64, dropped: u64 }`,
  `DwmSample::delta(self, later: DwmSample) -> DwmDelta` (saturating), `MotionStats::set_dwm(DwmDelta)`;
  `LoopParts.dwm: D` with `D: Fn() -> Option<DwmSample>` (tests pass `|| None`).

- [x] **Step 1: Failing tests in `plith-core`**

```rust
// page/tests.rs
#[test] fn layers_hold_outgoing_then_incoming_without_allocating() {
    let mut turn = PageTurn::new(0); turn.turn_to(1);
    let pages: Vec<usize> = turn.layers().iter().map(|l| l.page).collect();
    assert_eq!(pages, vec![0, 1]);
}
// timeline/tests.rs
#[test] fn late_is_raw_minus_locked_vblank() { /* anchor 0.0, then raw = p + 0.001: late ~= 0.001 */ }
// motion/tests.rs
#[test] fn hitch_detail_keeps_phases_before_and_at_the_gap() { /* 240 Hz: frames at k/240, one gap of 2 periods; detail.gap_ms ~= 8.33, before/at equal the recorded phases */ }
#[test] fn hitch_details_are_capped_and_count_is_exact() { /* 10 gaps: hitches == 10, details.len() == 4 */ }
#[test] fn report_line_ends_with_dwm_delta() { /* set_dwm(missed 2, dropped 1, refreshes 190): line ends "; dwm missed 2 dropped 1 of 190" ; no set_dwm: "; dwm n/a" */ }
#[test] fn detail_line_format() { /* "hitch expand #57: gap 8.3 ms; before wait 3.9 late 0.1 draw 0.2 present 3.1; at wait 0.4 late 4.2 draw 0.2 present 0.3" */ }
// dwm/tests.rs
#[test] fn delta_saturates_on_counter_reset() { /* later < earlier gives 0, never panics */ }
```

- [x] **Step 2: Run them, expect compile failures** (`cargo test -p plith-core`).
- [x] **Step 3: Implement the core side.** `PageLayers` is `[Option<PageLayer>; 2]` inside a
  newtype. Existing `motion` line text stays byte-identical up to `first wait X ms`, then
  `; dwm ...` is appended (Oyenza-comparable part first).
- [x] **Step 4: Render side.** In `run.rs` measure `wait_ms` around `source.wait()`, take
  `late_ms` from the tick, pass `FramePhases`; read `(parts.dwm)()` in `aim` when a motion
  starts and again when it finishes, and `set_dwm` the delta. `render/src/dwm.rs` wraps
  `DwmGetCompositionTimingInfo(HWND::default(), &mut info)` (`cbSize` set; on Windows 8.1+ only a
  null HWND is allowed; check the field names `cRefresh`, `cFramesMissed`, `cFramesDropped` with
  context7). `Painter` stops calling `ctx.GetFactory()` per frame (take `&ID2D1Factory1` from
  `Gpu`) and skips the clip layer when every placeholder bar lies inside the island's straight
  band (`PLACEHOLDER.top >= shape.radius` and the bar's x span inside `[ox, ox + width]`).
  `MotionStats::new` reserves 512 frames.
- [x] **Step 5: Sinks log detail lines.** `on_motion` in `crates/plith/src/app/wire.rs` writes the
  report line, then each `detail_lines()` entry.
- [x] **Step 6: Five checks green, commit** `perf(engine): hitch detail and frame cleanup`.

### Task 2: Load levers behind launch flags

**Files:**
- Create: `crates/plith-core/src/levers.rs` (+ `levers/tests.rs`), `crates/plith-render/src/levers.rs`
- Modify: `crates/plith-core/src/launch.rs` (+ tests), `crates/plith-render/src/thread.rs`,
  `crates/plith-render/src/target.rs`, `crates/plith-render/src/device.rs`,
  `crates/plith-render/Cargo.toml` (`Wdk_Graphics_Direct3D` if the scheduling class needs it),
  `crates/plith/src/app.rs`, `crates/plith/src/app/wire.rs`

**Interfaces:**
- Produces (core): `Levers { gpu_priority: i8, sched_class: SchedClass, latency: u8, sync: u8,
  mmcss: MmcssPriority }`, `Default` = R1 behaviour (`0, Normal, 1, 1, Normal`);
  `SchedClass { Normal, AboveNormal, High }`; `MmcssPriority { Normal, High, Critical }`;
  `Levers::describe(&self) -> String` for the start line; `Launch.levers: Levers`.
  Flags: `--gpu-priority <-7..7>`, `--sched-class normal|above|high`, `--latency <1..3>`,
  `--sync <0|1>`, `--mmcss normal|high|critical`; out of range is `LaunchError::BadValue`.
- Produces (core): `LeverOutcome { name: &'static str, result: Result<(), String> }`,
  `fn outcome_line(o: &LeverOutcome) -> String` = `lever gpu-priority 7: ok` or
  `lever gpu-priority 7: refused 0x80070005`.
- Produces (render): `RenderConfig.levers: Levers`; `RenderSinks.on_note: Box<dyn Fn(String) + Send>`
  (the render thread's non-fatal log lines; posts nothing to the UI thread).
- Consumes: `CompositionTarget::new` reads `latency` and `sync`; `Gpu::new` applies
  `gpu_priority` (`IDXGIDevice::SetGPUThreadPriority`) and `sched_class`
  (`D3DKMTSetProcessSchedulingPriorityClass` on the current process, only when not `Normal`);
  `join_mmcss` applies `AvSetMmThreadPriority` when not `Normal`. Verify each signature and its
  privilege note with context7 before writing it; a refusal is an outcome, never an error.

- [x] **Step 1: Failing tests:** `launch::tests::parses_every_lever_flag`,
  `launch::tests::rejects_gpu_priority_8_and_latency_0`, `levers::tests::default_is_r1_behaviour`,
  `levers::tests::describe_lists_all_five`, `levers::tests::refused_lever_is_logged_not_fatal`
  (`outcome_line` of an `Err("0x80070005")` contains `refused 0x80070005`).
- [x] **Step 2: Run, expect failures.** **Step 3: Implement core.** **Step 4: Run, expect pass.**
- [x] **Step 5: Render and app wiring.** The start line gains `levers <describe>`; each lever the
  render thread applies sends one `outcome_line` through `on_note` at start and after every
  `recover`. `Present(sync, 0)`; `SetMaximumFrameLatency(latency)`.
- [x] **Step 6: Five checks green, commit** `feat(engine): launch flags for load levers`.

### Task 3: Honest durations and tuning presets

**Files:**
- Modify: `crates/plith-core/src/island.rs` (+ tests), `crates/plith-core/src/tuning.rs` (+ tests),
  `crates/plith-core/src/launch.rs` (+ tests), `crates/plith-render/src/thread.rs`,
  `crates/plith-render/src/thread/run.rs`, `crates/plith/src/app.rs`
- Create: `crates/plith-core/tests/durations.rs` (drives `IslandMotion`, `PageTurn` and the
  content spring at 1/240 s)

**Interfaces:**
- Produces: `TuningSet { grow_w, grow_h, shrink_w, shrink_h, radius_grow, radius_shrink,
  card_grow_w, card_grow_h, page_offset, opacity: Tuning }`, `pub const R1: TuningSet`,
  `pub const BRISK: TuningSet`, `TuningSet::by_name(&str) -> Option<TuningSet>`.
  `IslandMotion::new(rest: IslandShape, set: TuningSet)`; `retarget` uses `card_grow_*` for a
  growing axis when both the current target and the new one differ from `REST`.
  `PageTurn::new(page, set)`. `Launch.tune: TuningSet` from `--tune r1|brisk`, default `R1`
  until Berke picks (Task 7). `RenderConfig.tune: TuningSet`.
- Values: `R1` = today's constants, `card_grow_*` = `GROW_W`, `GROW_H`. `BRISK` = R1 except
  `grow_h = (320, 29)` (zeta 0.81, keeps a small overshoot) and `card_grow_w = card_grow_h =
  (500, 40)` (zeta 0.89).
- Visible duration (master, 2026-11-14): the springs are not changed or snapped; the motion still
  runs to full settle so the full duration stays comparable with Oyenza. New
  `crates/plith-core/src/visible.rs`: `DrawnState { shape: IslandShape, content: f64, pages:
  PageLayers }`, `DrawnState::at_rest(scene_target: IslandShape, expanded: bool, page: usize)`,
  `fn visibly_differs(a: &DrawnState, b: &DrawnState) -> bool` (any of width, height, drawn
  radius, page offset over 0.5 DIP apart, or any opacity over 0.5/255 apart; a layer present in
  one and absent in the other differs when its opacity is over 0.5/255).
  `MotionStats::note_visible(anim_t)` marks the last frame that still differs from the rest state
  of the current target; `MotionReport.visible_ms` = that frame's `anim_t` minus the first plus
  one period (0 when no frame differed). The line reads
  `motion collapse: 194 frames, visible 333 ms, full 813 ms (238.8 fps at 240 Hz), ...`: fps over
  the full duration as before.

- [x] **Step 1: Failing tests** (simulated at 1/240 s until all of island, pages and content
  report settled; durations in ms as frames * 1000/240; tolerance one frame, 4.2 ms):

```rust
// tests/durations.rs, expected values from the plan's simulation, 2026-11-14
// full durations (all springs settled), expected values from the plan's simulation, 2026-11-14
#[test] fn r1_full_durations_match_r1_log() {}  // expand 800, collapse 813, page 0->1 775, 1->2 421, 2->0 563
#[test] fn brisk_full_durations() {}            // expand 592, collapse 813, pages 0->1, 1->2, 2->0 all 421
// visible durations: the test prints the table and asserts visible <= full for every motion and
// r1 collapse visible <= 450 (the radius tail is not counted); report the table to the master
#[test] fn visible_durations_are_shorter_where_a_tail_hides() {}
// visible/tests.rs
#[test] fn turnaround_moves_the_visible_end() {}
#[test] fn clamped_radius_tail_is_not_visible() {} // radius 16.3 under height 32: no difference
// tuning/tests.rs
#[test] fn brisk_zetas_match_the_plan() {} // grow_h 0.81, card 0.89, within 0.006
```

- [x] **Step 2: Run, expect failures.** **Step 3: Implement.** **Step 4: Run, expect pass.**
  If a simulated value differs from the plan by more than one frame, stop and report it to the
  master with the simulated table; do not adjust the test to match.
- [x] **Step 5: Start line gains `tune r1|brisk`.** Five checks green, commit
  `feat(engine): visible duration, tuning presets`.

### Task 4: Topmost explained and debounced; region-only hit testing

**Files:**
- Modify: `crates/plith-core/src/topmost.rs` (+ tests), `crates/plith-shell/src/topmost.rs`,
  `crates/plith-shell/src/window.rs`, `crates/plith-shell/src/window/state.rs`,
  `crates/plith-shell/src/hit.rs`, `crates/plith-shell/src/hit/apply.rs`,
  `crates/plith-core/src/launch.rs` (+ tests), `crates/plith/src/app.rs`, `crates/plith/src/app/wire.rs`
- Delete: `crates/plith-shell/src/hit/mask.rs` and the split input window code in `window.rs`

**Interfaces:**
- Produces: `TopmostFight::on_foreground(&mut self, now_ms: u64) -> TopmostAction`: raises at once
  only when the last raise was `DEBOUNCE_MS = 100` or more ago, otherwise restarts the retry count
  and returns `Raise { retry_in_ms }` with a flag `immediate: false` so the shell only re-arms the
  timer. `ShellEvent::Raised { foreground: String, above: Option<String>, immediate: bool }`.
- Culprit search skips windows that are not visible (`IsWindowVisible`), and `above` is `None` when
  nothing visible is above ours. Log: `topmost raise (foreground: X; above: Y|none)` or
  `topmost retry armed (foreground: X)` for a debounced one.
- `HitMode`, `--hit`, the split input window and the mask window are removed; `--hit` becomes an
  unknown flag; the start line drops `hit ...`. One shared `hwnd_of` in `plith-shell`
  (`crates/plith-shell/src/handles.rs`), used by `poster`, `topmost`, `window`.

- [ ] **Step 1: Failing tests:** `topmost::tests::second_foreground_within_100_ms_only_rearms`,
  `topmost::tests::foreground_after_100_ms_raises`, `launch::tests::hit_flag_is_unknown`.
- [ ] **Step 2: Run, expect failures.** **Step 3: Implement**, removing the split and mask code
  paths and their tests in `hit/tests.rs`. **Step 4: Run, expect pass.**
- [ ] **Step 5: Five checks green, commit** `refactor(shell): region hit only, quiet topmost`.

### Task 5: Panics, close order and the review minors

**Files:**
- Modify: `crates/plith-render/src/thread.rs` (+ `thread/tests.rs`), `crates/plith-render/src/gpu_paint.rs`,
  `crates/plith-shell/src/window.rs`, `crates/plith-shell/src/window/state.rs`,
  `crates/plith-shell/src/topmost.rs`, `crates/plith/src/app.rs`, `crates/plith/src/app/wire.rs`,
  `crates/plith/src/log.rs`, `crates/plith-core/src/launch.rs` (+ tests)

**Interfaces:**
- Produces: `render_main` runs the loop inside `std::panic::catch_unwind(AssertUnwindSafe(..))`;
  a panic calls `on_fatal(format!("render thread panicked: {payload}"))` (payload as `&str` or
  `String`, else `"non-string payload"`).
- Produces: `StopHandle::stop_and_join(&self)`; the join handle lives in
  `Arc<Mutex<Option<JoinHandle<()>>>>` shared by `RenderHandle` and `StopHandle`, so the second
  caller is a no-op. The `Closing` handler calls it, so the render thread ends before
  `DefWindowProc` destroys the window (L2, L5). The watchdog is still running at that point, so
  a `WaitForVBlank` that never returns is covered (R1 deferred minor).
- Produces: `plith_shell::guard(label: &str, f: impl FnOnce()) -> Result<(), String>` wrapping
  `catch_unwind`; `window_proc`'s `emit` and `foreground_proc` run through it and a panic posts
  `WM_CLOSE` and reports `ShellEvent::Panicked(String)` on the next safe emit, or at least logs
  through a `set_panic_sink` callback (pick the smaller of the two; the test pins the behaviour).
- Produces: `LogFile` writes through a `plith-log` writer thread fed by an `mpsc::Sender<String>`;
  `line()` never touches the file on the caller's thread; dropping `LogFile` joins the writer so
  `exit N` is the last line. Motion lines therefore leave the render thread at once.
- Minors: `Stack` field order `painter, target, gpu` (L4); `--idle-check` above 70 rejected at
  parse (L8, `launch::tests::idle_check_over_70_is_rejected`); `Error::from_thread` only where the
  failing call sets the thread error (check each site against the API doc); the stray
  `PostQuitMessage` on a failed create (`window.rs:318`) only when the window was created; thin
  `SAFETY` comments in `hit/apply.rs` and `topmost.rs` name the invariant.

- [ ] **Step 1: Failing tests:** `thread::tests::panic_in_paint_reaches_on_fatal` (a fake `Paint`
  that panics on its second frame; `on_fatal` receives a message containing `panicked`, the join
  returns), `thread::tests::stop_and_join_is_idempotent`, `window::tests::guard_turns_panic_into_logged_error`,
  `log::tests::lines_keep_order_and_flush_on_drop`, `launch::tests::idle_check_over_70_is_rejected`.
- [ ] **Step 2: Run, expect failures.** **Step 3: Implement.** **Step 4: Run, expect pass.**
- [ ] **Step 5: Five checks green, commit** `fix(engine): panic guard and join before destroy`.

### Task 6: Pre-session smoke (the island shows briefly; master's yes first)

- [ ] **Step 1:** `node ~/.claude/bin/heavy-run.mjs -- cargo build --release`.
- [ ] **Step 2:** With the master's yes (the island shows for about 10 s), run
  `target\release\plith.exe --demo basic --exit-after-demo --tune brisk --gpu-priority 7
  --sched-class high --mmcss critical` once and check the log: start line with `levers` and
  `tune`, one `lever` outcome line per lever (refused is fine), motion lines ending in
  `; dwm ...`, exit 0. Paste the lines into this task.
- [ ] **Step 3:** Message the master the run list for Task 7 and wait for his relay.

### Task 7: Console session with Berke and Deadlock (master arranges)

Asked in one batch; every run is `--demo basic --exit-after-demo` (about 60 s, 25 motions) started
on the desktop, then Berke alt-tabs into Deadlock (borderless). Nothing moves the mouse.

- [ ] Run A, baseline: no lever flags. Expect R1's picture; the hitch lines name the phase.
- [ ] Run B: `--gpu-priority 7`. Run C: `--sched-class high`. Run D: `--latency 2 --sync 0`.
  Run E: `--mmcss critical`. Run F: the winners together. For every run, record the game's own
  smoothness: Berke's eye (verbatim) and the DWM missed/dropped delta of the motion lines; a run
  whose game feels or measures worse rejects its lever, whatever our hitches say. Skip a run whose lever was refused at
  start, and skip later runs if one already reaches 0 to 1 hitches on every motion twice.
- [ ] Acceptance (G1 with a game): every motion 0 to 1 hitches, draw p99 under 1 ms. If no
  combination reaches it, record which phase the hitch lines blame and the DWM delta, and stop:
  the next lever (waitable swap chain or a D3D12 high-priority queue) goes to the master as a
  decision, not into this plan.
- [ ] Tuning by eye, no game: `--tune r1` then `--tune brisk`, `--demo basic` each; Berke's
  verdict verbatim. The pick becomes the default.
- [ ] Topmost: during run A, the log shows the foreground classes of one alt-tab and whether
  anything visible sits above; record the explanation of the raises per alt-tab.
- [ ] Close check: one run ended by Alt+F4 (Berke) mid-motion shows no `render stopped while
  closing` line.

### Task 8: Make the winners default and record them

**Files:** `crates/plith-core/src/levers.rs`, `crates/plith-core/src/tuning.rs`,
`crates/plith-core/src/launch.rs` and their tests, this plan.

- [ ] **Step 1:** Change `Levers::default()` (never a HIGH or REALTIME default) and the default `TuningSet` to the Task 7 picks;
  update `levers::tests::default_is_r1_behaviour` to `default_is_the_r1b_pick` with the picked
  values; `tests/durations.rs` keeps both sets.
- [ ] **Step 2:** Draft the ADR amendments below for the master (0003 clock and priorities, 0005
  latency and sync if changed): decision, the Task 7 numbers copied from the log.
- [ ] **Step 3:** Five checks green, commit `perf(engine): default to measured levers`.
- [ ] **Step 4:** `project-docs` handoff on this branch, plan banner, message the master.

## ADR amendment drafts (filled in Task 8; the master files them)

| ADR | Change | Numbers |
|---|---|---|
| 0003 | (Task 7) | (Task 7) |
| 0005 | (Task 7) | (Task 7) |

## Not in this plan

A waitable swap chain or a D3D12 queue (only if Task 7 says priorities are not enough; a master
decision), PresentMon captures (needs an install and elevation; ask first), a second borderless
game (deferred by the master), relayout on display change (R4), L3 `WaitForVBlank` timeout, L6
end-session handling, L7, L9, L11 to L14 (R4 wiring).
