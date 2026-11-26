# Halka motion on the console (plan 2 Task 7)

Measured 2026-11-26 19:28 to 19:31 and 20:06, console session (`tscon` from RDP; log line
`remote false`), 240 Hz, release build of `5b54152`, latency 2, tune brisk, scale 1.

## Demo runs (`--fixture banana`, audio, media, welcome expand and collapse in turn)

| Theme | Motions | 240.0 fps | Hitches | Worst gap | Draw p99 | Present p99 |
|---|---|---|---|---|---|---|
| halka | 25 expand, 25 collapse | 48 of 50 | 2 collapses: 6 and 1 | 8.3 ms | 0.24 to 0.75 ms | 0.11 to 0.20 ms |
| gece | 24 expand, 24 collapse | 47 of 48 | 1 collapse: 1 | 8.3 ms | 0.19 to 1.28 ms | 0.11 to 0.19 ms |

- The Halka run planned 10 passes but the 75 s demo watchdog ended it after 8 and a third
  (exit 4); the Gece run used 8 passes and exited 0.
- Every hitch is one missed vblank at the collapse (present 4.0 ms on the frame before the
  gap); the known collapse hitch, not Halka's. Expand never hitched in either theme.
- Ring-clip cost: Halka draw p50 0.12 to 0.38 ms against Gece's 0.10 to 0.43 ms. No
  measurable cost, so the spec's cut order (Step 4) was not needed.

## Normal run (`--theme halka`, real services, Berke's pointer)
Nine motions at 20:06 to 20:07: all 240.0 fps, 0 hitches, worst gap 4.2 ms, draw p99 at most
0.82 ms. Prewarm of pages 0 and 4 at first open 4.94 ms.

## What Berke saw
- The ripples were too fast to read ("açılış animasyonu anlaşılmıyor, çok hızlı"). Tuned on
  `feature/halka-tune`: `RIPPLE_FROM` 0.10 / 0.35 / 0.60, `PRESS_SPAN` 0.25, `HOLD_MS` 40,
  `FADE_MS` 120. Page now 312.5 ms after the open starts (fake clock), 162.5 ms before; test
  budgets 170 to 320 ms. Not yet seen by Berke; no console rerun after the tuning yet.
- The welcome song label lay over the date ("PER 26 KAS") when the last play started in the
  left part of the run: label rows y 104 to 130 share the date's rows y 100 to 112. Fixed on the
  same branch: the label flips right of the play start, never left of the date's end.
- The idle pill looks off-centre and its "icon design" is bad (Halka, Karot look). Screen
  capture: text 16 px from the left edge, about 14 px from the right, vertical centre within
  1 px; what reads as off-centre is not yet known.

## Not measured
- Live theme switch while open (Halka to Gece and back) and the UI-thread art decode on it.
- 0 render wakes at rest after the runs (no `--idle-check` run).
