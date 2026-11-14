# Engine R1b review (independent read)

Date: 2026-11-14. Branch `feature/engine-r1b`, range `6d25b23..cfaf5bf` (55 files). Read-only
review from `git diff`/`git show` and the worktree at `cfaf5bf`; nothing was built or run. Line
numbers refer to `cfaf5bf`.

## Strengths

- The shutdown story is coherent and documented where it lives. `StopHandle::stop_and_join`
  shares one `JoinHandle` behind a poison-tolerant mutex (`plith-render/src/thread.rs`), so the
  `Closing` join, the `on_panic` join and `RenderHandle::drop` are idempotent, and
  `stop_and_join_is_idempotent` actually proves the first caller waited (it checks `done`).
- The deadlock invariant holds in the code, not only in the comment: every render-to-UI path is
  `PostMessageW` (`ShapePoster::post`, `QuitHandle::quit`), the sinks only send on unbounded
  channels (`LogFile::line`) and the render thread never takes the join lock. The mailbox and the
  shape cell recover from poisoning (`scene.rs:65`, `:126`), so a render panic cannot turn into a
  second panic on the UI thread.
- `with_backstop` releases its timer by dropping the sender, so a returning or unwinding `work`
  frees the timer at once; the three tests cover return, overrun and unwind.
- `LogFile` moves disk I/O off the render thread, `flush()` is bounded (1 s) and is called on
  both `process::exit` paths (watchdog, backstop). On the normal path the last `Arc<LogFile>`
  is dropped at the end of `app::run`, which joins the writer before `main` calls
  `process::exit`: every sink closure that holds a clone has been dropped by then (render thread
  joined, `Shell` dropped, scoped helpers joined), so `exit N` really is the last line.
- Panics are bounded at every `extern "system"` boundary (`window_proc`, `foreground_proc`) and
  at the render loop; `Inner::fail` reports once and mutes `on_event`; `WM_DESTROY` panics still
  post `WM_QUIT`. Release profile keeps `panic = unwind`, so `catch_unwind` is effective.
- Levers: defaults never raise anything (`Levers::default`, `plith-core/src/levers.rs:67`), the
  OS calls are skipped at their defaults (`apply_gpu_priority` at 0, `apply_sched_class` at
  `Normal`, `apply_mmcss` at `Normal`), refusals become `lever ... refused 0x...` lines, and GPU
  priority is re-applied on `recover` because it is part of `Stack::new`.
- Hot path: `MotionStats` reserves 512 frames and caps hitch details at 4; formatting happens
  once per motion, not per frame. `needs_clip` drops the per-frame geometry when the bars sit in
  the straight band, and the factory is cached instead of `GetFactory` per frame.
- Structure rules hold: largest touched source file is `thread/run.rs` at 434 lines, largest
  test file 428, no `mod.rs`, no em dash in the diff, new pure logic (`levers`, `tuning`,
  `visible`, `panic_text`, `topmost`) is in `plith-core` with tests and no Windows imports.
- The handoff is honest about what was not measured (on-screen delivery, game cost, panic and
  backstop paths only in unit tests).

## Issues

### Critical

None.

### Important

**I1. The hit region can lead the displayed frame by one frame under latency 2, clipping the
island while it shrinks.**
`plith-render/src/thread/run.rs:407-417` publishes the shape right after `Present` returns;
`plith-shell/src/hit/apply.rs:38-41` applies it with `SetWindowRgn` on the UI thread within
about a millisecond. ADR 0004 records that this region clips the drawn DirectComposition
content (the 1 px outset made the corners jagged). With `SetMaximumFrameLatency(1)` `Present`
blocked until the previous frame had left the GPU, so frame N and region N usually reached the
same DWM composition. With latency 2 (`levers.rs:67-77`, ADR 0008) `Present` returns while frame
N may still be queued behind the game's GPU work, and ADR 0008 itself states one more frame
(4.2 ms at 240 Hz) may queue. The region change does not queue: DWM uses it at the next
composition.
Failure scenario: game running behind the desktop, user collapses the island. During the
collapse the region for frame N is applied while frame N-1 (larger) is still what DWM shows,
so the larger island is cut to the smaller rounded rectangle. The 3 px outset is far less than
one frame of collapse travel at peak spring speed (several DIP per 4.2 ms period, times the
scale), so the cut edge is visible as a flat or jagged rim for that frame, exactly the artefact
Berke rejected at 1 px. Growth is harmless (the region is larger than the drawn shape). The
console session judged stutter ("takılma yoktu"), not edges, and PresentMon was not run, so
nothing on the branch shows this does or does not happen.
Fix: make the region the union of the last two posted shapes (keep the previous `IslandShape`
in `Inner`, `CombineRgn(RGN_OR)` or simply the larger width, height and radius since the
island is centred and top-anchored). Clicks stay correct because `on_click` already tests the
latest shape. Alternatively, confirm on screen with Berke watching collapses with the game
behind, and record the result in ADR 0004 or 0008 either way.

### Minor

**M1. The post-loop render join is not under the backstop.**
`plith/src/app.rs:106-107` calls `render.shutdown()` after the message loop and after
`drive` has already joined the watchdog. Normally a no-op (the `Closing` handler joined), but on
any path where the loop ended without `Closing` running the join (loop error `-1`, a panic that
fires before `stopper.set` at `app.rs:90`, a future `WM_ENDSESSION` or `DestroyWindow` path) a
hung clock wait (`--clock dxgi`, `WaitForVBlank` has no timeout, deferred L3) keeps the process
alive forever with no window. Fix: route the final join through `close_render` (it is a no-op
when the slot is empty) or `with_backstop`.

