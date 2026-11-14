# 0005 Swap chain buffers and latency

Date: 2026-11-14. Status: accepted. Drafted by lane `engine` from the R1 console session
(release build, RTX 5080, 240 Hz, `%LOCALAPPDATA%\Plith\plith.log`); numbers copied from the log.

**Decision:** 2 buffers, `FLIP_SEQUENTIAL`, premultiplied alpha, `IDXGIDevice1::
SetMaximumFrameLatency(1)`, `Present(1, 0)` after the clock wait, no waitable object.

**Measured:** present p99 0.27 to 0.41 ms with dcomp and dxgi in every clean run; draw never
waited on DWM in a way the log shows. 3 buffers were not needed. Revisit together with the
P1 game-load gap.
