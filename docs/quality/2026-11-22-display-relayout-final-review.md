# Final review: feature/display-relayout (79427c2..25ecc6a)

Reviewer read the full review package diff (2547 lines) in three passes, plus the surrounding
unchanged code it depends on: `thread/run.rs`, `run/engine.rs`, `run/ambient.rs`,
`run/prewarm.rs`, `thread/clocks.rs`, `clock.rs::open_clock`, `plith-core` `timeline.rs`,
`ambient.rs`, `clock_choice.rs`, `layout.rs`, shell `poster.rs` (`Coalesce`), `window.rs::place`,
`topmost.rs::raise`, `monitor.rs::primary_monitor`, `sheet_window.rs::placement`, app
`app.rs::run_shell` and `state/sheet.rs`. No builds or tests were run (the five checks were
reported green by the caller).

## Strengths

- The split is right: the shell owns placement and the hit region, the app only logs and
  forwards, the render loop owns the stack and the clock. `plith-core::display::Display` keeps
  the "did the clock change" decision pure and unit tested.
- Coalescing reuses the existing `Coalesce` (`poster.rs:23-50`), clears it at the start of
  `relayout_to` (`window/display.rs:63`), so a change arriving during handling re-posts, and a
  failed post re-arms. `relayout_to` is idempotent: the `WM_DPICHANGED` that `SetWindowPos`
  itself can send while crossing monitors produces a second pass that compares equal and does
  nothing.
- Lock discipline is clean. The owner side takes `last`, then `clock_target`, releases, then the
  slot, then the mailbox; the loop side never nests locks (`take_relayout`, `clock_target` in
  `open_clock`). No deadlock is possible. `RelayoutHandle` is `Clone + Send + Sync` by
  construction.
- `request_relayout` merging (a later layout-only request keeps an earlier clock change and its
  hz, `run/relayout.rs:50-60`) is correct and tested.
- `place()` keeps `SWP_NOACTIVATE` and `HWND_TOPMOST` with no `SWP_SHOWWINDOW`: no activation, no
  show while hidden, and the z-order change is the same one the topmost fight
  (`topmost.rs:107-121`) already makes on every foreground change.
- The hit-region cache is cleared before `apply_latest` (`window/display.rs:71-72`), with a test
  that counts the rebuild (Focus 5).
- The three Task 3 rulings are sound. Applying after `take()` in the parked loop (so a
  suppressed snapshot is never painted) is the correct order. Going straight to WARP rather
  than through `GpuHealth` is reasonable, and its stated cost is smaller than the ruling says:
  the next display change runs `relayout` -> `recover(false)` again, so the hardware device
  returns at the next relayout (for example RDP back to console), not only at the next session.

## Review Focus, checked one by one

1. **Burst -> one relayout.** Met for messages that arrive before the posted
   `WM_APP_RELAYOUT` is dequeued (sent messages are dispatched before posted ones, so a single
   burst collapses). Messages spaced across pumps (RDP connect can deliver `WM_DPICHANGED`
   tens of ms after the `WM_DISPLAYCHANGE`s) give one relayout per real intermediate state, each
   a full stack rebuild. See Minor 3. Unchanged states never reach the render thread.
2. **Relayout during a motion.** No fatal, the frame of that tick is still painted (relayout is
   applied before the paint at `run.rs:268`). The rebuild time lands in that motion's stats as
   a hitch, and pages are rastered lazily inside the measured frames because `warm` is cleared
   (acceptable for a rare event). A relayout can be lost on the motion's last frame: Important 1.
3. **Relayout while suppressed.** Correct: `Snapped` -> `apply_relayout(false)`, stack rebuilt,
   no paint; unsuppress differs in `suppression`, so `Aimed` + settled paints once on the new
   stack.
