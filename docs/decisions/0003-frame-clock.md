# 0003 Frame clock source

Date: 2026-11-14. Status: accepted. Drafted by lane `engine` from the R1 console session
(release build, RTX 5080, 240 Hz, `%LOCALAPPDATA%\Plith\plith.log`); numbers copied from the log.

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
