# Performance, measured

What Plith costs when nothing is happening, and which of it has been fixed. Written the day 0.3.0
shipped, because the question "how is performance overall" had never been answered with a number
in this repo.

**Every figure in sections 1 to 4 is a DEBUG build over Remote Desktop, on one machine, with
Voicemeeter not installed.** That is not the shipping configuration and the numbers are not a
product claim. They are enough to find waste, which is what they were taken for.

**Sections 5 to 8 are the exceptions.** Section 5 was taken on Debug, on a Release build from
`bin`, and on a signed Release install running UIAccess. Sections 6 to 8 were taken on Debug and
on Release from `bin`, not on an install. Each says so where it says the numbers, and section 9
says how far that generalises, which is: two paths, not a build.

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

Every figure in this section was taken by hand. `scripts/measure-idle-cost.ps1`, added by section
6b, is the instrument that does it repeatably, and it samples a second axis this table does not
have: context switches per second, which is how often the process wakes a thread. That axis sees
timers this one cannot, and section 6b is the case that shows the difference.

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

Section 9 below used to carry this as the one reported symptom with nothing behind it: a person saw
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

Section 9 below used to carry this as the prime suspect with nothing behind it: the notch's own
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
measurement of it. When this section was written it ran for the life of the process, unlike
section 5's probe, because the symptom could not be reproduced on demand and a probe armed only
around a launch would be looking away at the moment it exists to catch. **It is now armed around
the launch and stops on the tick that reports it**, and section 6b is the measurement that closed
that decision. Every number in this section was taken with the probe still running for the life of
the process; the launch line and its stall column are unchanged by the retirement, which 6b shows.

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

- **`window`, 337 to 368 ms.** The `OsdHost` constructor, which creates the native banded HWND
  and builds `OsdContent`, plus `CardHost.Start` and `WeatherService.Start`. The largest single
  thing Plith does at startup by a factor of two and a half. This line said "and constructs all
  four widget pages" until section 7 split the phase and found it constructs one; the correction
  is recorded there rather than silently applied here.
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
reconsider once one is not. That condition was met once sections 7 and 8 landed, and section 6b is
what was done about it.

## 6b. Retiring the watchdog, on section 3's terms

Section 6 left this as an open item and named the terms rather than the answer: the block is
explained, so `UiStallWatch` had become four wakeups a second in exchange for a number nobody was
reading, and retiring it or arming it around the launch was to be **held to section 3's own
standard, which is to measure the idle wakeups and the idle CPU before and after**. Both are
measured here.

**What was changed.** The probe is armed at the end of `OnStartup` as before and stops on the tick
that reports the launch. The launch keeps its stall column on every run, which is the only figure
that measures the reported symptom. A block an hour into a session is now caught by nothing, and
that is the whole of what was given up.

It stops on the tick that REPORTS rather than on the first tick, and that is not cosmetic. The
settle is posted at ContextIdle and Input priority preempts it, so on a launch whose settle outruns
one interval the first tick finds nothing to report and the second one does. Stopping on the first
tick would drop the launch line on exactly the slow launches it exists to describe.

**The instrument, because section 3 had none.** Its 33-to-2 figure was taken by hand and left
nothing behind. `scripts/measure-idle-cost.ps1` is that instrument now. It samples a process at
rest on two axes: CPU as a per cent of one core from the process's own `TotalProcessorTime`, and
**wakeups as context switches per second**, per thread, from
`Win32_PerfRawData_PerfProc_Thread`. A `DispatcherTimer` tick wakes a thread that was asleep and
that wake is a context switch, so a 250 ms probe shows up on the second axis at a magnitude the
first one cannot see. `Get-Counter` is not used and the script says why: `\Thread(plith/*)\Context
Switches/sec` does not filter on this machine, matching all 13.000 threads on the system and
returning a six-figure total.

**What it measured.** Debug from `bin`, app at rest, ten samples of ten seconds per row. A/B/A/B
rather than A/B, because the first pair alone would not have been believable at this spread.

| build | probe | ui cs/s, median | ui cs/s, range | cpu %, range |
|---|---|---|---|---|
| before | 4/s, for life | 173,1 | 130,4 to 190,9 | 0,08 to 0,68 |
| before, again | 4/s, for life | 168,7 | 154,5 to 199,9 | 0,08 to 6,70 |
| after | none after the launch | 150,6 | 138,7 to 156,9 | 0,08 to 0,59 |
| after, again | none after the launch | 149,2 | 142,4 to 160,2 | 0,17 to 0,76 |
| positive control | 40/s, for life | 306,8 | 275,2 to 350,3 | 0,17 to 0,84 |

