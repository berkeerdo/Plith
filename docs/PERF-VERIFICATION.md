# Performance, measured

What Plith costs when nothing is happening, and which of it has been fixed. Written the day 0.3.0
shipped, because the question "how is performance overall" had never been answered with a number
in this repo.

**Every figure in sections 1 to 4 is a DEBUG build over Remote Desktop, on one machine, with
Voicemeeter not installed.** That is not the shipping configuration and the numbers are not a
product claim. They are enough to find waste, which is what they were taken for.

**Sections 5 and 6 are the exceptions.** Section 5 was taken on Debug, on a Release build from
`bin`, and on a signed Release install running UIAccess. Section 6 was taken on Debug and on
Release from `bin`, not on an install. Each says so where it says the numbers, and section 7 says
how far that generalises, which is: two paths, not a build.

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

## 5. The notch's own first open

Section 6 below used to carry this as the one reported symptom with nothing behind it: a person saw
the mouse go busy for about a second on first use, and the note guessed the likely place was the
first render of Plith's four widget pages. Both halves are now measured. The guess was wrong.

**The instrument.** `NotchOpenTrace` writes one line per open into `plith.log`, and
`scripts/measure-notch-open.ps1` drives the clicks and tabulates them. Four consecutive spans that
PARTITION the open, so adding them up lands on the total and no column can be blamed twice:
`layout`, `defer`, `show`, `settle`.

Beside them, and separate on purpose, a stall probe: a `DispatcherTimer` at Input priority, armed
only between a click and its settle. Input sits below Loaded, Render, DataBind and Normal in WPF's
queue, so that timer cannot tick while any of them is backed up, and cannot tick at all while the
thread is blocked outright. The largest gap between its ticks is therefore not a proxy for the busy
cursor. It is a measurement of it.

The split is the point. The expansion is a deliberate 340 ms quintic, so timing the open alone
would have called that animation a defect and still missed a block underneath it.

**What it costs.** Six runs across four configurations.

| configuration | first open | of which layout | later opens |
|---|---|---|---|
| Debug, from `bin` | 27 ms | 19 ms | 3 ms |
| Debug, from `bin`, again | 36 ms | 18 ms | 11 ms |
| Release JIT, from `bin`, uiAccess off | 26 ms | 18 ms | 7 ms |
| Release installed and signed, clicked by hand | 26 ms | 16 ms | 2 ms |
| Release installed and signed, driven | 33 ms | 21 ms | not captured |
| Release installed and signed, driven again | 28 ms | 14 ms | 3 ms |

The first open costs 26 to 36 ms, of which 14 to 21 ms is the first layout of the four widget
pages. Every open after it costs 2 to 4 ms and lays nothing out. **That is about two per cent of
the reported second, on the exact path the replaced hypothesis named.**

The layout column is the positive control on the whole instrument: large exactly once per process,
small every time after, which is the signature the code predicts. `WidgetHost` is `Collapsed` until
an open and a collapsed element is never measured, so the pages are laid out once, at the first
open, and never again.

Debug and Release sit within the noise of each other here. Worth stating because it is not what
section 6 assumes about Release in general: WPF layout is framework code and is already optimized
in both.

**The stall column says nothing, and says so out loud.** On every first open it reads 25 to 31 ms
over ONE sample. One sample means the probe fired once in the whole window, so the figure is the
width of that window rather than a block inside it. What can honestly be claimed is bounded: no
block longer than about 15 ms occurred, because a longer one would have taken a sample with it.
This is why the line carries the sample count at all. A stall of zero otherwise reads as "nothing
blocked" when it may mean "nothing was ever looked at".

**So where is the second?** Not here, and this run does not say. It was reported as "first use",
which is as consistent with app startup as with the first open, and startup has never been timed.
That is a separate run with a separate instrument. Section 6 is that run, and it found it.

**Three instrument defects, each found by running it rather than by reading it.**

1. **The `INPUT` struct marshalled to 48 bytes instead of 40.** A hand-simplified layout with
   `MOUSEINPUT` inline plus padding, instead of the explicit union the other drivers carry.
   `SendInput` refuses it with Win32 error 87 and no other clue. The union is back, with a comment
   saying why it is not decoration.

2. **The first version of the paint interval measured nothing at all.** It hung off
   `CompositionTarget.Rendering` and reported `0 ms` on every open, including the first. That event
   fires per frame whether or not the content in question was laid out, and the notch is animating
   by then, so it was timing the next frame boundary of an animation already in flight. Replaced by
   `LayoutUpdated`, armed at the CLICK: WPF runs layout at Render priority, above the Loaded
   priority the rest of the open is deferred to, so a handler attached in the deferred callback has
   already missed the pass it exists to time. Thirteen unit tests were green throughout.

