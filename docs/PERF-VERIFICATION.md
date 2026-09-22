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
reconsider once one is not.

**That condition is now met, and this is the open work item it creates.** The block is explained:
this section says where the launch's time goes, section 7 says what the largest span is made of,
and section 8 says why the biggest piece of that cannot be moved between our own controls. Nothing
is being hunted any more, so `UiStallWatch` is now four wakeups a second in exchange for a number
nobody is reading. Retiring it, or arming it only around a launch the way section 5's probe is
armed only around an open, is held to section 3's own standard rather than to a guess: measure the
idle wakeups and the idle CPU before and after, and record both here.

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
- **Long-run memory.** The longest sample here is five minutes. A leak does not show in five
  minutes, and nothing in this repo has ever run the app for a day and looked.
- **GPU and the render thread.** All the sampling above is CPU time per thread. Every stamp in
  section 5 is taken on the UI thread too, so none of it sees the frame the monitor scanned out.
- **Whether moving any of it is worth doing.** Section 6 says where the launch's time goes; it does
  not say that a person minds. Plith starts at login on the machine it was measured on, so the
  block lands while Windows is still bringing the desktop up. The report was "first use", which
  may have been a launch by hand.