4. **Rebuild fails -> WARP.** Correct (`run/relayout.rs:796-800`), with a fatal only when WARP
   also fails, both tested. Deviation from the plan text (which still says "goes through
   `GpuHealth`"): ruled, accepted; the plan's Review Focus 4 and Task 3 Step 3 should be updated
   to match.
5. **Hit region after a scale change.** Correct and tested.

## Issues

### Critical

None.

### Important

1. **A relayout can be stranded in the slot while the loop parks.**
   Files: `crates/plith-render/src/thread/run.rs:268-271` (animate), `run/ambient.rs:58-59`
   with `ambient_news` at `ambient.rs:88-106`, park at `run.rs:201`.
   The owner stores the request, then nudges the mailbox. `animate` and `ambient` call
   `apply_relayout` *before* reading `mailbox.latest()`. If the request and nudge land between
   those two calls (a microsecond window), `take()` consumes the nudged generation, this frame
   paints at the old layout, and:
   - in `animate`, if this is the motion's last frame (`finish_motion`, return) or the snapshot
     is `Snapped`, the loop goes back to `wait_newer(self.seen)` with `seen` already equal to
     the nudge's generation;
   - in `ambient_news`, if `wants_ambient()` is now false (title scroll ended, page without
     live nodes), it paints at the old layout and returns `Parked`, same result.
   The relayout then sits in `LoopShared` until the next unrelated mailbox publish. Meanwhile
   the shell has already moved and resized the window and rebuilt the region at the new scale,
   so after a 100% -> 150% change the island draws at the old pixel size, off-centre and
   clipped by the new region. It heals at the next publish (a content change, the minute tick),
   so it is narrow and self-healing, but it is exactly the "lost relayout" the brief asked about,
   and it would show up as a wrong-looking notch with no log line explaining it.
   Fix (any one): check the slot before parking (for example `if self.shared.has_relayout()`
   at the top of the `run` loop, before `wait_newer`, apply with the parked loop's repaint rule
   and `continue`); or move `apply_relayout` after the `mailbox.latest()` read in `animate`
   and into `ambient_news` before its final `paint_frame`. Because the request is stored
   before the nudge and both go through mutexes, any path that has observed the nudged
   generation is guaranteed to find the slot filled. Add a harness test that publishes the
   request and the nudge on the settling frame (the fake clock's script hook can do this).

### Minor

1. **hz-derived state is not refreshed inside a running motion or ambient run.**
   `run/relayout.rs:804-812` updates `hz`, `period`, `timeline` and the clock order, but the
   ambient run's `divisor` and `AmbientStats` (`ambient.rs:39-44`) and the motion's
   `MotionStats` (created with the old `hz` in `engine.rs:231`) keep the old rate. Scenario: a
   live page is open at 240 Hz (divisor 4) and the display becomes 60 Hz: the page ticks at
   15 fps instead of 60 until the run ends; going 60 -> 240 paints 240 fps (divisor 1, four
   times the GPU work). The `ambient`/`motion` lines for that run judge hitches against the
   wrong period. Fix: when the clock was reopened, end the ambient run (`AmbientEnd::Parked`,
   the loop re-enters ambient with the new hz) and drop or re-seed the motion stats.
   `prev_raw` is also not reset on a clock reopen (the demote path resets it); harmless today
   because a long interval is not a "fast" one, but it is worth making symmetric.
2. **A display change before `attach_relayout` is dropped.** `crates/plith/src/app/state.rs:175-179`
   logs and drops the event when `relayout` is `None`; `attach_relayout` happens only after
   `spawn` returns (`app.rs:327`). The posted `WM_APP_RELAYOUT` is normally dispatched by the
   message loop after that, so this needs something to pump messages during startup (a COM STA
   modal wait in `attach_feed`/`start_sides`), and a display change in the first second.
   The effect would be permanent until the next change: window moved, render at the start
   layout. Fix: on attach, forward the shell's current display (and let `RelayoutHandle` skip a
   display equal to `last`), or keep the latest unforwarded display in `App`.
3. **Spaced bursts rebuild more than once.** `window/display.rs:44-55`. Coalescing covers only
   messages queued before the post is handled. Each intermediate state (resolution first, DPI a
   moment later) costs a full stack rebuild with a blank composition target in between, and the
   intermediate read may be a transitional monitor state. A short `SetTimer` debounce
   (100-250 ms) on the visual window, reusing the existing timer plumbing, would make "one
   relayout per RDP connect" true in practice. Task 5 should count `render relayout:` lines on
   an RDP connect to decide whether this is needed.
4. **The settings window keeps the start `HMONITOR`.** `crates/plith/src/app/state/sheet.rs:97`
   stores `monitor.handle` once. After an RDP connect that handle is stale; `placement` falls
   back to the primary monitor when the handle is invalid, so it works, unless the value is
   reused for another monitor. Updating `sheet.monitor` in `on_display_changed` is one line.
   (The spec lists the sheet as a non-goal for its own DPI handling; this is only about where
   it opens.)
5. **Two render tests assert something that holds without the feature.**
   `crates/plith-render/src/thread/tests/relayout.rs:57` and `:74` compare the frame origin with
   `wide().island_origin(..)`, but the origin is DIP and equal across layouts by design (the
   implementer's own Task 3 ruling). `relayout_mid_motion_keeps_animating` also never checks
   that a relayout happened (no `wait_relayouts(&run, 1)` or `relayouts.len() == 1`), so it
   would pass if the relayout were ignored or applied after the motion. Add the relayout-count
   assertion and assert the relayout was applied before the report (for example via the
   `render relayout:` note order).
6. **Plan and spec drift.** Plan Review Focus 4 and Task 3 Step 1/3 still describe the
   `GpuHealth` path; spec Design 2-3 describes the app re-reading the monitor and logging
   `relayout: unchanged`, which the shell-owned design no longer does (an unchanged pass logs
   nothing). Update the text so the next reader is not misled; this is the "doc vs code: code
   wins, fix the doc" rule.

## Questions you asked about, with no issue found

- **Thread safety of `LoopShared` / `RelayoutHandle`:** fine (see Strengths).
- **Clock reopen with a new monitor:** `open_clock` reads `shared.clock_target()` each time,
  which is set before the request is stored. A clock opened in the short gap before the
  relayout is applied (a demotion) already targets the new monitor with the old timeline; it is
  dropped by `replace_order` at the next tick. Harmless. Only DXGI uses the monitor; DComp needs
  none; Timer uses hz.
- **Z-order / activation:** unchanged policy (see Strengths).
- **OSD:** separate process, untouched. **Sheet:** its own window proc, `is_visual` guards the
  new arms; only Minor 4 applies.
- **Mouse mapping after the move:** `message_point`/`raw_point` read `inner.layout()` live, and
  the island point is in DIP, so a press in flight keeps a consistent coordinate space. Between
  the shell's move and the render's rebuild (one frame or one wake) the drawn island and the
  hit mapping can disagree briefly; acceptable.
- **`prewarm` / `warmed_motion` / `content_at_rest` consuming generations:** they do not read
  the mailbox; the only consumers are the parked `take` (covered), `animate` and
  `ambient_news` (Important 1).

## Declined to judge

- Whether `DxgiClock::wait` on an output removed by an RDP connect returns promptly or blocks;
  a blocked wait would delay applying the relayout. Needs the on-screen run (Task 5).
- Whether `Stack::new` on hardware succeeds during the RDP adapter switch or routinely falls
  back to WARP. Needs the on-screen run.
- Visual flash during the rebuild (the composition target is released before the new stack
  presents). Accepted by the spec's choice of the device-loss path; Task 5 should look at it.
- Work-area changes (a taskbar moved to the top) arrive as `WM_SETTINGCHANGE`, not
  `WM_DISPLAYCHANGE`, so `work_top_px` is not followed. Outside the spec's trigger list.
- Following a non-primary monitor: spec non-goal.

## Assessment

**Ready to merge: With fixes.** The design and wiring are sound and all five focus points hold
in the common path; fix Important 1 (a stranded relayout that leaves a visibly wrong notch)
before merge, ideally with Minor 1 and the test gap in Minor 5. Task 5 (console and RDP on
screen) is still open, and per the project rule nothing here is proven on screen yet.

## Outcome (implementer, 2026-11-22)
- Important fixed in `51b0b30`: the loop applies a pending relayout before it parks; test
  `a_pending_relayout_is_applied_before_the_loop_parks` failed first, then passed. 1262 tests.
- Minors 1 to 5 deferred (listed in STATUS); minor 6 (stale spec and plan text) fixed in
  `51b0b30`.
