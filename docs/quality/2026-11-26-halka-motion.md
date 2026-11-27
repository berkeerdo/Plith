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

## Rerun after the tuning (2026-11-27, console, Berke at the PC)
Release build of `f6cccfe` plus the pill change below, 240 Hz, scale 1, `remote false`.

| Run | Motions | 240.0 fps | Hitches | Worst gap | Draw p99 | Present p99 |
|---|---|---|---|---|---|---|
| demo halka (audio, media, welcome x4) | 12 expand, 12 collapse | 22 of 24 | 2 expands: 1 and 16 | 8.3 ms | 0.26 to 0.77 ms | 0.12 to 4.19 ms |
| normal halka, Berke's pointer (12:27) | 14 expand, 14 collapse, 16 page | 42 of 44 | 2 pages: 2 and 1 | 8.3 ms | at most 1.03 ms | at most 4.17 ms |
| normal, live switch (13:00) | 25 | 24 of 25 | 1 collapse: 1 | 8.3 ms | | |

- The 16-hitch expand is one run's present stall (present p99 4.19 ms, draw p99 0.32 ms); it
  did not come back in 39 later expands. Not chased further.
- Live switch from the settings window: Halka to Gece while collapsed (13:00:31), Gece to
  Halka while open (13:00:35); every motion after it logs the new theme, no error or warn line.
  The UI-thread art decode on the switch is still not timed.
- `--idle-check 60 --theme halka --fixture banana`: 1 render wake-up (allowed 2; the Karot
  look's minute tick), 140.6 ms process CPU, exit 0.
- Berke: the slower ripples now read ("evet gözüküyor"). The welcome label was not remarked on.

## The idle pill's cut rim (2026-11-27)
What looked off-centre was the pill raster's own rim: its edge curves in from the top (inset
5, 12, 26 px at rows 16, 24, 31) while the island's corner is a 16 DIP arc (0, 2, 10), so its
light rim sat inside the island's edge and border and ended short of them. Fix (Berke chose
"draw it from the shape"): the raster is gone; the collapsed Halka pill is the collapsed
palette's gradient and border on the island's own outline, like Gece. A screen capture after
the fix shows one edge on the 16 DIP arc. Regression test
`the_pill_corner_has_no_rim_inside_the_edge`.
