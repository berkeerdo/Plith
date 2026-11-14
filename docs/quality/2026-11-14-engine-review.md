# Engine slice pre-merge review (concurrency and graphics axes)

Branch `origin/feature/engine-slice` at c010205, against `main`. This is a read-only review. Nothing was built or run.
Scope: the five axes in the brief only. All paths are on the branch.

**Result: 0 BLOCKING, 14 LATER, 1 merge note.**

## Merge note (not a code defect)

- The merge base is 5887b06, not main's head cc1dcbc, so the branch predates the R2 services
  merge. Both sides add a line directly after `windows = "0.62.2"` in the root `Cargo.toml`.
  The branch adds `windows-numerics`; main adds `windows-future` and `windows-core`. That is
  a textual conflict, and `Cargo.lock` will conflict too. The green fmt, clippy, tests and
  structure results were measured on the branch tree, not on the merged tree. Rerun them
  after resolving.

## Axis 1: thread ownership and the handoffs (clean)

What I verified:
- Every D3D, DXGI, D2D and DComp object is created inside `render_main` (`plith-render/src/thread.rs:195-226`)
  and owned by `GpuPaint`/`Engine`. They are dropped when `run_loop` returns, which happens on the render thread.
  Clocks (`IDXGIOutput`, waitable timer) are opened lazily inside `Clocks::source` (`thread/run.rs:112`) and are
  dropped on the same thread. Only an `isize` HWND/HMONITOR crosses (`RenderConfig`, `thread.rs:62-76`).
  The UI thread never names a graphics type.
- `SceneMailbox` (`plith-core/src/scene.rs:57-135`): it checks the generation and the stop flag
  under the mutex, waits on a condvar in a loop, notifies after unlocking, and `stop()` sets
  the flag under the lock. There is no lost wakeup. A publish between `latest()` and the
  settle check in `animate` is caught by `wait_newer(seen)`. The `update` closures are pure,
  and no sink runs while the lock is held, so no lock-order cycle exists.
- Shape handoff (`plith-shell/src/poster.rs:28-40` and `window.rs:342-346`). The cell is
  published before the `pending` swap, and the UI thread clears `pending` before it reads the
  cell. If the swap read `true`, the UI's later read sees the new frame through the mutex
  order. A failed post resets `pending`. The handoff is correct.
- The render thread never `SendMessage`s to the UI thread. It only uses `PostMessageW` for
  the shape and `WM_CLOSE`. So `spawn` blocking the UI thread in `ready_rx.recv()`
  (`thread.rs:177`) and `RenderHandle::drop` joining on the UI thread cannot deadlock today
  (but see L11).

## Axis 2: shutdown and lifetime

What I verified:
- Normal quit: `WM_CLOSE` emits `Closing`, which calls `StopHandle::request`. That sets the
  stop flag and calls `mailbox.stop()` (`wire.rs:86-91`), and only then does `DefWindowProc`
  destroy the window. The loop ends. `drive` sets `stop` and joins the helpers (`app.rs:147-151`),
  and after that `render.shutdown()` joins (`app.rs:101-102`). `Shell` drops after the join.
- Fatal: `on_fatal` posts `WM_CLOSE` unless the app is already `closing` (`wire.rs:131-139`).
  The render thread has already returned, so the join is immediate.
- The watchdog's force exit cannot fire during a normal quit. `stop` is set as soon as the
  message loop returns, and the watchdog is joined inside `drive` before `render.shutdown()`
  (`app.rs:146-149` vs `app.rs:102`). Its 5 s grace (`wire.rs:156`) is cut short by `stop`.
- Window handles: the input window and the WinEvent hook are released in `on_destroy`
  (`window.rs:305-319`), along with the power, WTS and AppBar registrations (`suppress.rs:59-74`)
  and the mask DC/DIB (`hit/mask.rs:141-150`). The timer handle is freed in `clock/timer.rs:87-92`.

LATER findings:
- **L1 A render-thread panic is silent.** `render_main` (`thread.rs:195-226`) has no
  `catch_unwind`, so a panic skips `on_fatal`. The UI keeps running with a frozen island,
  nobody quits, and `RenderHandle::drop` ignores the `Err` from `join` (`thread.rs:151`).
  The idle check even reports a **pass**: the wake count stops growing (`idle.rs:57-60`).
  Fix: wrap the body in `catch_unwind` and route the payload to `on_fatal`.
