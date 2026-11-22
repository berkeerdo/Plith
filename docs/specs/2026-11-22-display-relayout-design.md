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
As built (2026-11-22; the first draft had the app re-read the monitor, the shell does it):
1. Shell (`plith-shell/src/window/display.rs`): `WM_DISPLAYCHANGE`/`WM_DPICHANGED` post one
   `WM_APP_RELAYOUT` per burst. Its handler re-reads the primary monitor and the remote state;
   an unchanged display does nothing. Otherwise it stores the new `Display`, moves the window
   (`SetWindowPos`, `SWP_NOACTIVATE`, `HWND_TOPMOST` as at creation), clears the cached
   region and re-applies it, then emits `ShellEvent::DisplayChanged(Display)`.
2. App: logs `Display::log_line` and forwards the display through `Relayouter`.
3. Render (`RelayoutHandle`, `run/relayout.rs`): the handle stores a `Relayout { layout, hz,
   order }` in `LoopShared` (`order` only when monitor, hz or remote changed; it also moves the
   clock target the next clock opens for) and nudges the mailbox. The loop applies it after
   each wake, each motion frame, each ambient tick and before it parks: stores the layout,
   rebuilds the stack through `Paint::relayout` (the recover path), on failure WARP, then
   fatal; reopens the clock on a new order; repaints a still, unsuppressed island.
4. Log: `relayout: rect=x,y wxh scale=.. hz=.. remote=..` from the app, plus
   `render relayout: wxh scale=.. hz=.. clock=kept|reopened` from the render thread.

## Left to the builder
How the request reaches the render thread (a field on the shared state beside the mailbox
wake, or a channel), how the shell stores a mutable layout (`Cell`), test seams for the pure
"did anything change" decision (in `plith-core` or the app).

## Measuring
Console: change resolution and scale in Settings, the notch recentres, a motion line after it
has normal fps. RDP: connect from the Mac, the notch sits at the top centre of the RDP desktop.