**The wakeup axis sees it and the CPU axis does not.** Both before rows land at 169 to 173 and both
after rows at 149 to 151, so the probe was worth about **21 context switches a second on the UI
thread**, which is about an eighth of the idle total. The CPU column moves by nothing: the four
ranges overlap completely, and the one reading of 6,70 per cent is a single sample of one before
run that some other work landed in. This is exactly the case section 3 asserted without being able
to show it, that wakeups matter for reasons CPU per cent does not show, and it is the first time
this repo has had both axes on the same change.

**The positive control is what makes the 21 readable.** The two distributions overlap: the lowest
before sample, 130,4, is under every after sample. A median that moved 21 with that much spread is
not on its own a measurement. So the probe was rebuilt at 25 ms, a tenfold version of itself at
40 wakeups a second, and measured the same way: 306,8, which is 136 above the before median. That
gives **3,8 context switches per tick**, and the before-to-after drop gives 5,3 for the same
quantity. Two independent estimates of the same per-tick cost, one from a lever ten times larger
than the effect being claimed. The instrument is linear in the thing it is being asked about.

**The number nobody was reading, measured rather than assumed.** The retained `plith.log` at the
time of this run held **79 launch lines and 79 stall lines, and not one stall line sat more than
two seconds from a launch line.** Over every launch in that log the probe never once reported a
block that was not the launch itself. That is the evidence for "nobody was reading it" and it is
weaker than it looks: `DiagnosticLog` rotates at 512 KB, so that window is one day, and it is a day
of launches driven by `measure-startup.ps1` rather than a day of ordinary use.

**The launch line is unchanged**, which is the regression this change could have caused and did
not. Three runs after it: 897, 864 and 870 ms total, with `stall 965`, `934` and `940` ms **over 1
tick**, and the window sub-total landing on the `window` column on all three. The tick count is now
1 on every launch by construction rather than by luck, and it is still printed, because a stall
figure without its sample count reads as "nothing blocked" when it may mean "nothing was looked
at".

**Three things this does NOT say.**

1. **It does not say the UI thread is quiet.** It wakes **about 150 times a second with the probe
   gone**, and nothing here explains what the other 150 are. The probe was 21 of them. That is a
   new open question and it is in section 9.
2. **It does not measure Release, or a laptop on battery.** Debug from `bin`, on the one desktop
   every other number here comes from. The wakeup argument section 3 makes is about battery, and
   no measurement in this repo has ever been taken on one.
3. **It does not claim a person would notice either state.** An eighth of the idle wakeups of a
   process that costs well under one per cent of one core is a real quantity and a small one. What
   justifies the change is that the thing being paid for was no longer being read, not that the
   payment hurt.

**Re-arming it is one line** if a block is ever reported again, and the comment on
`App.StartStallProbe` keeps the old reason intact rather than deleting it, because the old reason
was a good one for as long as the symptom was unexplained.

## 7. Inside the `window` phase, and the largest thing in the launch

Section 9 used to carry this as the next thing to look at: section 6 found 337 to 368 ms in the
`window` column, the largest single span of the launch by a factor of two and a half over the next
one, and named it with a constructor rather than a cause. It is measured now, and the answer moved
the question somewhere the obvious fix does not reach.

**Three of the ten spans carry the whole phase**, and each one is a single call.

**The instrument.** `StartupWindowTrace` writes a second line per launch, immediately after the
launch line and from the same probe tick, and `scripts/measure-startup.ps1` tabulates it under the
first table. Ten spans that PARTITION the `window` column, the same property the launch's own ten
spans hold. Its zero is a clock read taken one statement after `StartupTrace` marks `cards`, and
its last mark one statement before `window` is marked, so the two instruments measure the same
interval from opposite sides. **That is the positive control on the pairing, and it is checked
rather than eyeballed**: the script warns when `sum` and `window` differ by more than 2 ms. Across
every run below they were identical on every row.

**What it costs.** Five launches per build, in milliseconds.

