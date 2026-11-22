# Relayout on a Display Change Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After an RDP connect, a resolution or a DPI change the notch moves to the new top
centre of the primary monitor, draws at the new scale and paces frames on the new clock,
without a restart.

**Architecture:** The shell owns placement: it coalesces `WM_DISPLAYCHANGE`/`WM_DPICHANGED`
into one posted message, re-reads the primary monitor, moves the window and re-applies the hit
region, then emits `ShellEvent::DisplayChanged(Display)` only when something changed. The app
logs it and forwards it to the render thread through a `Relayouter` (a trait, like `Stopper`).
The render loop takes the pending relayout before its next frame, rebuilds the paint stack at
the new layout (the device-loss path) and reopens the clock when monitor, hz or remote changed.

**Tech Stack:** Rust 2024, `windows` crate (Win32 `SetWindowPos`, `PostMessageW`), D3D11/DComp
stack in `plith-render`.

**Spec:** `docs/specs/2026-11-22-display-relayout-design.md`

## Global Constraints
- Code, comments, logs, commits in English; no em dash anywhere (`structure-check.mjs`).
- Limits: 500 source, 80 per function, never `mod.rs`; `window.rs` (466) and `engine.rs` (337)
  get new code in new files, not growth.
- Pure logic in `plith-core` with unit tests; no Windows code there.
- The move never activates the notch and never changes its z-order beyond `HWND_TOPMOST`.
- Builds and the full suite through `node ~/.claude/bin/heavy-run.mjs -- <cmd>`.
- Ask before every commit; branch `feature/display-relayout` in a worktree.

## Review Focus
1. A burst of messages (RDP connect sends several `WM_DISPLAYCHANGE` and a `WM_DPICHANGED`):
   one relayout, not five rebuilds. Test in Task 2 (coalesce flag) and Task 1 (unchanged =
   no event).
2. A relayout during a motion: the motion keeps going on the new layout, no fatal, no lost
   frame. Test in Task 3 (`relayout_mid_motion_keeps_animating`).
3. A relayout while suppressed (display off, locked): stored, no paint until unsuppressed.
   Test in Task 3 (`relayout_while_suppressed_paints_nothing`).
4. Stack rebuild fails after a relayout: goes through `GpuHealth` like a device loss, WARP
   fallback, not a silent black window. Test in Task 3 (`relayout_failure_reports_fatal`).
5. Hit region after a scale change: the region is recomputed even though the shape is the same
   (`applied` cache must be cleared). Test in Task 2.

---

### Task 1: `Display` value in plith-core

**Files:**
- Create: `crates/plith-core/src/display.rs`, `crates/plith-core/src/display/tests.rs`
- Modify: `crates/plith-core/src/lib.rs` (add `pub mod display;`)

**Interfaces:**
- Produces:
  `#[derive(Debug, Clone, Copy, PartialEq)] pub struct Display { pub layout: SurfaceLayout,
  pub monitor: isize, pub hz: f64, pub remote: bool }`
  `impl Display { pub fn clock_changed(&self, next: &Display) -> bool }` (monitor, hz or
  remote differ) and `pub fn log_line(&self) -> String` returning
  `relayout: rect=X,Y WxH scale=S hz=H remote=R` (scale and hz with two decimals).

- [x] **Step 1: Write failing tests** `same_monitor_new_scale_keeps_clock` (only layout
  differs: `clock_changed` false), `remote_flip_changes_clock`, `hz_change_changes_clock`,
  `log_line_format` (rect 1280,0 960x400 scale 1.50 hz 240.00 remote false gives exactly
  `relayout: rect=1280,0 960x400 scale=1.50 hz=240.00 remote=false`).
- [x] **Step 2:** `cargo test -p plith-core display` fails (module missing).
- [x] **Step 3:** Implement.
- [x] **Step 4:** Tests pass.
- [x] **Step 5:** Commit `feat(core): display value for relayouts`.

### Task 2: Shell moves the window and emits the change

**Files:**
- Create: `crates/plith-shell/src/window/display.rs`
- Modify: `crates/plith-shell/src/window/state.rs` (`layout: Cell<SurfaceLayout>`,
  `display: Cell<Display>`, `relayout_pending: Cell<bool>`), `crates/plith-shell/src/window.rs`
  (the `WM_DPICHANGED | WM_DISPLAYCHANGE` arm calls `display::on_message`; new arm
  `WM_APP_RELAYOUT if is_visual => display::on_relayout(inner)`; `ShellEvent::DisplayChanged`
  becomes `DisplayChanged(Display)`; `Shell::create` takes `Display` instead of
  `SurfaceLayout`), `hit/apply.rs` and `window/mouse.rs` (read `inner.layout.get()`),
  `crates/plith-shell/src/window/tests.rs`.

**Interfaces:**
- Consumes: `Display` (Task 1), `primary_monitor()`, `is_remote_session()`,
  `SurfaceLayout::new`, existing `place(hwnd, rect)` and `apply_latest(inner)`.
- Produces: `ShellEvent::DisplayChanged(Display)`; `Shell::create(display: Display, ...)`;
  `pub(crate) fn next_display(current: Display, monitor: MonitorInfo, remote: bool) ->
  Option<Display>` (pure, `None` when equal).

