# Performance, measured

What Plith costs when nothing is happening, and which of it has been fixed. Written the day 0.3.0
shipped, because the question "how is performance overall" had never been answered with a number
in this repo.

**Every figure here is a DEBUG build over Remote Desktop, on one machine, with Voicemeeter not
installed.** That is not the shipping configuration and the numbers are not a product claim. They
are enough to find waste, which is what they were taken for.

## 1. The resting cost

Two processes, both idle, notch at rest, nothing playing.

| | Plith | Plith.DropCatcher |
|---|---|---|
| Working set | 209 MB | 114 MB |
| Private bytes | 130 MB | 67 MB |
| Handles | 983 | 598 |
| Threads | 23 | 28 |
| Idle CPU, before the fixes below | 1,2 to 1,4 % of one core | 0,0 % |
| Idle CPU, after | 0,2 to 0,8 % of one core | 0,0 % |

The spread on the last row is real and is left in rather than averaged away: three consecutive
ten-second samples of the same build gave 0,81, 0,79 and 0,15 per cent. Anything quoted here as a
single number would be a number this instrument cannot produce.

The catcher costs nothing while it waits, which is worth stating because it is a whole second WPF
process: 114 MB of working set and no measurable CPU.

## 2. What the idle CPU actually was

Two hypotheses were wrong before the right answer arrived, and both are recorded because each one
looked obvious.

**WRONG: the hover poller.** `NotchHoverPoller` ticks every 60 ms for as long as the ambient notch
is the presentation, which is nearly always, and a 17 Hz timer is the first thing anyone would
suspect. MEASURED by raising its interval to 1000 ms, a seventeenfold cut: idle CPU went from
1,38 % to 1,22 %. The poller is worth about 0,16 points, so it was never the cost. Restored to
60 ms, where it belongs: the notch has to feel responsive to a deliberate move toward it, and this
polls `GetCursorPos` precisely so Plith does not carry a second global mouse hook.

**PARTLY RIGHT: a registry probe on a 33 Hz timer.** `OsdOrchestrator` polls every 30 ms, and its
tick ended with:

    if (VoicemeeterClient.IsInstalled && !_voicemeeter.IsLoggedIn && DateTime.UtcNow >= _nextReconnect)

`IsInstalled` opens two registry keys and stats a directory and a file. `&&` evaluates left to
right, so the expensive clause ran on EVERY tick and the three-second rate limit beside it could
never take effect: about 33 registry opens and 66 file system calls a second, for the entire life
of the process, on any machine with Voicemeeter installed but closed. FIXED by testing the clock
first. MEASURED: 1,38 % to 1,10 %.

That is a smaller win than the defect deserves, and the reason is this machine: Voicemeeter is not
installed here, so `TryGetVoicemeeterInstallPath` failed on the first registry open every time. On
a machine that HAS it installed and closed, the same code path opens the key, reads a string,
combines a path and stats a file, 33 times a second. The fix is worth more there than it measured
here, and that cannot be measured from this machine at all.

**THE REST: spread across the UI thread.** Per-thread sampling over 20 s windows shows one thread
carrying essentially all of it, and it is the UI thread: 172 ms per 20 s before the fixes, which is
0,86 % of a core. Slowing individual timers moves it only in fractions of a point. Finding what is
left needs a sampling profiler, and it was not chased further: the remaining figure is under one
per cent of one core, and the two defects above were found by reading the code the profile pointed
at rather than by narrowing the profile.

## 3. The poll rate now follows the source

`OsdOrchestrator` polls at 30 ms only while Voicemeeter is the active source AND logged in, and at
500 ms otherwise. Voicemeeter needs the fast rate because its API has no callback:
`VBVMR_IsParametersDirty` is an edge-triggered latch that has to be consumed. Windows Core Audio
has callbacks, so on that source the tick had no volume work to do at all; it reconciled which
source should be active and compared a clock, 33 times a second, forever.

The rate is chosen after `ReconcileActiveSource` rather than where `_activeSource` is assigned,
because it has to follow `IsLoggedIn` too: a Voicemeeter engine can die while it is still the
desired source, and polling a latch nobody holds is the same waste as polling on a machine that
never had it.

MEASURED: 1,10 % to the 0,2-0,8 % band above. It is also the one change here that reduces timer
WAKEUPS, from 33 a second to 2, which matters on a laptop for reasons CPU per cent does not show.

**Not measured, and it needs saying: Voicemeeter itself.** This machine has no Voicemeeter, so the
fast-rate path and the transition into it were never exercised on hardware. What was exercised is
the fallback source end to end: a real volume key, a Core Audio notification, and the OSD on screen
(`Volume notification: 50%` then `Show: transition at 1116,0`).

## 4. Latency, from the shelf work

| | measured |
|---|---|
| Shelf's first open after a login | 85 ms end to end (was about 200) |
| Every open after it | 12 ms |
| Page turn onto the shelf, end to end | about 340 ms: a 260 ms slide, then the handover, then a 70 ms fade |

Full ledgers for these in `docs/SHELF-VERIFICATION.md` sections 10.27 and 10.28.

## 5. What has never been measured

- **The Release build.** Everything here is Debug, which carries checks Release does not.
- **Long-run memory.** The longest sample here is five minutes. A leak does not show in five
  minutes, and nothing in this repo has ever run the app for a day and looked.
- **GPU and the render thread.** All the sampling above is CPU time per thread.
- **The notch's own first open.** A person reported the mouse going busy for about a second on
  first use. The shelf handover accounts for 85 ms of that and nothing else here accounts for the
  rest, so the likely place is the first render of Plith's four widget pages inside its own layered
  window. That path is animation-driven and timing it means timing a first render rather than a
  method; no instrument for it exists.