| build | window | fields | shell | band | content | pages | accent | hwnd | present | host | weather |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Debug, from `bin` | 373 | 147 | 0 | 2 | 12 | 12 | 0 | 144 | 13 | 1 | 42 |
| Debug, from `bin` | 370 | 144 | 1 | 2 | 12 | 12 | 1 | 143 | 12 | 1 | 42 |
| Debug, from `bin` | 363 | 142 | 0 | 2 | 12 | 11 | 1 | 138 | 14 | 1 | 42 |
| Debug, from `bin` | 361 | 141 | 1 | 2 | 12 | 12 | 1 | 138 | 12 | 1 | 41 |
| Debug, from `bin` | 365 | 143 | 1 | 1 | 13 | 11 | 1 | 139 | 13 | 1 | 42 |
| Release, from `bin` | 368 | 141 | 1 | 1 | 11 | 11 | 1 | 138 | 13 | 1 | 50 |
| Release, from `bin` | 360 | 143 | 1 | 1 | 12 | 11 | 1 | 133 | 12 | 1 | 45 |
| Release, from `bin` | 371 | 146 | 1 | 1 | 14 | 10 | 1 | 138 | 15 | 1 | 44 |
| Release, from `bin` | 355 | 140 | 1 | 1 | 13 | 10 | 1 | 133 | 12 | 1 | 43 |
| Release, from `bin` | 356 | 144 | 0 | 2 | 11 | 11 | 1 | 133 | 12 | 1 | 41 |

Debug and Release sit within the noise of each other for the third time in this document, which is
now the expected result rather than a finding.

**Where it goes.**

- **`fields`, 140 to 147 ms.** OsdHost's field initializers and the `BandWindow` base constructor:
  everything that runs before the first statement of the constructor body, which is the earliest
  point a mark of ours can reach. Read the heading below before drawing any conclusion from this
  name, because the name is misleading and the A/B is what says so.
- **`hwnd`, 133 to 144 ms.** `CreateWindow`: the native top-level layered window and the
  `HwndSource` over it. A single call into `BandWindow`, and a cause rather than a name.
- **`weather`, 41 to 50 ms.** `WeatherService.Start`.
- **The other seven total under 45 ms between them**, and two of those are worth stating because
  they were the suspects going in. Building the widget pages costs 10 to 12 ms, and
  `ApplyPresentationMode`, which reshapes the content, measures it through `Reposition` and parks
  the strip, costs 12 to 15.

**The widget pages were never the cost, and the earlier description of this phase was wrong.**
Section 6 said the `window` phase constructs all four widget pages. It constructs ONE, the clock;
the weather, media and shelf pages are installed by `AttachAudioSource` and `AttachShelf`, which
run after the phase is over, inside the `audio` column. A claim inherited from section 5 — where
four pages really are laid out, because by the time the notch opens all four exist — rather than
measured where it was repeated. That is the failure mode this repo has now recorded six times, and
this is the first time it travelled between two sections of the same document.

**The 127 ms belongs to the first XAML load, not to the control that pays it.** `fields` is a grab
bag of five allocations plus a base constructor, so it was split further in a throwaway build:
`new WidgetFrame()`, the one field initializer that loads XAML, was moved into the constructor body
with a mark of its own. It read **126 to 128 ms**, and the remaining four allocations plus both
`BandWindow` constructors read **16**.

That looked like an actionable answer, and it was not. `OsdContent`, the next XAML control built,
read 12 ms right after it, so the two were A/B'd by swapping which one is constructed first:

| order | first control | second control |
|---|---|---|
| frame first | `WidgetFrame` **126 to 128** | `OsdContent` 12 |
| content first | `OsdContent` **133 to 135** | `WidgetFrame` 2 to 3 |

**The cost follows the position, not the control.** Whichever XAML control this process loads first
pays about 130 ms, and the second pays ten. So it is the first BAML load, the merged resource
dictionaries being built, and the JIT of that path. None of that is `WidgetFrame`, and none of it
is removed by making `WidgetFrame` cheaper or by building it lazily: **deferring it moves the
130 ms to whatever loads next.** That is the fix this measurement rules out, and ruling it out is
most of what the measurement was for.

Both experimental splits were reverted. What ships is the ten-span instrument above, with
`_widgets` back where it was as a field initializer, so nothing in OsdHost is shaped by the
instrument beyond ten calls to `Mark`.

**The A/B needed a second run to be readable, and the reason is worth recording.** The first
attempt built the frame after the content but left the phase ORDER alone, and the trace clamps a
mark that arrives out of order to the previous boundary. So `frame` absorbed everything between
and `content` read 0. The clamp did exactly what it is built to do, which is refuse to report a
negative span; what it cannot do is tell a reader that the marks, rather than the launch, were the
thing that changed. Reordering the enum to match the code made the run readable.