3. **The script's own header claimed a signed Release Plith could not be driven.** Measured:
   `SendInput` from a MEDIUM-integrity shell reaches a UIAccess window at HIGH integrity. Accepted,
   no error, the notch opened. UIPI blocks window messages sent AT a higher-integrity window; it
   does not block injection into the raw input stream, which the system then delivers by hit test.
   A claim inherited rather than measured, which is the failure mode this repo has now recorded
   five times, and the first time the wrong claim was inside the instrument's own documentation.

## 6. What a launch costs, and the second is here

Section 7 below used to carry this as the prime suspect with nothing behind it: the notch's own
first open had been cleared at 26 to 36 ms, the report said "first use" rather than "first open",
and nothing had ever timed what happens between launching Plith and the notch being ready. It is
measured now, and the suspect was right.

**A warm launch takes 791 to 1008 ms, and the UI thread is blocked for 878 to 926 ms of it.** That
is the reported second. The notch's own open was about two per cent of it; this is nearly all of
it.

**The instrument.** `StartupTrace` writes one line per launch into `plith.log` and
`scripts/measure-startup.ps1` starts the process and tabulates the lines. Ten consecutive spans
that PARTITION the launch, so adding them up lands on the total and no column can be blamed twice.
What each one covers is documented on `StartupPhase` rather than repeated here.

Beside them, and separate on purpose, `UiStallWatch`: a `DispatcherTimer` at Input priority, ticking
every 250 ms. Input sits below Loaded, Render, DataBind and Normal in WPF's queue, so that timer
cannot tick while any of them is backed up and cannot tick at all while the thread is blocked
outright. The gap between its ticks is therefore not a proxy for the busy cursor. It is a
measurement of it. Unlike section 5's probe, this one is **not armed around one event**: it runs
for the life of the process, because the symptom could not be reproduced on demand and a probe
armed only around a launch would be looking away at the moment it exists to catch.

**What it costs.** Five consecutive launches per build, in milliseconds.

| build | total | clr | app | settings | cards | window | audio | hooks | tray | shelf | settle | stall |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Debug, from `bin` | 823 | 40 | 77 | 22 | 5 | 349 | 43 | 8 | 134 | 55 | 90 | 899 |
| Debug, from `bin` | 811 | 37 | 77 | 22 | 5 | 360 | 38 | 8 | 117 | 56 | 91 | 903 |
| Debug, from `bin` | 1008 | 38 | 76 | 22 | 5 | 340 | 38 | 9 | 117 | 56 | 307 | 897 |
| Debug, from `bin` | 808 | 38 | 77 | 22 | 5 | 344 | 38 | 8 | 128 | 56 | 92 | 881 |
| Debug, from `bin` | 805 | 37 | 76 | 22 | 5 | 360 | 37 | 9 | 114 | 56 | 89 | 896 |
| Release, from `bin` | 812 | 39 | 76 | 22 | 5 | 350 | 37 | 9 | 128 | 58 | 88 | 915 |
| Release, from `bin` | 913 | 37 | 76 | 21 | 5 | 345 | 39 | 8 | 114 | 57 | 211 | 884 |
| Release, from `bin` | 817 | 37 | 78 | 21 | 5 | 348 | 37 | 8 | 122 | 58 | 103 | 895 |
| Release, from `bin` | 809 | 37 | 77 | 22 | 5 | 343 | 37 | 9 | 131 | 55 | 93 | 879 |
| Release, from `bin` | 1003 | 37 | 76 | 21 | 6 | 343 | 36 | 8 | 118 | 54 | 304 | 878 |

Debug and Release sit within the noise of each other, which is the same result section 5 got and a
more surprising one here: section 5 timed WPF's own layout, which is framework code optimized in
both, while every span above is this repo's code calling COM, WinRT, the registry and the display
cable.

**Where it goes.** Three phases carry nine tenths of it:

- **`window`, 337 to 368 ms.** The `OsdHost` constructor, which creates the native banded HWND,
  builds `OsdContent` and constructs all four widget pages, plus `CardHost.Start` and
  `WeatherService.Start`. The largest single thing Plith does at startup by a factor of two and a
  half.
- **`tray`, 114 to 134 ms.** `StartBrightness` and `TrayIconHost.Initialize`.
- **`app`, 76 to 78 ms.** The `App` constructor and `InitializeComponent`, which is WPF parsing
  `App.xaml` and building the resource dictionaries. Before `OnStartup` runs at all.