- **L2 The window is destroyed before the render thread is joined.** `WM_CLOSE` falls through
  to `DefWindowProc` (`window.rs:354`), and the join only happens after the loop
  (`app.rs:102`). One in-flight paint can hit a dead HWND. That triggers `GpuHealth` →
  `recover()`: a full D3D device is created and then `CreateTargetForHwnd` fails on the
  destroyed window. The result is logged as "render stopped while closing" (`wire.rs:132-134`).
  It is bounded and handled, but it builds a device at exit. A cleaner order is
  stop + join inside `WM_CLOSE`, then destroy.
- **L3 (already known)** `WaitForVBlank` has no timeout (`clock/dxgi.rs:65-69`). If it blocks
  during shutdown, `render.shutdown()` hangs the UI thread forever, because the watchdog is
  already joined. This only matters on the DXGI fallback; DComp is first in `auto`.
- **L4 `Stack` drops in the wrong order.** Its fields are `gpu, target, painter`
  (`gpu_paint.rs:13-17`), so the device and DComp device are released before the swap chain,
  visual and brush that depend on them. COM refcounts keep this safe, but declare the
  dependents first. Also, `recover()` (`gpu_paint.rs:74-82`) is unmeasured on hardware: only
  the fake `Paint` in the tests and the `GpuHealth` unit test cover it. Whether
  `CreateTargetForHwnd` on the same HWND right after the old target's release succeeds, or
  returns `DCOMPOSITION_ERROR_WINDOW_ALREADY_COMPOSED` and turns the first device loss into a
  fatal quit, has not been shown.
- **L5 Stale-HWND posts after destroy.** `ShapePoster::post` and `QuitHandle::quit`
  (`poster.rs:28-54`) can post `WM_APP+1`/`WM_CLOSE` to the HWND value after the window is
  destroyed. This lasts until the join, and from the demo thread until `stop`. HWND reuse in
  that window of time is unlikely, but the message would land on a foreign window. L2's
  ordering removes most of it.
- **L6 Logoff and shutdown skip the clean path.** `WM_ENDSESSION` / `WM_QUERYENDSESSION` are
  not handled (`window.rs:332-360`): the process is ended without the render join or the
  AppBar `ABM_REMOVE`.
- **L7 Window classes are never unregistered** (`window.rs:70-88`). This is harmless for one
  run per process. R4 relayout or R7 multi-monitor re-create cases are covered by
  `ERROR_CLASS_ALREADY_EXISTS`.
- **L8 The watchdog limit is not enforced on the arguments.** `--idle-check` has no upper bound
  (`plith-core/src/launch.rs:129-131`). Any value of 74 or more, plus the 1.5 s settle
  (`idle.rs:11`), always trips the 75 s watchdog (`wire.rs:16`) and exits 4. The same applies
  to a long `repeat` demo. Reject this at parse time or derive the watchdog from the run length.

## Axis 3: parking (clean)

What I verified:
- At rest, `wait_newer` (`scene.rs:114-128`) is an untimed condvar wait. Spurious wakeups loop
  inside it without counting, and `wake()` only counts real returns (`thread/run.rs:230-231`).
  Topmost raises, hit updates and timers never publish a scene.
- Suppression (locked, display off, fullscreen) calls `snap` and goes back to the untimed wait,
  both from rest (`run.rs:236-238`) and mid-motion (`run.rs:276-279`). Clock waits are bounded
  (100 ms DComp/timer) only while animating.
- Resume: the next scene sets `aim`. When it is settled (unsuppress or a duplicate), exactly one
  frame is painted without a clock wait (`run.rs:241-245`). Otherwise the clocks restart, the
  timeline resets and the first frame is painted after the first tick. No frame is lost.
  Clocks stay open across parks and cost nothing while unwaited.

LATER findings:
- **L9 The idle check's CPU figure includes its own polling.** In idle-check mode, the idle
  thread and the watchdog each wake every 50 ms (`demo.rs:40-53`, `CHUNK_MS`). So the "ms
  process CPU" in the idle line (`idle.rs:53-58`) includes about 40 wakes/s of the instrument
  itself. The render wake count is unaffected. Use a single `WaitForSingleObject` on an event,
  or subtract a baseline.

## Axis 4: unsafe and FFI hygiene