**The positive control.** A deliberate `Thread.Sleep(200)` was injected just before the `accent`
mark, a column that reads 1 ms, and the launch re-run three times: `accent` went to 203 to 216 ms,
`window` went from about 363 to between 577 and 583, and the other nine sub-spans stayed inside the
noise of the run before it. `sum` still landed on `window` on every row. The delay appeared in the
column it was injected into and nowhere else.

**What this does NOT say.** Every launch above is warm, on Debug and on a Release build from `bin`
with `uiAccess` off, on one machine. Section 9's caveats about cold launches and installed builds
apply here unchanged. And nothing here says that 130 ms of first XAML load can be removed at all.
It says only that moving the control that pays it will not remove it.

## 8. Inside the first XAML load

Section 9 below used to carry this as its first open item: section 7 established that the largest
thing in the launch is the process's FIRST XAML control, whichever one that happens to be, and
that deferring it only moves the cost to whatever loads next. It did not say how much of that is
the BAML reader, how much is the merged resource dictionaries, and how much is JIT of the loading
path. It is measured now, and the split rules out the one fix that looked obvious going in.

**The instrument.** Seven throwaway marks in front of `fields`, in a throwaway build, reusing
`StartupWindowTrace` rather than writing a new type, so the positive control it already carries
still holds over the new columns: `sum` has to land on the `window` column, and it did on every
row of every run below. Five of them realize one merged dictionary each, by looking up every key
it holds and every key its nested merges hold. The other two build a control apiece, and they are
read against the `fields` column that was already there:

- **`empty`**, a `UserControl` whose whole content is a bare `Grid`. It references no brush, no
  style and no static, so whatever it costs is the framework's own first-XAML-load path with no
  content of ours in it to blame.
- **`rich`**, a control that uses every XAML FEATURE `WidgetFrame` uses without being it: star and
  fixed `RowDefinition`s, `x:Static`, a named element, `ClipToBounds`, a literal `#AARRGGBB`
  brush, `CornerRadius`, a `DynamicResource` brush, a `RenderTransform` and a `Cursor`.
- **`fields`**, unchanged, which is where `new WidgetFrame()` actually happens.

The dictionaries are resolved by `Source` and not by index, and that is not fussiness.
`ThemeService` has already run by this point: `SwapPalette` can move a palette within the
collection and `ApplyAccentOverride` APPENDS a sixth dictionary that has no `Source` at all, so
indices would have measured the wrong dictionary under each name and said nothing about it. Caught
by reading `ThemeService` before the run rather than by disbelieving a number after it. The
leading slash is the same disambiguation `ThemeService` documents, because "/Palette." must not
match `OsdPalette.Dark.xaml`.

**What it costs.** Five launches per build, in milliseconds. The three columns on the right are
unchanged from section 7 and are carried here as the control on the other three.

| build | window | empty | rich | fields | content | hwnd | weather |
|---|---|---|---|---|---|---|---|
| Debug, from `bin` | 351 | 54 | 80 | 3 | 11 | 133 | 41 |
| Debug, from `bin` | 347 | 54 | 80 | 4 | 11 | 130 | 40 |
| Debug, from `bin` | 345 | 54 | 77 | 4 | 11 | 132 | 41 |
| Debug, from `bin` | 346 | 54 | 79 | 3 | 12 | 130 | 42 |
| Debug, from `bin` | 346 | 55 | 80 | 4 | 11 | 128 | 40 |
| Release, from `bin` | 369 | 56 | 82 | 3 | 13 | 144 | 42 |
| Release, from `bin` | 371 | 58 | 85 | 3 | 12 | 142 | 45 |
| Release, from `bin` | 370 | 56 | 81 | 4 | 13 | 141 | 45 |
| Release, from `bin` | 382 | 61 | 85 | 3 | 12 | 138 | 46 |
| Release, from `bin` | 374 | 57 | 87 | 3 | 12 | 142 | 43 |

Debug and Release sit within the noise of each other for the fourth time in this document. It was
worth re-checking here rather than assuming, and this is the one section where that is true: the
claim below names JIT, which is the thing a Release build is supposed to change.

**Where it goes, and none of it is the control that used to be blamed.**