The remaining six phases total under 170 ms between them, and `cards` and `hooks` are close enough
to zero to be worth stating: constructing the SMTC client, four cards, the weather chain and
`CardHost` costs 5 ms, and installing the low-level keyboard hook, the flyout suppressor, the
foreground watcher and the hotkey costs 8.

**The stall is not a column that can be added to the others**, and it is larger than the total on
every row. It is anchored at the `App` constructor, so it spans `app` through `settle` and past
it, ending at the probe's first tick, which is up to 250 ms after the thread was free. What it
establishes is the shape rather than the exact figure: one unbroken block from the moment managed
UI code got control to the moment the launch finished, with nothing pumping input in between.

**Three things this does NOT say**, and the first is the one most likely to be misread:

1. **Every launch above is warm.** Windows had the binary and its dependencies cached from a launch
   minutes earlier. `clr` reads 37 to 53 ms here, which is low for a .NET WPF process and is a
   measurement of a warm file cache rather than of the runtime. A launch at login, on a machine
   that has just booted, is the case the report came from and is not in this table.
2. **The installed, signed Release build was not measured.** The Plith in `Program Files` is a dev
   build that predates this instrument and writes no line at all. Section 5 measured an install
   and found it within noise, which is a reason to expect the same here and not a reason to claim
   it.
3. **Nothing here says where inside `window` the 350 ms goes.** The phase names a constructor, not
   a cause.

**Two instrument defects, each found by running it rather than by reading it.**

1. **The stall column read `0ms over 0 ticks` on every launch.** The line was formatted at the
   settle, which is posted at ContextIdle, and the probe's first tick is one 250 ms interval away.
   Input priority sits above ContextIdle, so the tick would have won a race, but there was no race:
   the settle routinely arrives first. Honest, because the tick count said plainly that nothing had
   been sampled, and useless, because it always would. Split into `Idle`, which stamps the end of
   the launch, and `Report`, which formats from the probe's tick once there is a gap to report.
2. **The script picked launch lines by counting them.** `DiagnosticLog` rotates at a 512 KB cap,
   and a rotation resets the count, so the script then waits for a file to grow past a length it no
   longer has. It cost the fifth run of the first real Release measurement, which timed out while
   Plith had started and logged normally, and the comment above the defect asserted that counting
   lines was what made it rotation-safe. Lines are selected by their own timestamp now.

**The positive control.** A deliberate `Thread.Sleep(300)` was injected into the `hooks` phase and
the launch re-run: `hooks` went from 8 to 321 ms, the total went up by 358, and the other nine
columns stayed inside the noise of the run before it. The delay appeared in the column it was
injected into and nowhere else.

**What the watchdog costs.** Four timer wakeups a second, for the life of the process, against
section 3 which cut the orchestrator from 33 a second to 2 precisely because wakeups matter on a
laptop. It is worth paying while an unexplained block is being hunted. It is the first thing to
reconsider once one is not.

## 7. What has never been measured

- **The Release build, apart from two paths.** Everything above is Debug except sections 5 and 6.
  Section 5 was taken on Debug, on a Release build from `bin`, and on a signed Release install, and
  found the three within noise of each other; section 6 on Debug and Release from `bin`, likewise
  within noise. That is two paths measured on Release, not a build.
- **A cold launch.** Every launch in section 6 is warm: the binary and its dependencies were in
  the file cache from a launch minutes earlier, and `clr` reads 37 to 53 ms because of it. The
  case the report came from is a launch at login on a machine that has just booted, and it is not
  in that table. It needs a reboot between runs, which is the one thing none of these instruments
  can drive.
- **What is inside the `window` phase.** Section 6 found 337 to 368 ms in the `OsdHost`
  constructor, which is the largest single thing Plith does at startup and a phase name rather
  than a cause. It builds a banded native window and all four widget pages; nothing says which.
  That is where to look next, and it is the first place worth trying to move.
- **Long-run memory.** The longest sample here is five minutes. A leak does not show in five
  minutes, and nothing in this repo has ever run the app for a day and looked.
- **GPU and the render thread.** All the sampling above is CPU time per thread. Every stamp in
  section 5 is taken on the UI thread too, so none of it sees the frame the monitor scanned out.
- **Whether moving any of it is worth doing.** Section 6 says where the launch's time goes; it does
  not say that a person minds. Plith starts at login on the machine it was measured on, so the
  block lands while Windows is still bringing the desktop up. The report was "first use", which
  may have been a launch by hand.