What I verified:
- A script check found every `unsafe {}` block in non-test code with a `SAFETY:` comment within
  the preceding lines.
- HRESULTs on the hot path are all propagated, none unwrapped. `EndDraw` (`paint.rs:105`) and
  `Present(...).ok()` (`target.rs:125`) feed `GpuHealth`, and `DXGI_STATUS_OCCLUDED` is a
  success code, so it does not count as a failure. Clock errors demote (`run.rs:313-330`), and
  only the last clock's error is fatal.
- `unsafe_op_in_unsafe_fn = "deny"` is on. No `unwrap()`/`expect()` exists outside tests.

LATER findings:
- **L10 The `extern "system"` callbacks have no `# Safety` doc and no `catch_unwind`.** This
  covers `window_proc` (`window.rs:321`) and `foreground_proc` (`topmost.rs:53`). Since Rust
  1.81 an unwind out of them aborts, so this is not UB. But any panic in an app event handler
  (`Inner::emit` → `on_event`, `state.rs:81-92`) kills the process with no log line. Today the
  handlers cannot panic: the mutex poison is handled, and the `RefCell` borrows are not
  re-entered thanks to the `draining` queue. R4 handlers will be bigger. Wrap the `on_event`
  call in `catch_unwind` and log the payload.

## Axis 5: R4 readiness (services via `set_waker`, pages)

On main, R2 services take `set_waker(Box<dyn Fn() + Send + Sync>)` and expect the callback to
post a message to the UI thread (`plith-system/src/brightness/service.rs:327-332`). The engine
has no place for it yet.

- **L11 The UI thread has no generic wake message.** `ShellEvent` is a closed enum
  (`window.rs:39-55`). The message ids `WM_APP+1` (shape) and `WM_APP+2` (AppBar) are
  hard-coded across two files (`poster.rs:10`, `suppress.rs:25`). Add a `UiWaker` handle, like
  `QuitHandle`, that is `Clone + Send + Sync`. It should post a reserved `WM_APP+3`, coalesce
  with an `AtomicBool` like `ShapePoster`, and surface as `ShellEvent::Wake`. That fits
  `set_waker` directly and keeps `plith-shell` free of service types. Also write down the
  invariant that the render thread and the services never `SendMessage` to the UI thread:
  `spawn` blocks the UI thread without pumping (`thread.rs:177`), and the join does the same
  (`thread.rs:151`).
- **L12 Every publish costs a present, even with no change.** `SceneMailbox::update`/`publish`
  always bumps the generation (`scene.rs:83-103`). The loop answers a settled duplicate with a
  full paint and present (`run.rs:241-245`). Once services publish into the scene at meter or
  poll rates, the "parks completely at rest" rule breaks: one present per service tick. Skip
  the bump when `change(scene) == scene`, and give ambient updates their own divisor-paced path
  (spec section 6, frame clock).
- **L13 `Scene` cannot carry content.** It is `Copy` and holds only `expanded`, `page` and
  `suppression` (`scene.rs:28-36`), and `PaintFrame` has no content
  (`paint.rs:15-25`). R4 pages need text, artwork and levels. Plan an `Arc<PageContent>` (or a
  second latest-wins cell) next to the geometry scene, so the condvar handoff stays cheap and
  content updates can be compared and coalesced.
- **L14 The app wiring will not scale.** `run_shell` builds the event handler before the
  renderer exists and patches the stop handle in through an `Arc<OnceLock<StopHandle>>`
  (`app.rs:56-86`, `wire.rs:77-91`). With five services, a display-change render restart
  (`DisplayChanged` is only logged, `wire.rs:108`) and page state, each would need another
  `OnceLock`. A UI-thread-owned `App` struct that the handler reaches through `Rc<RefCell<..>>`
  (the shell is already single-threaded) is easier to grow. Related items:
  - The UI thread never calls `CoInitializeEx`/`OleInitialize`, but the spec says it is an STA
    (section 6) and R5 OLE drag and drop needs that before the window exists.
  - The single thread-local `CURRENT` shell (`window/state.rs:104-115`) allows only one
    `Shell` per thread, which R7 multi-monitor will hit.
  - A `Closing` event delivered while `emit` is already draining is queued, but
    `DefWindowProc` destroys the window before the stop request runs (`state.rs:81-92`,
    `window.rs:354`). R1 handlers never pump messages, so this cannot happen yet. R4 handlers
    that make COM calls could.