- **`empty`, 54 to 61 ms.** A control with no content of ours costs this. It is the BAML reader,
  the XAML object writer and the JIT of that path, and nothing in it is reachable from this repo
  except by not loading XAML at all.
- **`rich`, 77 to 87 ms.** The first USE of the feature vocabulary: a `DynamicResource` lookup, an
  `x:Static` resolution, a type converter, a `Freezable` under a `RenderTransform`. Paid once, by
  whichever control uses each feature first.
- **`fields`, 3 to 4 ms.** `new WidgetFrame()` plus four other allocations and both `BandWindow`
  constructors, all of it, now that the two probes in front of it have paid for the machinery.

**That is the answer, and it finishes section 7's sentence.** `fields` read 140 to 147 ms in
section 7, of which `new WidgetFrame()` was 126 to 128. It reads 3 to 4 here. Section 7 proved the
cost follows the POSITION rather than the control; this says what the positional work actually is,
and it splits in two: about 55 ms of framework core that any XAML control whatsoever would pay,
and about 80 ms of first-use that follows the FEATURES rather than the file. `WidgetFrame`'s own
content was never in it, and neither was `OsdContent`'s.

**The merged dictionaries are not the cost, and the obvious fix is dead.** `SettingsTheme.xaml` is
740 lines of styles for a window that may never open in a session, merged into `App.xaml` at every
launch, and going in it was the actionable-looking finding. It is not one. A `ResourceDictionary`
loaded from a `Source` has its STRUCTURE read when the `Source` is set, which happens inside
`App.InitializeComponent` and is already counted in the `app` column, but each of its VALUES is
held as a deferred byte range and realized on first lookup. Nothing looks up a Settings style until
Settings opens. Realizing all five eagerly measures this:

| build | window | osdpal | theme | pal | setth | cardtpl | empty | rich | fields |
|---|---|---|---|---|---|---|---|---|---|
| Debug, from `bin` | 411 | 2 | 19 | 0 | 39 | 1 | 30 | 88 | 4 |
| Debug, from `bin` | 409 | 1 | 16 | 0 | 41 | 0 | 30 | 89 | 4 |
| Debug, from `bin` | 412 | 2 | 16 | 0 | 40 | 1 | 30 | 87 | 4 |
| Debug, from `bin` | 394 | 2 | 14 | 0 | 40 | 1 | 29 | 81 | 3 |
| Debug, from `bin` | 404 | 1 | 14 | 0 | 37 | 0 | 29 | 86 | 4 |

Realizing the five costs 52 to 60 ms and makes the whole launch SLOWER, from a `window` of 345 to
351 up to 394 to 412. **The launch does not pay that money today.** So moving those dictionaries
out of `App.xaml` and loading them when Settings first opens saves nothing at launch, because
there is nothing there to save. That is the second fix these two sections have ruled out, after
section 7 ruled out deferring `WidgetFrame`, and ruling it out is most of what the measurement was
for.

What the dictionary run does show is an overlap rather than a saving: realizing dictionaries first
cut `empty` from 54 to 29, because realizing a deferred value RUNS the BAML reader and warms the
same path the first control otherwise warms. So roughly 24 ms of that 52 to 60 is work a first
control would have done anyway, and the rest is addition. Read that as a shape and not to the
millisecond: `hwnd` also read 146 to 152 in this run against 128 to 133 in the one above it, and
nothing here explains why.

**The positive control.** A deliberate `Thread.Sleep(200)` was injected inside the `empty` span
and the launch re-run three times: `empty` went from 54 to between 262 and 270, `window` went from
345 to 351 up to between 561 and 575, and every other column, `rich` and `fields` and `hwnd` and
`weather` included, stayed inside the noise of the run before it. The delay appeared in the column
it was injected into and nowhere else.

All of it was reverted. Nothing of this instrument ships, exactly as section 7's two splits did
not, and `StartupWindowTrace` is back at its ten spans.

**What this does NOT say.**

1. **`rich` is one column with three plausible tenants.** JIT of the lookup paths, static
   construction of WPF types touched for the first time, and the resource lookups themselves. It
   is named honestly as one column because this instrument cannot separate them, and separating
   them needs a sampling profiler or the runtime's own JIT events rather than another mark.
2. **The probes RELOCATE the cost; they do not prove it is irreducible.** What is ruled out is
   moving it between our own controls or our own dictionaries. Whether the framework's 55 ms core
   can be cut by a different loading strategy altogether, ReadyToRun or startup-path trimming, is
   not something any launch above touched.