- [x] **Step 1: Failing tests** in `window/tests.rs`: `next_display_equal_is_none`,
  `next_display_new_rect_recentres` (monitor 3840 wide at scale 1.5 gives the same surface as
  `SurfaceLayout::new` for it), `relayout_message_burst_posts_once` (two `on_message` calls
  with the pending flag set post one `WM_APP_RELAYOUT`, use the existing poster test seam),
  `relayout_clears_applied_region` (after `on_relayout` with a new scale `inner.applied` is
  `None` before `apply_latest`, so the region is rebuilt).
- [x] **Step 2:** `cargo test -p plith-shell window` fails.
- [x] **Step 3:** Implement. `on_message` sets `relayout_pending` and posts once (reset the
  flag if the post fails); returns `LRESULT(0)`. `on_relayout` clears the flag, computes
  `next_display`, on `Some`: store it and its layout, `place(hwnd, layout.surface)`,
  `applied.set(None)`, `apply_latest`, emit the event.
- [x] **Step 4:** Tests pass; `cargo clippy -p plith-shell -- -D warnings` clean.
- [x] **Step 5:** Commit `feat(shell): move the notch on a display change`.

### Task 3: Render loop takes a relayout

**Files:**
- Create: `crates/plith-render/src/thread/run/relayout.rs`
- Modify: `crates/plith-render/src/thread.rs` (`Paint::relayout`, `RenderHandle::relayouter`,
  `RelayoutHandle`, `open_clock` reads the shared clock target), `thread/run.rs`
  (`LoopShared` gains the pending slot and clock target; the parked loop and `animate` call
  `apply_relayout` after each wake), `gpu_paint.rs` (`relayout` stores the layout then runs
  the recover path), `plith-core/src/scene.rs` (`SceneMailbox::nudge`), thread tests.

**Interfaces:**
- Consumes: `Display` (Task 1), `clock_order(request, remote)`.
- Produces:
  `Paint::relayout(&mut self, layout: SurfaceLayout) -> windows::core::Result<()>` (default
  `Ok(())` for fakes); `pub struct RelayoutHandle` (`Clone + Send`) with
  `pub fn relayout(&self, display: Display)`; `RenderHandle::relayouter(&self) ->
  RelayoutHandle`; `SceneMailbox::nudge(&self) -> u64` (bumps the generation, always wakes);
  `LoopShared::request_relayout(&self, display: Display, order: Vec<ClockKind>)`.

- [ ] **Step 1: Failing tests** in `thread/tests` with the fake paint and fake clock:
  `relayout_at_rest_repaints_once_with_new_origin` (frame origin equals
  `new_layout.island_origin(shape)`), `relayout_mid_motion_keeps_animating` (motion report
  still written, no fatal), `relayout_while_suppressed_paints_nothing`,
  `relayout_with_new_hz_reopens_clock` (fake `open_clock` called again, period 1/new hz),
  `relayout_failure_reports_fatal` (fake `relayout` errors twice: `GpuHealth` path, then
  `on_fatal`), `nudge_wakes_waiter` in `scene` tests.
- [ ] **Step 2:** `cargo test -p plith-render thread` and `-p plith-core scene` fail.
- [ ] **Step 3:** Implement `apply_relayout(&mut self) -> Result<(), String>` in
  `relayout.rs`: take the pending value; store `layout`; `paint.relayout` (on error feed
  `GpuHealth` as a failed paint and follow its action); clear `warm` and `presented`; when
  the clock changed set `hz`, `period`, `timeline = VsyncTimeline::new(hz)`, replace
  `clocks.order` and drop the current source; log through `on_note`
  `render relayout: WxH scale=S clock=...`. Fix the stale R1 comment in `animate` about
  `needs_rate_reread`.
- [ ] **Step 4:** Tests pass; clippy clean.
- [ ] **Step 5:** Commit `feat(render): rebuild the stack on a relayout`.

### Task 4: App wiring

**Files:**
- Modify: `crates/plith/src/app.rs` (build the start `Display`, pass it to `Shell::create`
  and `RenderConfig`, attach `render.relayouter()`), `crates/plith/src/app/state.rs`
  (`Relayouter` trait beside `Stopper`, `attach_relayout`, the `DisplayChanged` arm),
  `crates/plith/src/app/tests.rs`.

**Interfaces:**
- Consumes: Tasks 1 to 3.
- Produces: `pub trait Relayouter { fn relayout(&self, display: Display); }` implemented for
  `RelayoutHandle`; `App::attach_relayout(&mut self, r: impl Relayouter + 'static)`.

- [ ] **Step 1: Failing test** `display_change_logs_and_forwards` (fake relayouter records the
  display, the log has the `Display::log_line` text).
- [ ] **Step 2:** Fails. **Step 3:** Implement; remove `display changed: restart needed`.
- [ ] **Step 4:** Five checks through heavy-run: fmt, clippy `--workspace -D warnings`, full
  tests, `node scripts/structure-check.mjs` 0, release build.
- [ ] **Step 5:** Commit `feat(app): relayout the notch on a display change`.

### Task 5: Measure on screen (Berke present)
- [ ] Console (`query session` first): change resolution, then scale 100/150 in Settings; the
  notch recentres, one `relayout:` and one `render relayout:` line per change, the next motion
  line keeps 240 fps and 0 to 1 hitches; a click on the notch still opens it (hit region).
- [ ] RDP from the Mac: connect, the notch sits top centre, `remote=true` in the line, motion
  runs on the timer clock; disconnect back to console, `remote=false`.
- [ ] Record results in this plan's progress log and STATUS; remove the Open risk.

## Progress log
(empty)
