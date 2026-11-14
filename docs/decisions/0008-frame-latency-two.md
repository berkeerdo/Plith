# 0008 Frame latency 2, no priority levers

Date: 2026-11-14. Status: accepted. Supersedes the latency line of ADR 0005; adds to ADR 0003.
Drafted by lane `engine` from the R1b console session with Berke (release build, RTX 5080,
240 Hz, Deadlock running behind the desktop, `basic` demo, 15 motions per run,
`%LOCALAPPDATA%\Plith\plith.log`); numbers copied from the log.

**Decision:** `IDXGIDevice1::SetMaximumFrameLatency(2)` replaces 1. Still 2 buffers,
`FLIP_SEQUENTIAL`, `Present(1, 0)` after the clock wait. No priority lever is on by default:
GPU thread priority, process scheduling class and MMCSS priority stay normal.

**Why:** with latency 1, Present waits until the previous frame has left the GPU; with a
borderless game behind, that wait took a whole period, and every hitch was that wait.

| Setting | 0-1 hitch motions | Hitches | Present p99 max | Min fps |
|---|---|---|---|---|
| latency 1, sync 1 (R1) | 11/15 | 21 | 4.35 ms | 227.6 |
| latency 2, sync 1 | 15/15 (and 30/30 in two tune runs) | 0 | 0.24 to 1.56 ms | 240.0 |
| latency 2, sync 0 | 30/30 (two runs) | 0 | 0.23 to 0.38 ms | 240.0 |
| latency 1, sync 0 | 15/15 | 0 | 3.40 ms | 240.0 |

Sync 0 hides the wait but keeps it (present up to 3.4 ms); latency 2 removes it, so sync stays 1.
Berke on the latency 2 run: "herhangi bir takılma yoktu".

**Priority levers, measured and rejected** (baseline 11/15 clean, 21 hitches):
`IDXGIDevice::SetGPUThreadPriority(7)` 10/15, 15; `D3DKMTSetProcessSchedulingPriorityClass
(ABOVE_NORMAL)` 12/15, 11; MMCSS `AvSetMmThreadPriority(CRITICAL)` 11/15, 12. All three were
accepted without elevation and gave no gain; Plith never takes from the game. They stay as
launch flags (`--gpu-priority`, `--sched-class`, `--mmcss`) for later measurement.

**Consequences:** when the GPU is behind, one more frame (4.2 ms at 240 Hz) may queue.
`DwmGetCompositionTimingInfo` counters do not track refreshes on this Windows 11 build (about 1
per second), so the motion line carries no DWM delta and what reaches the screen is checked by
eye or by PresentMon (not yet run). Game cost is untestable until R4: the island parks while a
game is in front.