3. Every launch here is warm, on Debug and on a Release build from `bin` with `uiAccess` off, on
   one machine. Section 9's caveats apply unchanged, and nothing here says a person minds.

## 9. What has never been measured

- **The Release build, apart from two paths.** Everything above is Debug except sections 5 to 8.
  Section 5 was taken on Debug, on a Release build from `bin`, and on a signed Release install, and
  found the three within noise of each other; sections 6, 7 and 8 on Debug and Release from `bin`,
  likewise within noise, which is now four findings of the same shape. That is still two paths
  measured on Release, not a build, and repeating one of them does not add a third. The Release
  build in `bin` cannot even be launched as it ships: its manifest asks for `uiAccess`, which
  Windows refuses outside a signed install, so every one of those runs used a Release build
  relinked with the Debug manifest. That is the JIT and the optimizer under measurement, not the
  shipping integrity level.
- **A cold launch.** Every launch in sections 6, 7 and 8 is warm: the binary and its dependencies
  were in the file cache from a launch minutes earlier, and `clr` reads 37 to 53 ms because of
  it. The case the report came from is a launch at login on a machine that has just booted, and it
  is not in those tables. It needs a reboot between runs, which is the one thing none of these
  instruments can drive.
- **Which of JIT, type initialization and resource lookup carries section 8's `rich` column.**
  Section 8 splits the first XAML load into about 55 ms of framework core and about 80 ms of
  first-use that follows the features rather than the file, and rules out both fixes that looked
  obvious. It cannot split that second figure further: three mechanisms share one column, and
  telling them apart needs a sampling profiler or the runtime's own JIT events rather than another
  mark. Only one of the three would be this repo's to move.
- ~~**What wakes the UI thread 150 times a second at rest.**~~ **Answered in section 10, and the
  question was built on an arithmetic mistake.** It set about 20 TICKS a second beside 150 CONTEXT
  SWITCHES a second as though they were the same unit. They are not: section 6b had already
  measured that one tick costs several switches, so the known timers never had to add up to 150 to
  explain it. `NotchHoverPoller` alone carries 80 to 85 per cent of the resting wakeups.
- **Long-run memory.** The longest sample here is five minutes. A leak does not show in five
  minutes, and nothing in this repo has ever run the app for a day and looked.
- **GPU and the render thread.** All the sampling above is CPU time per thread. Every stamp in
  section 5 is taken on the UI thread too, so none of it sees the frame the monitor scanned out.
- **Whether moving any of it is worth doing.** Section 6 says where the launch's time goes; it does
  not say that a person minds. Plith starts at login on the machine it was measured on, so the
  block lands while Windows is still bringing the desktop up. The report was "first use", which
  may have been a launch by hand.

## 10. What wakes the UI thread at rest, and it is one timer

Section 9 carried this as the open item: about 150 context switches a second on the UI thread with
nothing happening, and "the timers this repo knows about do not add up to it." Both halves of that
were wrong, and the way they were wrong is the point.

**The answer is `NotchHoverPoller`.** Its 60 ms `DispatcherTimer` carries **80 to 85 per cent of
the resting wakeups**, measured twice, hours apart, on two builds, at two different absolute
scales. Nothing else in the app comes close.

### The arithmetic mistake in the question

Section 9 set 20 ticks a second beside 150 context switches a second and concluded the timers did
not account for it. Those are different units. Section 6b had already measured the conversion on
this very axis, 3,8 to 5,3 switches per tick, and never applied it: at that rate the known timers
predict 76 to 106 of the 150 on their own. There was far less missing than the question claimed.

### What was measured

Debug from `bin` and the signed Release install, app at rest, three clean samples of ten seconds
per row. Each run changes ONE thing.

| run | build | configuration | ui cs/s |
|---|---|---|---|
| A | Debug from `bin` | as shipped, notch parked | 400 to 430 |
| | installed Release 0.3.1 | as shipped, notch parked | 390 to 420 |
| B | Debug from `bin` | `ShowWeather = False` | 401 to 420 |
| C | Debug from `bin` | `Presentation = ClassicOsd` | 70 to 89 |
| D | Debug from `bin` | `ShowNotchWidgets = False` | 394 to 415 |
| E | Debug from `bin` | poller interval 1000 ms | 85 to 87 |
| F | Debug from `bin` | poller 60 ms, `Poll` body stubbed out | 391 to 411 |
| G | Debug from `bin` | as F, timer at Input instead of Background | 389 to 422 |
| H | Debug from `bin` | poller 250 ms, body stubbed out | 147 to 157 |
| | `Plith.DropCatcher`, idle | a whole second WPF process | **0,0** |

