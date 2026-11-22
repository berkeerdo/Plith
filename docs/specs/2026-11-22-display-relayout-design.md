# Relayout on a display change (short design)

Approved by Berke 2026-11-22 ("önerinle devam edelim"). Small change.

## Intent
After an RDP connect, a resolution change or a DPI change the notch keeps the old centre and
size (seen 2026-11-22 over RDP). Today `WM_DISPLAYCHANGE`/`WM_DPICHANGED` only log
`display changed: restart needed` (`crates/plith/src/app/state.rs:208`). The notch must move to
the new top centre of the primary monitor and draw at the new scale, without a restart.

## Goals, non-goals
- Goal: window rectangle, hit region, island origin, swap chain size and DPI scale follow the
  primary monitor after a display change. The frame clock follows a new monitor, refresh rate
  or remote state.
- Non-goal: following a non-primary monitor, multi-monitor choice, the settings window (it
  handles `WM_DPICHANGED` itself), the OSD (separate process, already relayouts).
- Anti-goal: a relayout that activates the notch, changes its z-order, or leaves the render
  thread with a stale clock (motion at the wrong rate over RDP).

## Approaches
1. **Picked: the OSD's method.** Coalesce the messages into one posted `WM_APP`, re-read the
   monitor, move with `SetWindowPos`, send the render thread a relayout that rebuilds through
   the device-loss path. Proven in `plith-osd/src/band.rs` and its render thread.
2. Restart the process on a display change: simple, but loses state, flashes, and the instance
   guard and helper supervision make it noisy. Rejected.
3. Resize the swap chain in place (`ResizeBuffers`) instead of a rebuild: cheaper, but a second
   path to keep correct beside device loss, and a DPI change rebuilds the text formats anyway.
   Rejected for now; a display change is rare.

## Design
1. Shell: `WM_DISPLAYCHANGE`/`WM_DPICHANGED` set a pending flag and post one `WM_APP` message;
   that message emits `ShellEvent::DisplayChanged` from the loop, outside a sent message.
2. App: on `DisplayChanged` re-read `primary_monitor()`, compute `SurfaceLayout::new`. Nothing
   changed (layout, monitor handle, hz, remote): log a `relayout: unchanged` line and stop.
3. Shell `relayout(layout)`: store the layout, `SetWindowPos` (no activation, no z-order),
   re-apply the latest shape so the hit region uses the new layout.
4. Render: a `Relayout { layout, monitor, hz, remote }` request through the render handle. The
   loop takes it between frames, stores the layout (island origin), rebuilds the paint stack
   through `recover` with the new layout, reopens the clock when monitor, hz or remote changed,
   and repaints the current scene.
5. Log: `relayout: rect=x,y wxh scale=.. hz=.. remote=..` from the app, plus the render note.

## Left to the builder
How the request reaches the render thread (a field on the shared state beside the mailbox
wake, or a channel), how the shell stores a mutable layout (`Cell`), test seams for the pure
"did anything change" decision (in `plith-core` or the app).

## Measuring
Console: change resolution and scale in Settings, the notch recentres, a motion line after it
has normal fps. RDP: connect from the Mac, the notch sits at the top centre of the RDP desktop.
