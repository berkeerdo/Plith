# R1 ADR drafts (P1 to P5) for the master to file

Lane `engine`, 2026-11-14. Each draft is a builder call from spec section 11, decided by the
Task 10 console session with Berke (release build, console session 1, RTX 5080, 240 Hz primary
monitor, `%LOCALAPPDATA%\Plith\plith.log`). Measured = log lines and Berke's own words; the
numbers below are copied from the log, not rounded up.

## P1 Frame clock source

**Decision:** `DCompositionWaitForCompositorClock` is the default, `IDXGIOutput::WaitForVBlank`
second, a high-resolution waitable timer last; the timer only over RDP. The render thread joins
MMCSS `Games`.

**Measured (same `basic` demo, 15 motions each, 21:04):**

| Clock | fps | Hitches | Draw p99 ms | Present p99 ms |
|---|---|---|---|---|
| dcomp | 240.0 | 0 | 0.22 to 0.30 | 0.28 to 0.35 |
| dxgi | 240.0 | 0 | 0.21 to 0.31 | 0.28 to 0.40 |
| timer | 240.0 | 0 | 0.19 to 0.43 | 0.27 to 1.73 |

DComp drew every motion in auto mode (no demotion seen in any run). Timer present p99 grows up
to 1.73 ms, so it stays the last resort.

**Rules that go with it:**
- Phase lock tracks the earliest wake-up (a vblank wait only returns late): `e < 0` snaps the
  grid, `e >= 0` moves it by `0.005 * e`; the period stays nominal. Oyenza's symmetric phase
  and period gains park the grid at the mean lateness and drop valid early wake-ups; this
  deviates on purpose.
- The stamper's `dt` is always `periods * period`; the first step after rest is one period. A
  stall over 30 periods resyncs but keeps its whole length, so it counts as a hitch.
- Demotion after 3 tick-to-tick intervals under 0.15 period (a slow frame never demotes); each
  motion starts again on the first source.
- `WaitForVBlank` has no timeout, so a shutdown can wait one vblank (it blocks only if the
  output stops producing vblanks). DComp and timer waits are bounded at 100 ms.
- `DCompositionWaitForCompositorClock` is linked statically: Windows 11 build 22000 or later.

**Known gap:** with a borderless game running behind (Deadlock), 13 of 25 motions showed 1 or 2
single missed vblanks (worst gap 8.3 ms, 235 to 240 fps) while draw p99 stayed at or under
about 0.9 ms. Every run without a game: 0 hitches. First item of the next engine job.

## P2 Hit testing

**Decision:** one DirectComposition window (`WS_EX_NOREDIRECTIONBITMAP`) with a `SetWindowRgn`
round-rect region **3 px outside** the drawn shape, applied on the UI thread from a shape the
render thread posts (coalesced) every painted frame. Oyenza: two windows and a 30 Hz mask.

**Measured:**
- Region at 1 px outset (21:05): clicks inside toggle, clicks outside reach the window below,
  but Berke: "köşeler pürüzsüz değil" (corners not smooth); the GDI region clips the
  anti-aliased edge.
- Region at 3 px outset (21:14): Berke: "evet koşler ürüzssüz evet sıkıntı yok açılıp
  kapanıyor her soru evet çalışıyor" (corners smooth, outside clicks pass, toggle works).
- UI-thread cost per region update, 14 runs from 20:23 to 21:30: mean 0.43 to 0.78 ms per run,
  per-run max 0.5 to 6.8 ms, except 33.2 ms once in the lock run (most likely the session
  switch around lock or unlock; the render thread is separate and that run had 0 hitches);
  0 errors in 30,229 updates.
- Split (separate input window, layered click-through visual): Berke saw no island in its one
  run; the run may have closed before he looked, so it is unverified and dropped. Mask uses the
  same layered visual window and was not run.
- Trade: clicks within 3 px outside the edge count as the island. Pen and touch: not tested
  (no pen or touch screen used).

## P3 Swap chain buffers and latency

**Decision:** 2 buffers, `FLIP_SEQUENTIAL`, premultiplied alpha, `IDXGIDevice1::
SetMaximumFrameLatency(1)`, `Present(1, 0)` after the clock wait, no waitable object.

**Measured:** present p99 0.27 to 0.41 ms with dcomp and dxgi in every clean run; draw never
waited on DWM in a way the log shows. 3 buffers were not needed. Revisit together with the
P1 game-load gap.

## P4 Scene handoff between threads

**Decision:** a latest-wins mailbox (`Mutex` + `Condvar` + generation): the UI and demo threads
`update` it atomically, the render thread parks in `wait_newer` at rest and reads `latest()`
each frame while moving. Not a channel: intermediate targets are meaningless.

**Measured:** `--idle-check 60`: 0 render wake-ups in 60 s, 62.5 ms process CPU (G2). Display
off, lock and fullscreen each parked the render thread mid-motion with no motion lines until
return (display off 3.6 s at 21:24:02; lock 18.5 s at 21:29:11; fullscreen during Deadlock).

## P5 UI layer shape (retained or immediate)

**Decision:** deferred to R4; R1 draws no widgets (one placeholder bar per page). R4 opens with
this ADR, with the offscreen content layers (group opacity, text rasterised once) from spec
section 6.