**The catcher is the control that makes the rest readable.** A complete second WPF process, idle,
wakes its UI thread ZERO times a second. There is no WPF floor to blame: every wakeup counted
above is work this app asked for.

### It is linear in the tick rate, and that is the proof

The poller's rate was set three ways and the tick rate logged from inside `Poll` rather than
assumed, because "the timer fires far more often than it was told to" was a live hypothesis worth
killing. It was not over-firing: 15,9 ticks/s at 60 ms and 4,0 at 250 ms, as configured.

| ticks/s, logged | ui cs/s | cost per tick across the step |
|---|---|---|
| 1,0 | 85,7 | |
| 4,0 | 150,2 | 21,5 |
| 15,9 | about 404 | 21,3 |

Three points, two independent slopes, 21,5 and 21,3. A straight line with an intercept near 64.
**The poller was costing about 340 wakeups a second and the whole rest of the app about 64.**

### Four hypotheses died on the way, and each one looked obvious

- **WRONG: the weather widget's perpetual animations.** `WeatherMark` and `WeatherWidget` start
  `RepeatBehavior.Forever` animations, and a live WPF animation ticks the media context every
  display frame. This machine's panel runs at **239 Hz**, and 239 plus section 6b's 150 lands
  almost exactly on the 400 being measured. It was a good fit and it was wrong: run B turned
  weather off and moved nothing. `WeatherMark` stops its motion on `IsVisibleChanged`, and the
  arithmetic coincidence was a coincidence.
- **WRONG: the notch's widget pages.** Run D removed them. Nothing moved.
- **WRONG: the win32 calls inside the poll.** `Poll` calls `GetCursorPos` and `GetAsyncKeyState`,
  both syscalls into the window manager, which is exactly where a per-tick cost of 20 would
  naturally come from. Run F replaced the whole body with an immediate `return` and still read 391
  to 411. **The cost is the tick, not the work inside it.**
- **WRONG: the timer's dispatcher priority.** The poller runs at `Background` and section 6b's
  cheap probe ran at `Input`, the one structural difference between them. Run G moved the poller
  to `Input` and changed nothing.

### The absolute number is not a property of this app

The same installed binary read **390 to 420** early in this session and **203 to 208** an hour
later, restored and at rest both times. Under the second state the attribution was re-run and held
exactly: AmbientNotch 203 to 208 against ClassicOsd 32,6 to 32,9, the same 84 per cent share of a
smaller total. **The share is stable, the scale is not.**

The likely hidden variable is the machine-wide timer resolution. `NtQueryTimerResolution` reported
**1,0 ms** during the second state, against a 15,625 ms default. The first state's poller logged
15,9 ticks/s, which is 60 ms rounded up to 4 x 15,625 ms, the signature of the coarse default. Any
process on the machine can raise that resolution, and a browser playing video routinely does.

So this axis carries a caveat every earlier section missed: **a wakeup figure is comparable only
against another taken minutes from it.** Section 6b's rows were interleaved A/B/A/B, which is what
makes them survive this. Its 3,8 to 5,3 per tick and this section's 21,4 are the same quantity in
two machine states, not a contradiction.

### The instrument, and a confound it used to miss

`scripts/measure-idle-cost.ps1` now gates every sample on input idleness via `GetLastInputInfo` and
prints a `clean` column. The reason is specific: Plith carries a system-wide `WH_KEYBOARD_LL` hook
on its UI thread, so **every keystroke anyone types anywhere wakes the thread being measured**. The
first sample taken in this investigation was contaminated that way. A resting figure taken while
someone types is measuring the typing, and nothing in the old script could tell the two apart.
Every number in this section comes from a row that reported `clean`.

### What this does NOT say

1. ~~**It does not say the poll rate should change.**~~ **Closed by section 11**, which made the
   rate follow the cursor and measured both halves of the trade.
2. **It does not explain the floor.** About 64 wakeups a second remain with the poller at one tick
   a second, and the orchestrator at 2, the fullscreen watcher at 1 and the clock at 1 do not
   obviously fill it. A smaller open question of the same shape.