**M2. A `Closing` event can be queued behind the destroy.**
`plith-shell/src/window/state.rs:85-99` queues an event that arrives while another is draining,
and `window.rs:367` returns `None` for `WM_CLOSE`, so `DefWindowProc` destroys the window before
the queued `Closing` (and its render join) runs. Today no handler pumps messages, so this needs
a nested `WM_CLOSE` that cannot currently happen; it becomes live as soon as a handler pumps
(a modal, a COM call with a message filter, `SendMessage` to another thread's window). Fix: for
`WM_CLOSE`, when `draining` is set, return `Some(LRESULT(0))` and post `WM_CLOSE` again, or call
`DestroyWindow` yourself after the drain.

**M3. Leading-edge topmost debounce delays the raise for the window that matters.**
`plith-core/src/topmost.rs:36-48`: within 100 ms of a raise, a foreground change only re-arms
the 500 ms retry. One alt-tab is three foreground changes and the target is the last, so if the
target is itself topmost (an "always on top" tool, an overlay) the island stays covered for up
to 500 ms. Known gap in the handoff. Fix: for a debounced change arm the timer for
`DEBOUNCE_MS - elapsed` (trailing edge) instead of `RETRY_MS`; the tests in
`topmost/tests.rs` change accordingly.

**M4. `target.rs` still describes latency 1 behaviour.**
`plith-render/src/target.rs:129` says `present` waits "for one vertical blank"; with latency 2
it returns without waiting unless the queue is full. Also `BufferCount: 2` (`target.rs:85`)
with a flip-sequential chain leaves only one buffer that can be queued beyond the one on screen,
so the second queued frame is held as a GPU-side wait on the back buffer rather than a real
second flip. The measurement says this helps; the doc should say where the wait went (GPU
queue, invisible to `draw_ms` and `present_ms`) so later readers do not chase a phantom.

**M5. The motion line cannot see the failure latency 2 can introduce.**
Hitches are measured on the clock-locked animation time (`motion.rs:217-233`). With up to two
frames in flight, a varying queue depth judders on screen while every gap in the log stays one
period. "90/90 at 0 hitches" is a statement about submission cadence. The handoff says this;
the STATUS line written at merge should keep that qualifier, and R4's PresentMon capture is the
real acceptance check for ADR 0008.

**M6. A refused non-default `--latency` leaves the device at the DXGI default of 3.**
`plith-render/src/levers.rs:48-58`: on refusal of a non-default value nothing falls back, and
the device default is 3, not the documented 2. Only reachable through a flag; log line says
"refused" but not what is in effect. Fix: on refusal, retry with `Levers::default().latency`.

**M7. Forced exits skip every `Drop`.**
The backstop and watchdog `process::exit` paths skip `ABM_REMOVE`, the WinEvent unhook and,
once R4 wires `plith_system::FlyoutSuppressor`, its restore. Today the OS cleans up the first
two when the process dies; the flyout would stay hidden until Explorer restarts. Not a defect
on this branch, but R4 must give the suppressor a restore that runs on these paths (or restore
it from the backstop closure before `exit`).

**M8. Tests whose names claim more than they check.**
- `levers::tests::refused_lever_is_logged_not_fatal` (`plith-core/src/levers/tests.rs:33`) only
  tests the line format; non-fatality lives in `apply_latency` and the callers, which have no
  test (the handoff lists the latency rule as untested).
- `window::tests::a_panic_is_reported_once_and_mutes_the_handler` runs `Inner::fail` with
  `visual == 0`, which calls `PostQuitMessage(1)` on the test thread: harmless today, but it
  leaves a `WM_QUIT` in the queue of a test thread that a later test on the same thread could
  read.

**M9. Small items.**
- `plith/src/main.rs:10-11` usage text lacks `--gpu-priority`, `--sched-class`, `--latency`,
  `--sync`, `--mmcss` and `--tune`.
- Every foreground change writes a `topmost` line (`wire.rs:126-130`); in normal mode that is
  three lines per alt-tab for the whole session with no rotation. Fine for R1b measurement,
  should be gated or rate-limited before the app runs all day.
- `RESERVED_FRAMES = 512` (`motion.rs:12`) covers 2.1 s at 240 Hz but only 1.4 s at 360 Hz; a
  long retargeted motion on a faster panel reallocates mid-motion. Size it from `hz`.
- If `on_panic` itself panics (it runs outside `guard`, inside `Inner::fail`), the unwind
  reaches an `extern "system"` function and the process aborts without a log line. Low risk;
  wrapping the `on_panic` call in `catch_unwind` costs one line.

## Declined to judge

- Whether `IDCompositionDevice::CreateTargetForHwnd` (called on `recover`, `gpu_paint.rs:96`)
  sends a message to the window's thread. If it does, a GPU recovery that coincides with the
  `Closing` join would block until the 3 s backstop and exit with code 1. The backstop bounds
  it either way; I did not verify the API behaviour.
- Whether 2 buffers with `SetMaximumFrameLatency(2)` behave the same across vendors (only one
  RTX 5080 was measured). Needs hardware, not reading.
- The spring numbers of the `brisk` preset and the `visible_ms` thresholds (0.5 DIP, half an
  8-bit step) as perceptual choices; they are consistent with their tests and the plan.
- Real game cost of the levers (plith parks in front of a game; R4).

## Verdict

**Ready after fixes.** No Critical items. I1 should be fixed (union region, a small change) or
explicitly confirmed on screen before ADR 0008 is treated as settled. The Minor items can go to
the deferred ledger; M1 and M4 are cheap enough to do in the same pass.