3. **It does not explain why one tick costs about 21 switches**, or about 10 in the other machine
   state. That a bare `DispatcherTimer` tick with an empty handler costs that much is measured
   here and unexplained here. Telling it apart needs the kernel's own scheduling events rather
   than another counter.
4. **It is one machine, at the local console, on one display.** No laptop, no battery and no
   Remote Desktop session, which is what sections 1 to 4 were taken over and is a live candidate
   for why their scale differs again.

## 11. The poll rate now follows the cursor

Section 10 left the decision open: the poller cost about 340 wakeups a second and the question was
whether that bought enough responsiveness. It did not have to buy anything while the cursor is
nowhere near the notch, which is the state a resting machine is in.

**`NotchPollRate` holds the rule.** Poll at 60 ms when the cursor is within 400 DIP of the OSD
window, or when a mouse button is held, or before the notch has been positioned. Poll at 200 ms
otherwise. The fast rate is UNCHANGED at 60 ms, so the sampling of a hover a person actually
performs is identical to what shipped; what changed is when it applies.

**The margin and the slow rate are one decision, not two.** A cursor would have to cross the whole
400 DIP margin inside one 200 ms tick to reach the notch without ever having been seen approaching
it, which is 2000 DIP a second. `NotchPollRateTests` holds the two constants to that relationship
rather than to their values, so changing either one alone fails the suite.

**The button clause is about the shelf, not about hover.** `DragApproachDetector` samples where a
press began on the RISING EDGE of the button, so a press first observed while the cursor is already
over the OSD records the origin as "inside" and refuses the drag. Any button held anywhere
therefore forces the fast rate. It costs nothing at rest for the obvious reason: a resting machine
is not holding a mouse button.

### What it bought, interleaved because section 10 showed the scale drifts

Installed Release 0.3.1 as the before, Debug from `bin` as the after, the cursor parked by the
script at the same point on every run so the two are compared at one cursor position.

| run | build | ui cs/s |
|---|---|---|
| before | flat 60 ms, Release 0.3.1 | 175,9 to 201,1 |
| after | adaptive, Debug | 80,9 to 87,7 |
| after, again | adaptive, Debug | 79,3 to 86,4 |

**About 188 down to about 83: 56 per cent of the resting wakeups, gone.** The residual is the
model's own prediction rather than a surprise: the floor measured at 33 in this machine state,
plus a poller now ticking 5 times a second at the roughly 9 switches a tick this state costs, is
about 78 against the 83 measured.

### What it cost, which is the half worth measuring

A new instrument, because nothing here could see a peek. The first version read the notch window's
height and reported "never peeked" against the SHIPPING build, which is the instrument being wrong
rather than the app: the window is sized to the open panel and keeps that size while parked, so
only what is DRAWN changes. Running the before build first is what caught it. The oracle is now a
band of screen just below the resting strip, sampled until it stops looking like it did at rest.

| approach | before | after |
|---|---|---|
| glide in from far away, the human case | 429 to 469 ms | 404 to 433 ms |
| teleport straight onto the strip | 46 to 92 ms | 29 to 109 ms |

**The glide is unchanged**, which is the whole design: crossing the margin puts the poller back at
60 ms hundreds of DIP before the cursor arrives. The glide figures are dominated by the sweep
itself, which the script spends about 360 ms performing.

**The teleport is the case that widens, and only a machine can do it.** A jump from outside the
margin straight onto the strip waits for the next slow tick, so it is bounded by 200 ms plus the
peek's own growth rather than by 60. Twelve measured jumps landed between 29 and 109 ms, which
does not reach that bound and is not evidence that nothing does. No hand moves a pointer this way;
a KVM, a remote desktop and this script's own `SetCursorPos` all do.

`scripts/measure-notch-open.ps1` was run against the new build as the standing check on the other
path: five opens, 30 ms for the first and 3 to 4 ms after, against section 5's 26 to 36 ms.
Unchanged.

### What this run does NOT say

1. **One glide out of eight never peeked, and it is recorded as unmeasured rather than as a
   defect.** The person using the machine moved the physical mouse during that run, against a
   script driving the same pointer. A contested pointer is not a measurement of anything. It has
   not been re-run on a quiet machine.
2. **It is the same single machine, at the console, in one timer-resolution state.** Section 10's
   whole caveat about scale applies unchanged.
3. **Nothing here measured a battery**, which is the argument wakeups are reduced for in the first
   place. No measurement in this repo ever has.
