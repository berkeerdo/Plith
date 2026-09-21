# Shelf verification

What has actually been looked at on a running build, and what has not.

The shelf lives in a **layered window with per-pixel alpha**, in a second process. That is a
deliberate choice (it is the only way the shape can grow out of the notch rather than cut into
place) and it carries a price that this document exists to keep honest: **a layered window cannot
be captured over Remote Desktop by any means.** No screenshot tool, no `PrintWindow`, no render
harness reaches it. So every visual claim about the shelf is either something a person looked at
at the physical console, or it is not a claim at all.

Session type is read from `qwinsta`, not from `$env:SESSIONNAME`. The environment variable is
stamped at process start and was wrong on this machine once already. Measured again on
2026-09-19, after moving the session to the console with `tscon`: `qwinsta` reported `console`
and `$env:SESSIONNAME` still read `RDP-Tcp#0`. The variable is not merely unreliable, it is
stale by construction.

## The sentence above is true of Remote Desktop and FALSE of the console (2026-09-19)

**At the physical console the shelf CAN be captured, and this changes what this document is
for.** Driven on 2026-09-19: the catcher's layered shelf window was captured, magnified, and read
pixel by pixel, including a forty-frame burst taken through its growth animation.

The trick is one flag. `BitBlt` must be called with `SRCCOPY | CAPTUREBLT`; without `CAPTUREBLT`
layered windows are simply absent from the result, and .NET's `Graphics.CopyFromScreen` cannot
pass it (its `CopyPixelOperation` enum has no member for the combination, and the OR of the two
is rejected as an invalid value). So the convenient API silently returns a screenshot with the
shelf missing, which is indistinguishable from a shelf that never appeared. That is very likely
how "no screenshot tool reaches it" became the premise in the first place.

What this buys, and it is worth stating plainly because it changes the cost of every remaining
item here: a person at the console is still required to PRESS things, because synthetic input
from a Medium process cannot reach Plith's UIAccess window at all (which is this feature's whole
subject). But they are no longer required to JUDGE things. Alignment, clipping, colour, the
growth animation, whether a control is where it should be: all of it can now be measured off a
capture rather than reported from memory.

It already paid for itself twice on the day it was found. A stale installed catcher was caught by
measuring the pixels along one row of a capture and finding the new-stack outline absent
(see 6.9), and the extension tag's overflow out of its glyph was caught in a magnified crop
rather than by a person squinting at a 22 DIP icon.

**What is still out of reach over Remote Desktop is unchanged**, and the paragraph above stands
for that case: the flag does not help there, because the layered surface is never composed into
the remote session's frame buffer to begin with.

## The Remote Desktop half is FALSE TOO, and the real rule is about the SESSION (2026-09-19)

The paragraph directly above is wrong, and it was wrong when it was written. Measured from an
`rdp-tcp#0` session, confirmed by `qwinsta` rather than by `$env:SESSIONNAME`: the catcher's
layered shelf window was captured, magnified and read. Over Remote Desktop. The capture is in
`docs/screenshots/shelf-live-centred.png` and it is what section 7.5 is written from.

Worse than wrong, it was never measured against the shelf at all. The claim is inherited: the
`CAPTUREBLT` sentence appears in `docs/PHASE5-VERIFICATION.md`, `docs/PHASE6-VERIFICATION.md` and
`docs/ROADMAP.md`, all three about the OSD, which is Plith's own UIAccess window in a different
process at a different integrity level. It was carried into this document and applied to a
different window, and then the conclusion it licensed ("or it is not a claim at all") set the
cost of every remaining item here.

**And `CAPTUREBLT` is not what makes it work.** Falsified on purpose, as this document requires
of its own checks: with the `-bor` removed, the shelf was still captured, pixel for pixel as
colourful as before. The flag is kept in `scripts/capture-shelf.ps1` because it is genuinely
required where the surface is not already composited into the desktop DC and it costs nothing,
but on this path it is not load-bearing.

**What the failure actually tracks is whether the session is connected.** The captures succeeded
repeatedly while `qwinsta` showed the session `Active`, and began failing, with `BitBlt` simply
returning false, the moment the RDP client disconnected and the session turned to `Disc`. A
disconnected or locked session has no composed desktop to blit from and this fails for ANY
window, layered or not. That is a property of the session, not of the window.

This is very probably how the original premise was born. "`BitBlt` returned false while I was on
RDP" is one observation away from "layered windows cannot be captured over RDP", and nothing in
the failure distinguishes them. `scripts/capture-shelf.ps1` now prints the session state inside
that exact error message, so the next person gets the distinction handed to them rather than
having to suspect it.

**What is genuinely unchanged**: a person is still required to PRESS things, for the reason given
above. Nothing here touches that.

## The PRESSING half is false too, and for the third time the claim was about a different window (2026-09-19)

The sentence directly above is wrong. It is true of Plith's own OSD — a **UIAccess** window at
High integrity, which synthetic input genuinely cannot reach — and the shelf is not that window.
The shelf belongs to `Plith.DropCatcher` at **Medium** integrity, and UIPI does not stand between
one Medium process and another. Measured from an `rdp-tcp#0` session: UI Automation reads the
shelf's whole tree with names and screen rectangles, `AutomationElement.FromPoint` resolves every
element in it, and `SendInput` drives the window. The instrument is `scripts/drive-shelf.ps1` and
the record is §3.10.

That is three claims in this document, all inherited from the OSD's notes and applied to the
shelf without ever being measured against it: that layered windows cannot be captured, that
Remote Desktop cannot capture them, and now that nothing can press them. **The pattern is the
finding.** Whoever reads the next such sentence here should assume it is about a different window
until they have measured it against this one.

The first thing pressing it found was a tile that answered a pointer only where its icon or label
painted, and was dead in the middle. See §3.10.

---

## 1. The shelf probe (Task 4)

### What it is

`Plith.DropCatcher.exe --shelfprobe x y w h` opens `ShelfWindow` on its own, at the rectangle
given in **physical screen pixels** (the same four numbers the `OpenShelf` wire message carries),
with three invented entries in two stacks and the window's built-in dark palette. No Plith, no
pipe, no single-instance guard: the flag is handled ahead of the guard, beside `--probe`,
`--dragout` and `--dragsource`, for the same reason those are.

The three entries are invented paths that need not exist. `ShelfModel` stats a path only to choose
between the folder icon and the document icon, and a path that is neither is drawn as a document,
which is what these three are meant to be. Two stacks rather than one puts a stack separator in
the picture; three entries split 2 + 1 means the two columns are not the same shape, so a column
that lines up wrongly has something to line up wrongly against.

The probe exits when the shelf reports itself closed, so the process going away is itself the
signal that a dismissal worked.

### The exact command

From a **physical console session**, not over Remote Desktop:

```
cd C:\Projects\plith
dotnet build Plith.slnx -m:1
.\src\Plith.DropCatcher\bin\Debug\net10.0-windows10.0.22000.0\Plith.DropCatcher.exe --shelfprobe 708 0 384 224
```

`708 0 384 224` puts the shelf against the top edge, centred on a 1920-wide monitor at 100 %
scale: `(1920 - 384) / 2 = 768`, less a little, is close enough for a probe, and the numbers are
physical pixels so a 125 % display needs them scaled up (`384 * 1.25 = 480`, `224 * 1.25 = 280`).
**384 x 224 is the shelf page's measured DIP size**, not a guess: `ShelfSurface` wants 210 DIP at
384 wide plus 14 DIP of margin. See `scripts/render-widgets.ps1`, the `shelf-surface` section.

The log to read afterwards is `%LOCALAPPDATA%\Plith\dropcatcher.log`. It stamps UTC.

### What a person at the console must look at

Every one of these is a thing to **look at**, not to infer from the log. The log cannot tell any
of them apart from its own success.

1. **It grows, it does not appear.** The shape starts at the notch's open frame (356 x 116 DIP)
   and grows to 384 x 224 over 220 ms, easing out. The bottom corners round from 8 to 16 DIP as it
   goes. What must NOT be visible: a hard cut from nothing to the full page, or the page arriving
   at full size and then the frame catching up.
2. **The page arrives into the shape, not with it.** Content opacity stays at zero for the first
   55 % of the growth and then ramps in. If the tiles are legible in the first frame, the fade is
   not working.
3. **The page does not reflow while it fades in.** The tiles are pinned at the final size and
   clipped by the growing shape. If the two columns visibly slide apart or the file names re-wrap
   during the last half of the growth, the pinning is not working, and it reads as a window being
   resized rather than a shape opening.
4. **It is readable.** Two columns of tiles, a separator between them, the word "Shelf" and two
   drawn controls (a plus, a cross) along the top. Three file names, none of them torn mid-word.
   The header must not be clipped by the rounded bottom corners, and nothing may sit outside the
   surface.
5. **The colours are the product's.** This probe uses the built-in dark fallback palette, not a
   palette from Plith (there is no sender for one yet). It should look like the dark OSD: a
   near-black gradient panel, near-white ink, a dim separator.
6. **`Esc` closes it.** The window goes away and the probe process exits.
7. **Clicking another application closes it.** Same outcome.
8. **The pointer leaving and coming back does NOT close it.** Move the pointer off the shelf and
   back within about half a second: it must still be there. Move it off and leave it off for a
   second or more: it must go away. The grace period exists because the pointer crosses outside
   the surface on the way to a tile at the edge. Both halves of this have been driven with a
   scripted cursor and behave, so what is left here is the only part a script cannot judge:
   whether 500 ms is the right length for a hand rather than for a `SetCursorPos` call.
9. **It does not appear in Alt+Tab** while it is open, and it does not steal the taskbar.
10. **A second `OpenShelf` on an open shelf moves it, it does not re-grow it.** Still not
    reachable, and Task 6 did not make it so: `OsdHost.OpenShelf` refuses while it is already
    standing aside, so a second click cannot produce a second message. It would take a monitor or
    DPI change re-asserting the rectangle, which nothing does yet. The log distinguishes the two:
    `Shelf opened` against `Shelf re-asserted`.

### Status: PARTLY RUN at the console, 2026-09-19

Driven at the physical console on 2026-09-19, over four runs. Item by item:

| Item | At the console |
|---|---|
| 1, 2, 3 (it grows, the page arrives into it, it does not reflow) | NOT looked at. "It appears" was reported, which is not the same claim |
| 4 (it is readable) | half: the page was legible, the clipping and tearing checks were not made |
| 5 (the colours) | behaved as designed, which here means the dark FALLBACK, not the product's accent. See the two subsections below |
| 6 (`Esc` closes it) | **NOT tested.** Zero `Shelf closing: Esc.` lines on 2026-09-19. `foreground=True` makes it possible, which is not the same as testing it |
| 7 (clicking another application closes it) | PASSES, twice |
| 8 (the pointer leaving) | the leave-and-stay-away half fired twice. The return-within-the-grace half, and the question the item exists for (is 500 ms right for a hand), are unanswered |
| 9 (not in Alt+Tab, no taskbar) | NOT tested |
| 10 (a second `OpenShelf` re-asserts) | still unreachable, unchanged |

The rectangle for this machine is `1088 0 384 224`, not the 1920-wide example above. What was
measured, what the mode cannot show, and the false alarm it caused are two subsections below,
after the scripted table.

The earlier text here said none of the items had been looked at, and named `rdp-tcp#0` as the
reason. That was true when written and is kept in the history rather than in the banner, because
a stale NOT YET RUN reads as a claim about today.

### What HAS been measured, scripted, on 2026-09-19

These were driven from a script and judged from the log and the process state, so they are true
regardless of whether anyone could see the screen. They cover the plumbing, not the picture.

| Measured | Result |
|---|---|
| `Plith.DropCatcher.exe` still runs at Medium integrity with the new window in it | `Integrity: MEDIUM` |
| `OpenAt` applies the rectangle and the window gets a handle | `Shelf opened at 708,0 384x224. handle=0x...` |
| `Esc` dismisses | `Shelf closing: Esc.` then the probe exits |
| Another window taking focus dismisses | `Shelf closing: another window took focus.` then the probe exits |
| The shelf does NOT dismiss itself when nothing happens | still running after 4 s |
| The dismissal event fires exactly once per dismissal | the probe's subscriber logged one exit line, not two, even though `Hide()` raises `Deactivated` a millisecond after an `Esc` had already closed it |
| The pointer leaving dismisses, after the grace period | cursor driven with `SetCursorPos` onto the shelf and off it: `Shelf closing: the pointer left and did not come back.` about 500 ms later, one line |
| The pointer leaving and RETURNING does not dismiss | off for 250 ms and back: still up 1.5 s later, then gone 1.5 s after leaving again |

The pointer rows are the only ones driven with a real cursor rather than a message. They are worth
separating out because they are what exercises `MouseEnter`/`MouseLeave` at all, and because they
happened to answer a second question: moving the physical cursor counts as user input, and on both
of those runs the shelf logged `foreground=True` where an otherwise identical scripted run logged
`False`. That is consistent with the foreground note below, and it is the reason that note does not
claim the real gesture is broken.

That last row found a real defect while it was being measured: `Dismiss` logged a second
"Shelf closing" line for the `Deactivated` that `Hide()` itself causes. `CloseNow` was already
idempotent so nothing downstream doubled, but the log claimed the shelf closed twice for two
different reasons. `Dismiss` now checks the same flag before it writes the line.

### Open hazard, measured and NOT fixed: the foreground lock

`ShelfWindow` depends on activation for two of its three dismissals (`Esc` needs keyboard focus,
`Deactivated` needs to have been activated at all). **`Activate()` does not always succeed.**

Measured, reproducibly: launched while another application held the foreground, the shelf opened
on screen, topmost, and logged `foreground=False`. Launched when no other window held the
foreground, the same binary logged `foreground=True` and both `Esc` and the focus-change
dismissal worked. The foreground window in the failing runs was identified by PID: an unrelated
Notepad. This is Windows' documented foreground lock, not a bug in the window: a process that is
not already the foreground process is not allowed to become one.

In the `foreground=False` state the shelf is **on screen and dismissable only by the mouse-leave
timer**. `Esc` does nothing and `Deactivated` never fires, because the window was never activated.

And the mouse-leave timer is not a floor. It is armed by `MouseLeave`, which WPF raises only after
a `MouseEnter`, so if the pointer never goes over the shelf at all, **nothing dismisses it**: no
keyboard, no focus change, no timer. If you are at the console and the shelf appears stuck, it is
not frozen and it does not need the process killed. **Click it.** The click activates the window,
after which `Esc` works and clicking away closes it as normal; moving the pointer across it and
off also arms the leave timer. Then read the log: an open line with `foreground=False` on it is
this case, and an open line with `foreground=True` is something else and worth reporting.

The fix belongs on Plith's side, and **Task 6 wrote it**: `ShelfSession.Open` calls
`AllowSetForegroundWindow(catcherProcessId)` before it sends `OpenShelf`, which is the documented
way one process hands its foreground privilege to another. What is NOT established is that it
works, because the return value turned out to say nothing. See §2.8, where it returned `True`
from a process that plainly did not hold the foreground. The line to check in the log is still
`foreground=` on every `Shelf opened` entry; it is now paired with a grant line in `plith.log`.

It is also genuinely unknown whether the real gesture hits this at all: the shelf opens in
response to a physical click, and user input relaxes the foreground lock in ways a scripted
`Start-Process` does not. That is a thing to measure at the console, not to assume in either
direction.

### What was measured AT THE CONSOLE, on 2026-09-19

The session was moved to the physical console with `tscon 1 /dest:console`. Confirmed by
`qwinsta` (session 1, `console`, active) and, separately, by the desktop's own display stack:
the adapter attached to the desktop became `\.\DISPLAY1`, an NVIDIA GeForce RTX 5080, one
monitor 2560x1440 at 96 DPI, scale 1.0. The rectangle for THIS machine is therefore
`1088 0 384 224` rather than the 1920-wide example above: `(2560 - 384) / 2 = 1088`, and at
scale 1.0 a DIP is a physical pixel, so the page's 384 x 224 needs no scaling.

A caution worth keeping, because it cost a round: **`tscon` moves the session but does not move
the display.** For several minutes the session read `console` while the desktop was still being
drawn by the `Microsoft Remote Display Adapter` at 1800x1131, which is the old RDP client's
window size, with the physical output not attached to the desktop at all. `qwinsta` saying
`console` is necessary and not sufficient. The check that settles it is `EnumDisplayDevices`:
whether the adapter carrying the desktop is a physical one.

| Measured at the console | Result |
|---|---|
| The rectangle is applied | `Shelf opened at 1088,0 384x224` |
| The window reaches the foreground | `foreground=True`, on all four runs |
| It does not dismiss itself | one run stood 3 m 45 s untouched |
| Another window taking focus dismisses it | twice, `Shelf closing: another window took focus.` |
| The pointer leaving dismisses it after the grace | `Shelf closing: the pointer left and did not come back.` |
| Selection, including multi-selection | a drag carried `2 path(s)`, which only a two-tile selection can produce |
| A tile drags OUT | `Drag out returned Copy for 1 path(s).` |
| A drag does not strand the shelf | `Shelf dismissal deferred (...): a drag or a menu is in flight.` |

**Still not looked at**, and not inferable from any row above: items 1, 2 and 3, the growth
animation itself. What was reported was "it appears", which is a different claim from "it grows
rather than cuts". Item 4 is half answered, the page was legible. Right-click was not tried, so
the context menu is unmeasured here.

### What this mode CANNOT show, written down because it produced a false alarm

A person at the console clicked the header controls, saw nothing happen, and reported the
feature as broken. That reading was correct about what was visible and wrong about the product,
and the fault is this section's: it listed ten things to look at and never said which controls
are inert here.

Three different reasons, worth keeping apart because they are not one finding:

1. **Dead because there is no wire.** Clear, new stack, remove, and restack-by-drag all go
   through `App.WireShelf` onto `_client`, and in the probe `_client` is null, so every send is
   a silent no-op. The page does not update itself either: in the real product Plith answers by
   re-sending the whole shelf, and that reply is what repaints. `WireShelf`'s own header comment
   states this. This document did not.
2. **Dead because the paths are invented.** Open and "Show in file manager" never touch the
   wire, but the probe's three paths do not exist, so they fail for an unrelated reason. A drag
   out likewise returns `Copy` with nothing landing, which reads as a broken drag and is not
   one.
3. **Not the product's colours.** No Plith means no `Palette` message, so the window opens in
   its built-in dark fallback carrying none of the configured accent. Item 5 says this already;
   it is repeated here because it was raised as a defect, which is the evidence that saying it
   once, in a numbered item, was not enough.

What DOES work here, all of it local and repainted locally: selection and multi-selection
(`ShelfWindow.xaml.cs:260-262`), arrow and `Space` navigation (`ShelfSurface.xaml.cs:331-341`),
the context menu opening, dragging a tile out, and every dismissal path.

One procedural finding, from driving it: **this section cannot be run item by item by someone
who reports between items.** Both routes back to a terminal dismiss the shelf, moving the
pointer off it and clicking another window. The items have to be run in one pass and reported
afterwards.

---

## 2. The hand-over (Task 6)

Plith's notch goes down, `Plith.DropCatcher` opens the shelf in its place, and what the person
does there comes back as requests Plith's own `ShelfStore` answers.

The path: a click on the notch's shelf page raises `ShelfWidget.OpenRequested` on **button up**,
`OsdHost.OpenShelf` calls `ShelfSession.Open`, and the session sends `Palette`, then one `Items`
message per stack, then `OpenShelf` carrying the shelf's rectangle in physical pixels. The notch
goes down on the session's `Opened` event, never before it. `ShelfClosed` coming back prunes empty
stacks and brings the notch up again.

### Status: NOT YET RUN

The session that built this is `rdp-tcp#0` (`qwinsta`, 2026-09-19). The shelf is a layered window
in a second process and cannot be captured over Remote Desktop, so nothing below has been looked
at. Every item is phrased so that someone who was not here can execute it.

### 2.1 The catcher must be at MEDIUM

Start Plith and let it start the catcher. Read `%LOCALAPPDATA%\Plith\dropcatcher.log`: the first
line of the run must say `Started. Integrity: MEDIUM`.

If it says `HIGH` the launch route is wrong, and nothing after this works. That is the failure mode
which looks exactly like success: the catcher starts, connects, shows itself, and simply never
receives anything.

`plith.log` should carry a matching `Drop catcher start: <Started|AlreadyRunning|NotFound|Failed>`
on the same startup.

### 2.2 The click opens the shelf

Open the notch, page to the shelf, and click the page body (not the rail at the bottom).

Expected: the notch goes down, and the shelf grows out of the same rectangle it was occupying,
384 x 224 DIP against the top edge. `dropcatcher.log` gets one `Shelf opened at x,y 384x224`.

What must NOT happen: the notch going down and nothing appearing; the shelf appearing somewhere
other than under the notch; or the shelf appearing and the notch still being there underneath it.

This is also the check that the click reaches the page at all. `OsdHost.OnNotchClicked` is wired at
`PreviewMouseLeftButtonDown` and returns early once the frame is open, and `WidgetFrame` marks rail
clicks handled on the rail element, so the page body should get the event untouched. That was read
rather than run. The page also now carries `Background="Transparent"`, without which WPF hit-tests
straight past it and only the file names and icons would be clickable. If a click on the gap
between two tiles does nothing while a click directly on a file name works, that brush is the
thing to look at.

### 2.3 The shelf arrives dressed

The shelf must be in Plith's colours from the first frame it is legible, not repaint after it has
grown. `Palette` and every `Items` message are sent before `OpenShelf` for exactly this.

What would show a failure: the shape growing in the built-in near-black fallback and then
changing colour, or growing empty and filling in afterwards. Worth running once on a non-default
accent and once on the light theme, since the fallback is a dark palette and the failure is
invisible against a dark accent.

### 2.4 `Esc` brings the notch back

With the shelf open, press `Esc`. The shelf goes away and the notch comes back.

Read `dropcatcher.log` for `foreground=` on the `Shelf opened` line first. `foreground=True` means
this test is meaningful. `foreground=False` means the catcher never got activation, in which case
`Esc` cannot work by construction and the thing to report is the `False`, not the `Esc`. See
§2.8.

Then check that the notch does not come back OPEN and stay open: it should either already be at
rest, or collapse on its usual schedule once the pointer leaves.

### 2.5 The drag detector must not pull the notch back under the shelf

With the shelf open, move the pointer away from the top of the screen and back, and drag a window
across the top edge. The notch must stay down the whole time.

`OsdHost` tracks WHY it stood aside (`StandAsideReason.Drag` against `.Shelf`) precisely because
the drag detector keeps polling under an open shelf and will raise a departure. If the notch
reappears over or beside the shelf, that guard is not holding.

### 2.6 No catcher: a sentence, not a dead click

Kill `Plith.DropCatcher.exe`, then click the shelf page.

Expected: the line under the tile row changes from "Click to open the shelf" to a sentence saying
what is wrong, and goes back to the hint after six seconds. The notch must NOT go down.

Four sentences are reachable and they are not interchangeable. "Starting the shelf helper" and
"The shelf helper is starting up" both mean wait a moment; "missing from this install" means
waiting will never help; "Windows would not start the shelf helper" means the launch was refused.
Killing the catcher and clicking immediately should give one of the first two, and clicking again a
few seconds later should open the shelf normally, because the first click restarted it.

To reach the third, rename `Plith.DropCatcher.exe` beside `Plith.exe` and click.

### 2.7 Killing the catcher WHILE the shelf is open

The worst failure this feature has, and the one that is least obvious from the outside.

Open the shelf, then kill `Plith.DropCatcher.exe` from Task Manager while it is on screen.

Expected: the shelf vanishes with the process, the notch comes back within a moment, and
`plith.log` carries `Drop catcher disconnected.` followed by `The catcher went away while the
shelf was open; putting the notch back.` Then press a volume key: the OSD must appear.

That last sentence is the whole test. Plith's window is hidden for the duration of a shelf, and
no `ShelfClosed` can arrive from a process that no longer exists, so the failure is not a missing
shelf but a missing OSD: volume keys showing nothing at all until Plith is restarted. If the notch
does not come back, check that `plith.log` has the disconnect line; if it does not, the channel's
`Disconnected` signal is what broke, and if it does, the `Shelf` stand-aside is not being ended.

Worth running twice, because there are two mechanisms and they cover different instants. Killing
the catcher while the shelf is up is the case above. Killing it and clicking the shelf page in the
same second exercises the other one: `ShelfSession.Open` asks the channel a second time after it
has sent, and should show "The shelf helper stopped responding." rather than taking the notch
down. Both end with the notch up and the OSD working.

### 2.8 The foreground hand-over

`ShelfSession` calls `AllowSetForegroundWindow` with the catcher's process id before it sends
`OpenShelf`, which is the fix §1's open hazard asked for. `plith.log` logs what it passed and what
came back:

```
AllowSetForegroundWindow(pid 16100) returned True.
```

**A `True` here does not mean the grant worked.** Measured on 2026-09-19: a background console
process that plainly did not hold the foreground called it against the real catcher's pid and got
`True` back, on a machine whose `ForegroundLockTimeout` is 150000 ms, so the lock is on. The return
value is necessary and not sufficient.

The half that answers the question is the catcher's own line. Pair them: a `plith.log` grant of
`True` followed by a `dropcatcher.log` `foreground=True` is the working case. A grant of `True`
followed by `foreground=False` means the grant did not take, and `Esc` will not work, which is the
state §1 describes, where the shelf is dismissable only by moving the pointer across it and away.

### 2.9 Shelf actions come back and are answered

Wired in Task 7: see §3 for the console items that exercise this end to end. `ShelfSession.HandleMessage`
routes `RemoveItems`, `ClearShelf`, `NewStack` and `Restack` to `ShelfStore` and re-sends the whole
shelf after each, and the catcher now sends all four, from `ShelfSurface`'s hover affordance,
context menu and tile drag, bridged onto the wire by `App.WireShelf`.

### What HAS been measured, on 2026-09-19

| Measured | Result |
|---|---|
| The catcher still starts at Medium when Explorer launches it, with this build | `Started. Integrity: MEDIUM` |
| `AllowSetForegroundWindow` against the real catcher pid, from a process not holding the foreground | returned `True` |
| `ForegroundLockTimeout` on this machine | `150000`, so the lock is enabled |
| Overlapping sends on one pipe cannot interleave | `DropChannelServerTests.Server_DoesNotInterleaveOverlappingSends`, 20 messages queued unawaited, all 20 lines decode in order |
| The same test fails without the fix | one line came back with 96 fields instead of 5, and DECODED as a plausible `Items` message |
| The shelf page's three states, rendered offscreen at frame size, dark and light | `scripts/render-widgets.ps1`: `widget-shelf`, `widget-shelf-empty`, `widget-shelf-unavailable` |
| The glance page breaks long names the way the catcher's surface does | rendered: `invoice-2026-09.xlsx` is trimmed to `invoice-2...` rather than torn into `invoice-20` over `26-09.xlsx` |
| A send that fails does not mute the channel | `DropChannelServerTests.Server_KeepsSendingAfterASendFailed`: a client connects and goes, a send is made into the corpse, a new client connects and is sent to successfully |
| A catcher that goes away is reported | `DropChannelServerTests.Server_ReportsAClientThatGoesAway` |
| A channel loss with a shelf open puts the notch back | `ShelfSessionTests.ChannelLost_WithAShelfOpen_PutsTheNotchBack`, against a real connected pipe |
| The server survives a catcher restart and sends to the new one | driven in a standalone harness as well as in the suite: connect, disconnect, reconnect, send, read |

One thing measured by accident and worth writing down, because it cost an hour: **this pipe is
created with both buffer sizes set to zero, so a write does not return until the other end reads
it.** A test that awaits a send before starting a read does not fail, it deadlocks, and the runner
reports that as "Test host process crashed" rather than as a hang.

The render harness reaches the notch's shelf PAGE because it is an ordinary WPF control rendered
to a bitmap. It does not reach the shelf SURFACE in place: `shelf-surface.png` is that control
rendered offscreen too, not the window on screen. Nothing in this document's §1 or §2 visual items
is answered by a render.

---

## 3. The actions (Task 7)

Remove, clear, new stack, restack, open and show in the file manager, all raised by
`ShelfSurface` and sent by `App.WireShelf` as `RemoveItems`, `ClearShelf`, `NewStack` and
`Restack`. Open and show in the file manager never touch the wire at all: `ShelfActions` runs
them directly, in this process, at the Medium integrity it already has. See that class's own
header comment for why Plith cannot do either on the catcher's behalf.

### Status: PARTLY RUN, 2026-09-19 — and the first thing running it found was a dead tile

**§3.1's first half, §3.2's selection ring, §3.3, §3.5 and §3.9 have now been driven on a running
build**, along with the context menu that §3.7 and §3.8 depend on. Two instruments and two
records:

| Item | State | Record |
|---|---|---|
| §3.1 hover reveals the remove control, at the tile centre | RUN, passing | §3.10 |
| §3.1 remove takes the row out of `shelf.txt` | **RUN, passing** | §3.12 |
| §3.2 a click at the tile centre selects | RUN, passing | §3.10 |
| §3.2 remove acts on the whole selection | **RUN, passing** (partial, see §3.12) | §3.12 |
| §3.3 clear empties the shelf without asking | **RUN, passing** | §3.11 |
| ~~§3.4 new stack by the plus control, then a drag into it~~ | RUN and passing, then **DELETED with the stacks** | §3.12, §3.4 banner |
| ~~§3.5 dragging a tile onto another stack restacks it~~ | RUN and passing, then **DELETED with the stacks** | §3.11, §3.4 banner |
| ~~§3.6 dragging past the last stack starts a new one~~ | RUN and passing, then **DELETED with the stacks** | §3.12, §3.4 banner |
| every file on a FULL shelf is in the UIA tree | **the check that replaced them**, NOT YET RUN | §3.4 banner |
| §3.7 open, §3.8 show in the file manager | NOT RUN (menu items exist and are named) | §3.10 |
| §3.9 the menu does not let the shelf close under itself | RUN, passing | §3.10 |

§3.10 carries the defect that running the first set found: **a tile only answered a pointer where
its icon or its label happened to paint, and was dead everywhere else, including its exact
centre.** Fixed on this branch, with an automated check that fails the build if it comes back.

§3.11 carries what the second set needed: `scripts/drive-shelf.ps1` drives the catcher alone, and
everything whose expectation is "the tile disappears" or "`shelf.txt` no longer lists it" needs
the REAL pair of processes, which `scripts/drive-shelf-pair.ps1` now drives — notch click, page,
shelf, and `shelf.txt` read back as the oracle.

~~The four items still NOT RUN are blocked by an anti-cheat driver filtering injected input on
this machine rather than by anything in the product; §3.11 has the measurement and what it takes
to clear it.~~ **All four have since been run and pass (§3.12), and the stated blocker was not
the one that mattered.** Vanguard's driver was loaded for every run on 2026-09-20 and refused
`SendInput` on ONE attempt; every other attempt that reached the check accepted a full click
round trip. (The exact denominator is not known: three early attempts recorded that a
precondition had failed without recording which, which is itself one of the defects below.)
What actually stood between these items and a verdict was a fullscreen game holding the pointer,
three defects in the driving script, and a stale Plith instance, none of them in the product,
and none of them the thing this paragraph named.

**And §3.5 and §3.6 could not have passed before 2026-09-19 regardless of who ran them.** The
whole-branch review found the tile drag could never start at all (see §6.1), so the two items that
restack by dragging were describing a gesture the product did not have. §3.5 has since been run
and passes; §3.6 is unchanged and still NOT RUN.

~~Same constraint as §1 and §2: the shelf is a layered window in a second process and cannot be
captured over Remote Desktop, so none of the items below has been looked at.~~ **That sentence was
wrong when it was written, and is struck rather than deleted because it set the cost of this
section for as long as it stood.** The claim is dealt with at the top of this document: it was
inherited from the OSD's notes and applied to a window it had never been measured against. The
shelf captures fine over Remote Desktop while the session is connected.

What has been measured without pressing anything is listed at the end of this section, and it is
not a substitute for any of these: build, tests and lint all stayed green through every defect §1
and §2 found on hardware, stayed green through §3.10's, and would stay green through the rest.

### 3.1 Remove, hover and menu, agree with `shelf.txt`

Drop three files on the notch, open the shelf. Hover a tile: a small remove control appears in
its top-right corner. Click it. Expected: the tile disappears from the page, and
`%LOCALAPPDATA%\Plith\shelf.txt`, opened in Notepad, no longer lists that path. The file staying
plain-text readable is a feature this slice deliberately kept, and this is the one item that
actually looks at it rather than trusting that it still is.

Right-click a second tile instead of hovering it, choose Remove from the context menu. Same two
expectations: gone from the page, gone from `shelf.txt`.

### 3.2 Remove acts on the selection, not just the one tile

Ctrl+click two tiles in the same stack so both carry the selection ring, then click the remove
control on ONE of them (or use its context menu). Expected: both selected tiles are removed, not
only the one the control was on. This is `ShelfModel.DragPaths`'s own rule showing up on screen;
`ShelfModelTests` covers the rule itself, but nothing before this item has looked at it wired to
a real control.

Then hover and remove a tile that is NOT part of any selection. Expected: only that one tile
goes.

### 3.3 Clear empties the shelf without asking

Click the header's clear control (the X). Expected: every tile is gone immediately, no
confirmation dialog, and `shelf.txt` is left with nothing in it (or absent, depending on how
`ShelfStore` writes an empty shelf). If a confirmation appears, that is a regression: the header
comment beside `ClearRequested`'s handling explains why this action was built to ask nothing
first.

### 3.4, 3.5 and 3.6 are GONE with the stacks they described (2026-09-20)

These three items asked about making a stack, restacking a tile into another stack, and
dragging past the last stack to start a new one. The shelf is one flat list now, so none of
those gestures exists: `NewStack`, `Restack`, `PruneEmptyStacks`, the column drop target and the
`+N` chip are all deleted, and `DropVerb` lost two of its members with them.

**3.4 and 3.6 were RUN and passing hours before they were deleted**, which is not waste. What
they measured is recorded in 3.12, where it belongs: a record of what the code did on the day.
Four of the five instrument defects that run uncovered had nothing to do with stacks, so those
fixes outlive the items entirely.

What replaced them is one check that the stack build could never have passed, in
`scripts/drive-shelf-pair.ps1`: seed the shelf to exactly its capacity and require every file to
be present in the UIA tree. The stack build capped at 20 against a surface that drew 10, and a
folded tile is in no UIA tree at all, so half a full shelf was unreachable by key and invisible
to a screen reader. The cap is now defined as what the grid draws, and that check is what would
notice if a fold ever came back.

### 3.7 Open, from the right process

Right-click a tile and choose Open. Expected: the file opens in whatever handles it, at Medium
integrity. There is no way to read integrity off a running window directly; Process Explorer's
"Integrity Level" column against the opened application's process is the way to check it, and it
should read Medium, not High. A High-integrity document window (dragging fails onto it, or it
otherwise behaves oddly for reasons that are not visible on screen) is exactly the failure
`ShelfActions.Open`'s header comment describes, and would mean this call happened from Plith
instead of the catcher.

### 3.8 Show in the file manager opens the right window class

Right-click a tile and choose "Show in file manager". Expected: a file manager window opens with
the file selected. On a machine where Files is the default file manager (this one, per
`ShelfActions`' own header comment), that window's class is `WinUIDesktopWin32WindowClass`, not
Explorer's `CabinetWClass`, checkable with a tool like WinSpy or the Accessibility Insights
window property view. This item exists because that fact was measured once already for
`ShelfActions` to be written correctly; it is listed here so a machine with a different default
file manager does not get read as a regression when the window class simply differs for an
unrelated reason.

### 3.9 The context menu does not let the shelf close under itself

Right-click a tile to open its context menu, and while the menu is up, let the mouse-leave grace
period that would normally dismiss the shelf run out (wait past it, or move the pointer off the
shelf first). Expected: the shelf stays up with the menu open. Close the menu (Esc or a click
elsewhere). Expected: the shelf now behaves normally again, in particular still dismissable by
`Esc` or by the mouse leaving. A shelf that survives the menu but can never be dismissed
afterwards means `_menuOpen` was set and never cleared; a shelf that vanishes out from under an
open menu means the opposite.

### 3.10 Driven on a running build, and the tile that was dead in the middle (2026-09-19)

**The premise that a person was required to PRESS things is false for this window, and it is the
third inherited claim in this document to fall the same way.** It is true of Plith's own OSD,
which is a UIAccess window at High integrity that synthetic input genuinely cannot reach. The
shelf is not that window. It belongs to `Plith.DropCatcher`, which runs at **Medium**, and UIPI
does not stand between one Medium process and another. Measured from an `rdp-tcp#0` session:
UI Automation reads the shelf's entire tree with accessible names and screen rectangles
(17 elements), `AutomationElement.FromPoint` resolves every one of them, and `SendInput` drives
the window. The catcher's own integrity is in its log on every run (`Integrity: MEDIUM`), which
is where this could have been noticed at any point.

The instrument is committed as `scripts/drive-shelf.ps1`, for the reason `capture-shelf.ps1` is:
it is the only thing in this repository that presses what a person presses.

#### The defect: a tile answered a pointer only where it painted

**Measured by controlled comparison, the same click on the same tile at two points:**

| Point | Hover | Click |
|---|---|---|
| The tile's exact centre | 0 px changed, no remove control in the UIA tree | 0 px changed, no selection |
| The tile's icon, 14 px away | 48 px changed, remove button present and named | 593 px changed, the selection ring |

The cause is one missing line. `BuildTile`'s `NamedBorder` had **no `Background` at all**, and WPF
hit-tests a `Transparent` brush but not a `null` one, so only the painted icon and label answered
the pointer. The gaps between them — which on a 64 DIP tile holding a 22 DIP icon and a 24 DIP
label includes the middle, where a person aims — fell through to the layout `Grid` behind.

The product said so itself once it was asked. With a temporary log line on the tile's own
`MouseEnter`/`MouseLeave`, a pointer travelling **inside** one tile produced
`MouseEnter quarterly-report.pdf` over the icon and `MouseLeave quarterly-report.pdf` thirty-two
milliseconds later, still inside the tile, at its centre.

**`ShelfWindow.xaml` states this exact rule in its own comment** — "1% alpha on the background
because WPF hit-tests a Transparent brush but not a null one" — for the window's background. The
tile did not follow it. Fixed by giving the tile `Background = Brushes.Transparent`, with the
measurement in the comment beside it.

**What it cost, and it is more than a hover.** `tile.PreviewMouseLeftButtonDown` is what calls
`BeginPress`, so a press in the dead area never selected, never armed a press, and therefore
could never become a drag. That is §3.2, and it is the press that §3.5, §3.6 and the whole of
Task 8's drag-out design start from. This is the same failure family as §6.1, arriving by a
different route: the gesture existed and the pointer could not reach it.

#### Why nothing here could have caught it, and what now does

Every other check in `render-widgets.ps1` hands an event straight to the element it means — the
press-to-drag check calls `$pressedTile.RaiseEvent($down)`, which is a press the tile receives
**by construction**. Real input arrives at a POINT and the window decides whose it is. Those are
different questions, and the second had never been asked.

`render-widgets.ps1` now asks it: the **tile-hit check** hit-tests five points inside every tile
(the centre and a quarter of the way in from each corner) and requires each to resolve to that
tile. Verified red-green against freshly built binaries, not against a stale one: with the
`Background` line removed it fails on ten points across three tiles, every centre among them;
with it restored it passes and the script reaches `Done.`

#### What was driven, and what passed

`scripts/drive-shelf.ps1`, eight checks, all passing after the fix:

| | |
|---|---|
| §3.1 hover reveals the remove control, **at the tile centre** | 48 px changed; the UIA button is present at 16x16, named `Remove quarterly-report.pdf from the shelf` |
| §3.2 a click at the tile **centre** selects it | 593 px changed, the selection ring |
| The tile context menu | opens anchored to the pressed tile, carrying exactly `Open`, `Show in file manager`, `Remove` |
| §3.9 the shelf survives its own open menu | pointer held off the shelf for 2.5 s against a 500 ms grace, shelf still up, and the product logs `Shelf dismissal deferred ... a menu is in flight` |
| §3.9 `Esc` closes the menu | no menu items left in the UIA tree |
| §3.9 the shelf outlives its menu | still up after the menu closed |
| §3.9 the shelf holds still afterwards | no dismissal in 1.5 s with the pointer on the shelf, so the next step's `Esc` is credited with a dismissal it actually caused |
| §3.9 `Esc` still dismisses afterwards | it did, within 5 s — but see the log finding below |

So §3.9 is **RUN and passing**, including the part the render harness's menu-survives-render
check explicitly could not reach: that the popup appears anchored to the right tile, and that the
grace period and `Esc` behave around it.

#### A finding NOT fixed: the log credits the wrong reason for a dismissal

`ShelfWindow.Dismiss` ends `_log.Info($"Shelf closing: {_pendingDismissal ?? why}.")`. The
first-reason-wins rule is right for the retries a deferral causes, but it also overrides a
genuinely new one. Measured, with a control: an ordinary `Esc` logs `Shelf closing: Esc.`, while
an `Esc` pressed after a dismissal was deferred logs `Shelf closing: the pointer left and did not
come back.` — even with the pointer back on the shelf and the menu closed, so the recorded reason
has already been refuted.

Behaviour is correct; only the record is wrong. It is filed rather than fixed because this
document's method depends on that log being honest (§1 tells a reader to distinguish states by
it), and because the surrounding dismissal logic records three previous defects in this exact
area, so it is not a line to change while finishing something else. The underlying cause is that
`_pendingDismissal` is cleared only by `Activated`, never by the pointer coming back and
refuting it.

#### What this instrument CANNOT answer, and it is most of the rest of this section

In `--shelfprobe` mode `App`'s `CatcherClient` is null, so `RemoveItems`, `ClearShelf`,
`NewStack` and `Restack` are raised and go nowhere. **Plith owns the shelf model** and answers
those verbs with a fresh set of `Items` messages, so with no Plith the page never changes in
response to them. Every item expecting a tile to disappear or `shelf.txt` to change — §3.1's
second half, §3.3, §3.4, §3.5, §3.6 — needs the real pair of processes, not this probe.

§3.7 and §3.8 are blocked for a smaller reason: the probe's three paths are invented
(`C:\Probe\...`), so `Open` and `Show in file manager` have nothing real to act on. Their menu
items exist and are correctly named, which is as far as this goes.

#### Harness lessons, which cost more than the defect did

Three instrument faults each produced a confident, plausible, wrong number first, and two of them
read as product defects. They are written into `drive-shelf.ps1`'s header so they are not
rediscovered:

1. **PowerShell assigns to a COPY** when the target is a field of a nested value type, so every
   `$input.u.mi.dwFlags = ...` was discarded and `SendInput` received an all-zero structure while
   reporting success. Nothing was being sent at all, and the run reported a 128-pixel "hover".
   All input construction now lives in C#, and the script proves the pointer moves before it
   measures anything.
2. **A pointer that TELEPORTS is not a pointer that arrives.** A single absolute move onto a
   window leaves WPF's `Mouse.DirectlyOver` stale and nothing under it sees a `MouseEnter`.
3. **`LeaveGrace` is 500 ms.** A "rest" baseline taken with the pointer parked off the shelf, then
   compared against a "hover" taken a second later, is two captures of the same desktop — the
   shelf had dismissed itself in between. It reports **exactly zero** changed pixels, which reads
   precisely like a product ignoring the mouse. Baselines are now taken over a neutral part of
   the shelf.

The pattern worth keeping: every one of those was caught by asking the product rather than the
picture — the catcher's own log, a temporary handler, the UIA tree, the cursor shape. A pixel
count says something changed; it never says the right thing changed, and it says nothing at all
about why it did not.

#### The gates, after the fix

| | |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors |
| Tests | `dotnet test Plith.slnx -m:1`: 473 + 17 passed, 0 failed |
| `check-a11y` · `check-shared-xaml` · `check-win32-flags` | passed, exit 0 |
| `check-contrast` | passed, 288 measurements across 9 accents and both themes |
| `render-widgets` | reached `Done.`, with second-pass, menu-survives-render, press-to-drag, drop-target and the new **tile-hit** check all passing |
| `drive-shelf` | 8 of 8 checks passing on a running build |

Worth saying plainly, because it is this branch's recurring lesson: **every one of those gates was
green before the fix as well.** The tile had been dead in the middle since Task 7, through a
whole-branch review, and nothing in the repository was capable of noticing.

### 3.11 Driven against the REAL pair, and the fourth inherited premise falls (2026-09-19)

§3.10 left most of this section unreachable for a stated reason: the shelf probe has no Plith
behind it, so `RemoveItems`, `ClearShelf`, `NewStack` and `Restack` are raised and go nowhere.
That reason was correct. What was never examined is whether the real pair could be driven at all,
and the answer is that it can, by a script, end to end: **the notch can be clicked open, paged to
the shelf widget and clicked, and the shelf opens on the wire with Plith answering it.**

The instrument is `scripts/drive-shelf-pair.ps1`. It is a second script rather than a mode of
`drive-shelf.ps1` because almost none of it is the same work: it starts both processes, reaches
the shelf through the notch, and judges every result by reading `shelf.txt` rather than by
counting pixels.

#### Why a Debug build changes what is reachable, and why nobody had noticed

**Plith's own OSD is not a UIAccess window in the build this repository verifies against.**
`src/Plith/app.manifest` sets `uiAccess="false"` and `Plith.csproj` swaps in
`app.release.manifest` only for Release. So a Debug Plith runs `asInvoker`, at **MEDIUM**
integrity, measured from its token: `MEDIUM (0x2000)`, the same level as the catcher and the same
level as the driving script. Plith says so itself on every Debug start:

> `UIAccess NOT granted — the band window falls back to a plain topmost window ... Expected for a
> Debug build; a signed install in Program Files should have it`

UI Automation reads the OSD's whole tree, `SendInput` and `SetCursorPos` drive it, and the click
that opens the widget frame is an ordinary click. The premise that a person is required to press
Plith's OSD is **true of the signed install and false of every build anyone verifies against** —
which is the fourth claim in these documents to be inherited from one window and applied,
unmeasured, to another. §3.10 said the pattern is the finding; this is the pattern again, and it
is the one with the widest reach: it gates most of the open items in
`docs/PHASE5-VERIFICATION.md` and `docs/PHASE6-VERIFICATION.md`, and those were never about the
shelf at all.

`WheelDecoder`'s own header carried the same claim in source ("no agent may drive input anyway")
and has been corrected in the same commit.

#### What was driven, and what passed

| | |
|---|---|
| §2.2 a click on the shelf page opens the shelf | `Shelf requested at 1088,0 384x224 with 2 stack(s).` then `Shelf opened at 1088,0 384x224 ... foreground=True` |
| The shelf arrives holding what Plith loaded | `[alpha.txt bravo.txt charlie.txt] [delta.txt echo.txt]`, both stacks, in order |
| **§3.5** dragging a tile onto another stack restacks it | `[alpha bravo charlie] [delta echo]` → `[echo alpha bravo charlie] [delta]` in `shelf.txt`, and the catcher logged `Drag out returned Copy, Link for 1 path(s).` |
| **§3.3** clear empties the shelf without asking | `[echo alpha bravo charlie] [delta]` → `(empty)`, with no second catcher window on screen |

**§3.5 and §3.3 are RUN and passing.** Both are round trips through two processes: the surface
raises the verb, Plith's `ShelfStore` decides what is true, writes the file, and sends the page
back. Neither could have been answered by the probe, and neither can be faked by a screenshot —
the evidence is the file, on disk, before and after.

#### The foreground question, answered by the real gesture

§1's open hazard ends: *"It is also genuinely unknown whether the real gesture hits this at all:
the shelf opens in response to a physical click, and user input relaxes the foreground lock in
ways a scripted `Start-Process` does not. That is a thing to measure, not to assume in either
direction."*

Measured, on every run that reached the shelf:

```
[Shelf] AllowSetForegroundWindow(pid 63068) returned True.
[Shelf] Shelf requested at 1088,0 384x224 with 2 stack(s).
Shelf opened at 1088,0 384x224. handle=0x9E0135E, foreground=True
```

So the click path reaches the foreground. That does not vindicate `AllowSetForegroundWindow`'s
return value, which §2.8 already showed says nothing — the grant and the click arrive together
and this cannot separate them. What it does settle is the direction that mattered: the gesture a
person actually makes does **not** land in the `foreground=False` state, so `Esc` and the
focus-change dismissal are live on the real path.

#### Three preconditions, each of which cost a run

1. **The session must be Active.** Unchanged from `capture-shelf.ps1`: a disconnected or locked
   session has no desktop and `BitBlt` fails for any window in it.

2. **Nothing may cover the monitor.** `OsdHost.OnForegroundCoversMonitorChanged` hides the notch
   outright while the foreground window covers the screen. That is the designed fallback, not a
   fault, and **a maximised terminal triggers it** — so a driver cannot simply run from one, and
   the first three attempts found no notch at all and said so in a way that read like a broken
   build. The script now opens a 320x200 window of its own and lets that hold the foreground. It
   deliberately does not minimise anything belonging to the person; a fullscreen game that keeps
   taking the foreground back is reported as a precondition failure instead.

3. **Injected input must actually reach the desktop.** See below.

#### Instrument defect 4: an anti-cheat driver filters injected input, PARTIALLY

`SendInput` began returning 0 with `ERROR_INVALID_PARAMETER` (87) mid-run, having worked minutes
earlier. The cause is not Plith and not the script: **Riot Vanguard** (service `vgc`) was loaded,
and it filters injected input process-wide. Bisected one flag at a time:

| Event | Result |
|---|---|
| `MOUSEEVENTF_MOVE`, relative / absolute / absolute+virtualdesk | refused, 87, on every attempt across the whole afternoon |
| `MOUSEEVENTF_LEFTDOWN` | **intermittent**: refused at 19:57, accepted at 20:02 and 20:04, refused again from 20:06, with a protected game running throughout |
| `LEFTUP`, `RIGHTDOWN`, `RIGHTUP`, `WHEEL`, every keyboard event | accepted, from the same process, in the same second as a refused `LEFTDOWN` |
| `SetCursorPos` | works throughout; it is not injection |

**The obvious rule is not the measured one, and the difference is worth keeping.** "It blocks
while a game is running" would be a tidy story and it is not what happened: the same binary, from
the same session, with VALORANT running the entire time, had `LEFTDOWN` refused, then accepted
twice, then refused again. Whatever Vanguard keys on, it is not simply the game's presence, and
this section does not have the measurement to say what it is. What it can say is that the refusal
is loud, unmistakable and never silent, which is all an instrument needs.

**The partial is the trap.** An instrument that checks only "did `SendInput` succeed" on one
event would send half a click — `LEFTDOWN` refused, `LEFTUP` accepted — and then measure the
result, with the product blameless and the numbers meaningless. Movement therefore goes through
`SetCursorPos`, stepped rather than jumped because defect 2 is about the pointer ARRIVING and not
about which call moved it; and the script refuses to measure anything until a full button round
trip has been accepted.

This also bounds what can be finished here: **§3.1's second half, §3.2, §3.4 and §3.6 are still
NOT RUN**, not for any reason in the product, but because the block reappeared before the run
reached them. The run that does finish them needs a spell where `LEFTDOWN` is accepted, and the
cheapest way to get one is a machine with Vanguard's driver unloaded, which means a reboot with
no Riot game started rather than closing the game. Re-running costs about a minute: the script
seeds its own fixture, restores the shelf it replaced, and stops everything it started.

#### Two things the run found that are not defects, and one that is

**The two-tile rule.** A stack column folds what it cannot show into one chip named
`N more items in this stack`. A folded item is in no UIA tree, and asking for it by name reports
"not found", which reads exactly like a page that has lost its accessible names. The first
fixture put three files in a stack and produced four such false alarms in one run. The fixture is
now four files in two stacks of two, and the steps are ordered so that every tile a later step
needs is still drawn.

> **The sentence above used to read "draws at most two tiles", and that is wrong. It was corrected
> 2026-09-20 after it cost a second run.** The chip costs a SLOT. `ShelfSurface.VisibleRowsShown`
> is `stackCount > 2 ? 1 : stackCount`, so a stack of two draws both tiles and a stack of **three
> draws exactly one**, not two, plus a `+2` chip. "At most two" is true only in the vacuous sense;
> the row that actually disappears is the second one, as soon as a stack passes two.
>
> This mattered because the rule as written was used to design the fixture. `drive-shelf-pair.ps1`
> carried the comment *"Stack 1 is now [charlie alpha bravo]; alpha is its second tile and still
> drawn"* directly above a three-item stack, and §3.6 and §3.4 then reported `alpha.txt not in the
> UIA tree` and `bravo.txt found: False`, both reading as product defects and both being the
> fixture. The safe targets are any tile in a stack of one or two, and the FIRST tile of any stack
> whatever its size.
>
> It is the same failure this document keeps recording, one level up: a claim written from
> reasoning rather than from the arithmetic, then trusted by the thing built on top of it.

**Half of §3.4 is already answered, and the half that is answered is the one a reader would
doubt.** The plus control does produce an empty column, and that column does announce itself:
a run that got that far reported `empty stack announced: True`, with an element matching
`Stack N, 0 items` in the tree. That matters because an empty stack writes **nothing** to
`shelf.txt` — `ShelfStore.Save` skips empty stacks — so the page is the only witness the click
has, and the driver's §3.4 step is built on that name. What is still unrun is the second half:
a drag landing in the new column. A `FAIL` on §3.4 from a future run is therefore about the
drag, not about whether the name exists.

**The catcher owns more than one layered window.** Taking "the first layered catcher window"
found the wrong one the moment a drag had run, and every check after it reported "not in the UIA
tree". The shelf is now identified by the one whose tree announces itself as `Shelf, N stacks`.

**And one that is: `ShelfWidget`'s row announcement reaches nothing, confirmed on a running
build.** `AutomationProperties.SetName(Tiles, "Shelf, N items")` sets the name on a `StackPanel`,
which WPF gives no automation peer, so a screen reader never hears it. This was already known —
`check-a11y.ps1` carries it as a suppressed entry, filed under §5.4 as predating Task 9 — but it
had been established by reading the code. The live tree now confirms it: with five files on the
shelf page, the names present are the five file names, their type chips and `Click to open the
shelf`, and `Shelf, 5 items` appears nowhere. Left filed rather than fixed, for the reason the
lint's own comment gives; recorded here because a confirmed absence and an inferred one are not
the same evidence.

#### A Phase 6 constant, characterised by real input for the first time

`docs/PHASE6-VERIFICATION.md` §16 names the three provisional paging constants as the
highest-risk items on that branch, correctable only from the log line each commit writes. That
line has now been produced by real input:

```
[OsdHost] Widget page committed: delta=120, index=1/4
[OsdHost] Widget page committed: delta=120, index=2/4
[OsdHost] Widget page committed: delta=120, index=3/4
```

One wheel notch is one page, at 700 ms spacing, with no accumulation carried across. That is
`CommitThreshold = 120` and `IdleRearmMs = 150` behaving exactly as designed **for a wheel**. It
says nothing about a touchpad's two-finger swipe, which delivers many small deltas rather than
one of 120 and is the case those constants were actually chosen for.

### 3.12 The last four items, and five defects that were all in the instrument (2026-09-20)

**§3.1's second half, §3.2, §3.4 and §3.6 are RUN and passing.** Every item in section 3 that a
script can reach has now been driven against the real pair, judged by reading `shelf.txt` before
and after rather than by looking at pixels.

| Item | Measured |
|---|---|
| **§3.1** remove takes the tile off the page AND out of `shelf.txt` | `[bravo][alpha][delta][charlie]` → `[bravo][alpha][delta]`, and the name is gone from the UIA tree (`still drawn: False`) |
| **§3.2** remove acts on the whole selection | `[bravo][alpha][delta]` → `[bravo]`: two tiles selected, ONE remove control clicked, both left |
| **§3.4** the plus control makes a stack, and a drag lands in it | empty column announced as `Stack 1, 0 items`, then `[alpha bravo][delta][charlie]` → `[bravo][alpha][delta][charlie]` |
| **§3.6** dragging past the last stack starts a new one | `[charlie alpha bravo][delta]` → `[alpha bravo][delta][charlie]` |

**Not one of the obstacles was in the product.** That is the finding, and it is the same one
§3.10 and §3.11 recorded, so it is now a property of this work rather than an anecdote.

1. **A fullscreen game owned the pointer, and the check blamed the wrong thing.** The opening
   check slept 200 ms, sampled the cursor once, and reported "the pointer did not move", which
   was false. `SetCursorPos` landed on its target exactly, six times out of six, read back
   immediately. Something moved it away again afterwards: `WardogsClient-Win64-Shipping`, whose
   mouse capture re-centred the cursor, which is why the pointer kept coming to rest on exactly
   `1280,720`, the precise centre of a 2560x1440 screen. A round-numbered resting position is
   the tell. The game also covered the monitor, so it failed precondition 3 at the same time,
   and that case has the opposite remedy (close it, rather than let go of the mouse). The check
   now answers "did the call take effect" and "is anything else driving the pointer" separately,
   and names the foreground window. Filed as defect 5 in the script's own header.

2. **A hand arriving mid-run broke nothing and ruined everything.** `Move-Pointer` glided to a
   tile and the caller pressed, with nothing verifying that the pointer had arrived. A pointer
   nudged aside between the two produced a press on whatever happened to be underneath, reported
   as a verdict about the shelf, silently, with every call returning success. Defect 4 at least
   shouts. This one could not, until `Move-Pointer` was made to check arrival before each press.

3. **The overflow rule was written down wrong, and the fixture was built on it.** See the banner
   in §3.11: the chip costs a SLOT, so a stack of three draws ONE tile, not two. §3.6 and §3.4
   were asking the UIA tree for tiles the surface had folded away, and both reported the product
   broken. Corrected in the document and in the script, which now drags the FIRST tile, drawn at
   any stack size, and thereby un-overflows the column for the steps that follow.

4. **An assertion that could not return True on any input.** `@($after | Select-Object -Last 1)`
   does not reach into the last stack: `Read-Shelf` returns an array OF ARRAYS, and
   `Select-Object` emits the inner array as one object without enumerating it, so the result is a
   one-element array whose element is an `Object[]`, and `-contains 'charlie.txt'` compares a
   string against an array. §3.6 failed while printing its own correct end state next to the word
   FAIL. Reproduced in isolation before being fixed to `$after[-1]`. The `Where-Object` forms in
   §3.1, §3.2, §3.4 and §3.5 were never affected: there `$_` is bound to each inner array.

5. **A stale Plith made the catcher look guilty.** `Program.cs` takes a per-user single-instance
   mutex, and the pre-run kill silenced its own errors. An instance from three hours earlier
   survived, so the Plith the script started exited at once and logged nothing. The script,
   waiting on a log line from a process that was already gone, reported
   `The drop catcher never connected within 30 s`, sending the reader to the pipe, the catcher
   and the ACL, none of which was at fault. The kill is now verified rather than assumed, and the
   wait asks `$proc.HasExited` instead of only watching the log.

**What the report did with all of this, before it was fixed: threw it away.** The verdict block,
both process logs and the shot paths sat AFTER the `try/finally`, so any exception propagated
past them and printed none. A run that got most of the way through reported one line about an
`IntPtr` cast and was indistinguishable from a run that never started. The report now prints on
every path and the exception surfaces after it. Three attempts earlier in the night recorded only
"died on a precondition" without recording which, and are the reason the Vanguard denominator in
this section's opening is unknown. It is the same mistake one level out, in the wrapper rather than
the script.

#### Two honest gaps in §3.2, which is marked passing

The written item asks for `Ctrl`+click on two tiles **in the same stack**. The driver selected
two tiles in DIFFERENT stacks. That is arguably a stronger test of "remove acts on the selection"
(the rule is `ShelfModel.DragPaths`'s and has nothing to do with columns), but it is not the
item as written. The item's second half, *hover and remove a tile that is NOT part of any
selection, expecting only that one to go*, is **not driven at all**. §3.2 is marked passing for
the half that ran.

#### What this section is likely to outlive

On 2026-09-20 the decision was taken to replace stacks with a single flat shelf, on the grounds
that Alcove and Dropover have no such concept and the surface is simpler without it. If that
lands, **§3.4, §3.5 and §3.6 describe behaviour that no longer exists** and should be deleted
rather than migrated, and the `+N` chip goes with them, which also retires defect 3 above, and
the accessibility gap where a folded tile is in no UIA tree and reachable by no key. §3.1, §3.2
and §3.3 survive unchanged. The measurements above are kept because they are what the code did on
the day, not because the code will keep doing it.

### What HAS been measured, on 2026-09-19

| Measured | Result |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors |
| Tests | `dotnet test Plith.slnx -m:1`: 472 + 17 passed, 0 failed |
| `scripts/check-a11y.ps1` | passed: every interactive control has an accessible name, no icon-font use |
| `scripts/check-shared-xaml.ps1` | passed |
| `scripts/check-contrast.ps1` | passed: 234 measurements across 9 accents and both themes |
| The shelf surface, offscreen, dark theme, lime accent | `shelf-surface.png`: header controls and tile layout unchanged from Task 6's render; the hover remove control is correctly invisible at rest, since nothing in an offscreen render hovers a tile |
| Same, light theme, lime accent | unchanged layout, ink and surface both legible |
| Same, dark theme, white accent | unchanged layout, selection ring legible against the white accent |
| The cache-hit second pass (Task 5's own check) still passes after Task 7's tile restructuring | `shelf-surface-pass2.png`: "second-pass check passed" on all three renders above |
| A context menu opened on a tile is force-closed, not orphaned, before a later `Render` tears that tile down | `render-widgets.ps1`'s new "menu-survives-render check": a real `ContextMenu.IsOpen` set true, `MenuOpenChanged` reports `true`, `Render` runs again, `IsOpen` reads `false` and `MenuOpenChanged` reports `false` after one dispatcher pump. This is the review-found Critical from the first round, reproduced and then shown fixed rather than reasoned about. |

The renders confirm something narrower than the console items above, and it is worth being exact
about the difference: wrapping each tile's content in a Grid (so the hover remove button has
somewhere to sit without disturbing the icon and label) did not move anything on screen, and did
not break the render harness's own structural assumption about where the icon host lives inside
a tile (`scripts/render-widgets.ps1` needed one extra `.Children[0]` to reach it, now updated).
Nothing about a render can show a hover or a drag, so §3.1, §3.2 and §3.4-§3.7 are unanswered by
it. §3.9 is a partial exception: the new menu-survives-render check drives a real `ContextMenu`
through exactly the sequence that broke the first version of `_menuOpen` (open it, force a
`Render` while it is open, confirm it closes and `MenuOpenChanged` reports it), so the MECHANISM
behind §3.9 is measured, off-screen, not reasoned about. What that check cannot reach is
everything §3.9 actually asks a person to look at: whether the popup visually appears anchored to
the right tile, whether `Esc` and the mouse-leave grace period behave as expected around it, and
whether any of this looks right rather than merely being internally consistent. §3.9 stays
NOT YET RUN for those reasons; only the stuck-flag hazard it exists to catch has independent,
automated evidence now.

---

## 4. Dragging out (Task 8)

A tile pressed on the shelf and dragged into any ordinary application. One `DoDragDrop`, started
by `ShelfWindow.StartDrag` and by nothing else, carrying BOTH the shelf's private
`Plith.Shelf.Paths` format and `DataFormats.FileDrop` in a single `DataObject`, offered as
`Copy | Link` and never `Move`. The same gesture is therefore also the restack from Task 7: the
destination is not known until the release, so nothing branches at the source and each target's
own `DragOver` picks the format it understands.

This is the step whose failure mode is not a wrong pixel but a hung process. Measured on
18.09.2026, three runs at Medium integrity with the press verified by `WindowFromPoint` to belong
to another process, `DoDragDrop` never delivered a drag for a press it did not receive: no drop
target saw a `DragEnter`, the call returned `None`, and one of the three did not return at all and
was still blocked seventeen seconds after the release. The full ledger is in
`docs/superpowers/plans/2026-09-17-shelf-drop-catcher.md`, Task 8. That is why `StartDrag` refuses
any drag whose source element does not belong to its own window, and why every item below that
touches the catcher's liveness is worth doing rather than assuming.

### Status: NOT YET RUN

**Every item in this section was unreachable until 2026-09-19, and not for want of a console.**
`DoDragDrop` was never called: the press that would start it was recorded in a local captured by
the tile's own handlers, and that tile was destroyed by the re-render the same press triggers, so
the replacement tile came up with no press behind it and `DragOutRequested` was never raised. A
person at the console would have found §4.1 through §4.9 all failing the same way, with no line in
the log at all rather than a wrong one. Fixed and now driven by the render harness: see §6.1. The
items below stay NOT YET RUN.

The session that built this is `rdp-tcp#0` (`qwinsta`, 2026-09-19), so a drag gesture cannot be
driven and the layered window cannot be captured. Nothing below has been looked at. What was
measured without a console is listed at the end of this section, and it reaches none of it.

**Record two things for every run below**, because this is the measurement the whole slice was
built on top of and the next person should not have to take it on trust:

- the `Drag out returned <effect> for <n> path(s).` line from
  `%LOCALAPPDATA%\Plith\dropcatcher.log`, verbatim, including the effect name, and
- the `Started. Integrity: <level>` line from the top of the same log for that run. It must read
  **Medium**. A `High` there means the catcher was launched as a child of Plith and inherited
  UIAccess, in which case every result in this section is about a different process than the one
  that ships and none of it counts.

Any of `Refused a drag whose press did not land on this window.`, `Refused a drag with no live
press behind it.` or `Refused a drag: one is already in flight.` means `StartDrag`'s guard turned
the gesture away. None of the three should ever appear during an ordinary drag from a tile; if one
does, that is the finding, not the drag that failed. The middle one is the guard that answers the
measurement this task exists because of, so it is the one to look for first: it refuses a call that
arrives with no live button press behind it, which is the shape that hung for seventeen seconds.

Two further refusal lines belong to the window rather than to the drag: `Shelf close refused: a
drag is in flight. It will be taken down when the drag ends.` and `Shelf re-assertion refused at
X,Y WxH: a drag is in flight.` They mean a message arrived over the pipe during a drag and was held
off the live drag source. **Neither can appear in today's code**, for the reasons §4.10 sets out;
if one ever does, something new is sending mid-drag messages and §4.10 is the item to read.

### 4.1 One tile into a file manager

Open the shelf with three items in one stack. Press one tile and drag it into an Explorer or Files
window, release.

Expected: the file lands in that folder, the log says `Copy` or `Link` (never `Move`), and **the
row is still on the shelf**. The row staying is not a leftover, it is the contract: `Copy` is what
was offered, so the shelf keeps its reference. A tile that disappears after a successful drop
means `Move` reached the wire somehow, and the original file may have been deleted.

Check the source folder afterwards: the original file must still be where it was.

### 4.2 A multi-selection drags together

`Ctrl`+click two tiles so both carry the selection ring, then press one of THEM and drag into the
file manager.

Expected: both files land. This is `ShelfModel.DragPaths`'s rule reaching a real drag for the
first time; `ShelfModelTests` covers the rule, and nothing before this item has seen it decide
what a `DataObject` carries.

Then drag a tile that is NOT part of the selection. Expected: only that one file lands, and the
selection on screen collapses to that tile.

### 4.3 Below the threshold it is a click

Press a tile, move about two pixels, release.

Expected: the tile selects and **no drag starts**. Nothing in the log says `Drag out returned`.
The threshold is `SystemParameters.MinimumHorizontalDragDistance` /
`MinimumVerticalDragDistance`, so a machine whose owner has tuned that value will have a different
number of pixels here, which is the point of using the system's value rather than one of ours.

Then press, move clearly further than that (half a tile is plenty), and release back on the shelf.
Expected: a drag DID start, and the log has a return value for it.

### 4.4 Release over empty desktop, and the catcher survives it

**This is the item that would find a hang, and it is the reason this task was scheduled last.**

Drag a tile out and release it over bare desktop, where nothing accepts a drop.

Expected: whatever the shell does with it (most likely nothing at all), the log records a return
value (`None` is a perfectly good answer here), and then:

1. The shelf still responds: `Esc` closes it, or the pointer leaving it takes it down.
2. The notch comes back, which means `ShelfClosed` crossed the pipe, which means the catcher's UI
   thread is still pumping.
3. Drop another file on the notch. It must still be caught.

If the log has **no** `Drag out returned` line for this gesture and the shelf has stopped
responding, `DoDragDrop` did not return: that is the seventeen-second failure from 18.09.2026
arriving in the shipped path, and it is a stop-everything finding rather than a bug to file.
Record the elapsed time before killing `Plith.DropCatcher.exe`.

### 4.5 and 4.6 are GONE: there is no restack (2026-09-20)

Both were about dragging a tile onto another stack through the same `DoDragDrop` call that
carries a drag out. With one flat list there is nothing to restack onto: the within-surface drop
target is deleted and a tile drags out of the shelf only. The drag-out half of that call is
unchanged and is still covered by 4.1 to 4.4.

### 4.7 A drag does not strand the shelf

While a drag is in flight the shelf must not be dismissed by losing activation, because the person
is holding a file over another application by definition. But a dismissal that arrives during one
is DEFERRED, never dropped.

Drag a tile out, hold it over another window for several seconds, then release. Expected: the
shelf survives the whole hold, and goes away afterwards (§4.6's second half). A shelf that stays
on screen forever after this, unreachable by `Esc` and by the pointer leaving, means
`_dragInFlight` was left set: check the log for a `Shelf dismissal deferred` line with no `Shelf
closing` after it.

### 4.8 Cancel with `Esc` mid-drag

Press a tile, drag it off the shelf, and press `Esc` while still holding the button.

Expected: the drag cancels, the log records a return value (`None`), nothing lands anywhere, and
the shelf is in the same state as §4.6's second half afterwards. This exercises the same exit path
as a successful drop, which is the point: `_dragInFlight` is cleared by a `finally`, so a cancel
has to be as safe as a drop.

### 4.9 is GONE with the restack it asked about (2026-09-20)

See 4.5 and 4.6 above. The question it asked, whether the shelf kept its activation across a
restack, has no gesture left to ask it of.

### 4.10 A pipe message arriving in the middle of a drag: NOT TESTABLE TODAY

`DoDragDrop` pumps a modal message loop, so the catcher's pipe reader keeps running and a message
from Plith can be dispatched while a drag is in flight. `OpenAt` and `CloseNow` both refuse while
`_dragInFlight` is true, and the close is remembered and honoured the moment the drag returns.

**There is nothing to run here, and this section says so rather than offering a step that would
produce a false pass.** An earlier draft of this item told a tester to change the display scale
mid-drag and look for `Shelf re-assertion refused`. That step was wrong twice over, and both ways
would have looked like a pass:

- **`OpenAt`'s refusal is unreachable.** `ShelfSession.Open` has one caller, `OsdHost.OpenShelf`,
  which returns early while `_standAside` is `StandAsideReason.Shelf`. That is its state for the
  entire life of the shelf, so no second `OpenShelf` can be sent while one is open, from any
  cause at all.
- **The trigger does not exist.** Nothing in Plith hooks `DpiChanged` or a display-settings change
  to re-send the shelf's rectangle. Changing the scale mid-drag sends nothing.

A tester following that step would have watched the drag survive for a reason having nothing to do
with the guard, found no refusal line, and ticked a check that exercised nothing.

**What would have to exist before either half becomes testable**, which is the useful thing to
write down:

- For the `OpenAt` half: something that re-sends `OpenShelf` while the shelf is up. That means
  both a sender (a `DpiChanged` or display-change hook that re-derives the rectangle) and a change
  in `OsdHost.OpenShelf`, which currently refuses to send anything at all while standing aside.
- For the `CloseNow` half: a `CloseShelf` verb on the wire that the catcher's `App` routes to
  `CloseNow`. There is no such verb, and `App` calls `CloseNow` nowhere.

When either exists, the shape to expect is: the refusal line in the log during the drag
(`Shelf re-assertion refused at X,Y WxH: a drag is in flight.` or `Shelf close refused: a drag is
in flight. It will be taken down when the drag ends.`), the drag completing normally, and for the
close, the shelf going down as soon as the drag returns with no second gesture needed. Note for the
re-assertion half that a refused rectangle that genuinely DIFFERS leaves the card visibly smaller
than its window until the shelf is closed and reopened; that is the documented price of the
refusal, not a second defect.

**Not every mid-drag message is refused, and that is deliberate.** `Items` and `Palette` arrive the
same way and are obeyed: both re-render, which tears down and rebuilds the tiles, including the one
the drag started from. That is survivable where `Hide()` and `Activate()` are not, because the
`DataObject` was built before the render and does not reference any tile, and the drag source
handed to `DoDragDrop` is the WINDOW rather than the tile. A tile vanishing mid-drag is a visual
oddity, not a cancelled gesture. If a hardware run ever shows a drag dying when the shelf refreshes
under it, that reasoning is what was wrong.

### 4.11 The stand-in must not be visible while the shelf is open

The internal tile drag now carries `DataFormats.FileDrop`, and `CatcherWindow` (the notch's
stand-in, in this same process) accepts `FileDrop` as `Copy`. So if the stand-in were ever on
screen at the same time as the shelf, dragging a tile across it would drop the same files back onto
the shelf they came from, silently re-adding them.

Open the shelf and look at the top of the screen where the notch was: it must be gone, replaced by
the shelf itself, with no catcher window visible anywhere. Then drag a tile slowly across the whole
top edge of the shelf and release back inside it. Expected: nothing is re-added, and
`%LOCALAPPDATA%\Plith\shelf.txt` holds the same number of entries afterwards as before.

This item is here because the invariant it checks ("the stand-in and the shelf are never up
together") is currently held by the hand-over sequence in §2 rather than by anything in this
window, and Task 8 gave it a consequence it did not have before.

### What HAS been measured, on 2026-09-19

| Measured | Result |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors |
| Tests | `dotnet test Plith.slnx -m:1`: 472 + 17 passed, 0 failed |
| `scripts/check-a11y.ps1` | passed |
| `scripts/check-shared-xaml.ps1` | passed |
| `scripts/check-contrast.ps1` | passed, 234 measurements across 9 accents and both themes |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#A3E635'` | passed, including the Task 5 second-pass check and the Task 7 menu-survives-render check |
| Session type | `qwinsta`: `rdp-tcp#0`, active. Not the console, so no gesture could be driven |

None of that reaches a single item above, and the difference is worth stating plainly rather than
leaving it to be inferred. The test suite is not STA and never constructs `ShelfWindow`; the
render harness constructs `ShelfSurface` but not the window that owns the drag, so `StartDrag` is
not executed by anything in this list. The guard inside it, the `finally` that clears
`_dragInFlight`, the two formats in the `DataObject` and the effect that comes back are all
unexecuted code as far as every green line above is concerned.

---

## 5. Accessibility and keyboard (Task 9)

Every tile, every stack, and the header's clear and new-stack controls now carry an
`AutomationProperties.Name` that actually reaches a real automation peer - a review of this
section's first draft found that "carry a name" and "reach an automation peer" were not the same
claim, and §5.4 is the corrected record of what was wrong and what fixed it. Arrow keys move the
selection, `Space` toggles it, `Enter` opens the current tile, and `Delete` removes the current
selection (or just the current tile, if nothing is selected). `scripts/check-contrast.ps1` now
scans `src\Plith.DropCatcher` as well as `src\Plith`, and measures two more code-behind pairs plus
the selection ring, which had never been measured at all before this task. `scripts/check-a11y.ps1`
now reads code-behind as well as XAML, which is what found the automation-peer defect in the
first place; §5.6 has its own before-and-after record of that.

### What a green build, a green test run and a green lint prove about this section: nothing

Say it plainly rather than by implication, because it is the one fact this whole document exists
to keep in front of a reader: **the test suite is not STA and never constructs `ShelfWindow`, and
the shelf is a layered window with per-pixel alpha that cannot be captured over Remote Desktop by
any means.** Every measurement below that is a build, a test, a lint or a render is real and is
listed because it is real, but not one of them presses a key. A screen reader announcing a tile's
name, an arrow key visibly moving the selection ring, `Space` adding a second tile to a selection,
`Enter` opening a file, `Delete` removing a row - none of that is answered by anything in this
document. All five need a person, a keyboard and a screen reader, at the physical console, and
none of that has happened yet.

### 5.1 The keyboard path itself, and why it lives in `ShelfWindow` rather than `ShelfSurface`

`ShelfWindow`'s own header comment already said why the window, not the page, takes keyboard
focus: "Esc, arrow keys and a visible selection have nothing to arrive at in a window that never
takes focus." `OnPreviewKeyDown` already existed there for exactly that reason, handling only
Escape. This task's plan named only `ShelfSurface.xaml.cs` as the file its keyboard work would
touch, and that turned out not to be possible to honour literally: a key reaches the element that
holds keyboard focus, `ShelfSurface` is never that element, and no amount of code inside it changes
that. `ShelfWindow.OnPreviewKeyDown` now forwards every key that is not Escape to
`ShelfSurface.HandleKey`, which is the smallest change that could make the surface answer a key at
all. This is a deviation from the plan's stated file list, recorded here and in
`docs/superpowers/plans/2026-09-18-shelf-slice-2.md` rather than left unmentioned, and it is a
one-method, two-branch change: Escape still dismisses the shelf exactly as it did before, and
`HandleKey` is free to decline anything it does not use.

### 5.2 The selection ring, made measurable rather than left exempt

`SelectionRing` has no static entry in any theme dictionary: Plith computes it at runtime with
`ContrastInk.RingOn(accent, surfaceEnd)` and sends the answer over the wire, precisely so the ring
cannot drift from the rest of the product's contrast-derived colours. A reviewer of an earlier task
correctly flagged that as invisible to `check-contrast.ps1`'s resource-key sweep: there is nothing
for `TryFindResource` to find.

It is measurable, and now is: `check-contrast.ps1` calls `AccentTheme.Derive(...).Accent` and
`AccentTheme.DeriveOsdSurfaces(...).SurfaceEnd` (the same two calls `ShelfSession.DerivePalette`
makes, not a second, looser derivation that could quietly disagree with production) and then
`ContrastInk.RingOn` itself, for the same 9-accent, 2-theme matrix as everything else the script
checks. `ContrastInkTests` already covers `RingOn`'s own logic on hand-picked colours; what it
cannot cover is whether the real function clears 3:1 for the accents this script actually sweeps,
on the surface colour the product actually derives for each one. Measured on 2026-09-19: all 18
combinations (9 accents x 2 themes) clear the 3:1 non-text threshold, folded into
`check-contrast.ps1`'s existing 288-measurement total. The white-accent render below shows the
walked case on screen: a lime accent needs no walk at all, and a near-white one visibly does.

### 5.3 The stack caption, added because the alternative was a fake measurement

Task 9's brief handed over two `check-contrast.ps1` entries to add verbatim, one of them labelled
`ShelfSurface.xaml.cs:stack caption` with `NotchInkMuted` as the foreground. Nothing in
`ShelfSurface.xaml.cs` used `NotchInkMuted` before this task - the pair would have measured a
colour nothing drew, which is exactly the failure mode this script's own header comment warns
against ("a gate that reports pairs the product does not have is a gate people learn to argue
with"). Rather than add the check and leave it hollow, `BuildColumn` now draws a small "Stack N"
caption above each column in `NotchInkMuted`, which is also the sighted match for the per-stack
`AutomationProperties.Name` §Step 1 below required, and which needed a wrapping `Border` around
each column regardless. The first version of that wrapper reached nothing at all; see §5.4 for
why, and for what actually carries the name now.

This added height to a control whose frame size (`NotchGeometry.ShelfFrameDip`, 384 x 224) is a
measured constant from an earlier task, fixed elsewhere, and out of this task's file list to
change. Rather than trust arithmetic about whether 13 more DIP of caption would fit inside
whatever slack was left, it was rendered: `scripts/render-widgets.ps1 -Theme Dark -Accent
'#A3E635'`, `-Theme Light -Accent '#A3E635'` and `-Theme Dark -Accent '#FFFFFF'` all produced
`shelf-surface.png` with the caption, the tiles and the selection ring all inside the frame, no
clipping against the rounded bottom corners in any of the three. That is a real measurement (the
control was actually laid out at 384 x 224 and the pixels actually inspected), and it is also the
limit of what a render proves: it says the OFFSCREEN control fits at this size, not that the
on-screen shelf window does, since the render harness does not construct `ShelfWindow` and cannot
reach the layered window at all.

### 5.4 Automation names, and the defect class they were built to dodge, then reproduced, then fixed

**Correction to what this section claimed on first submission.** It said "Every tile `Border`,
the overflow tile, ... and the header's clear and new-stack buttons all carry peers already
(`Border` and `Button` both have one)". That is false. `Border` does not have an automation
peer, `check-a11y.ps1`'s own `$peerless` list already named it as one of the WPF types that
never gets one, and this project's own `WidgetFrame.xaml` says the same thing in these words. A
review of this task caught it: every `AutomationProperties.SetName` this task had written landed
on a `Border` (the tile, the overflow tile, the new per-stack wrapper) or on the `Columns`
`StackPanel` (also peerless), which reproduced the exact defect this section's first paragraph
was busy describing, one layer further out. `Button` genuinely does have a peer; that half of the
sentence was correct.

**The fix, and the smaller of two shapes it could have taken.** Every `Border` that carries a
name in `ShelfSurface.xaml.cs` (the tile, the overflow tile, the per-stack wrapper) is now a
`NamedBorder`, a private nested class that overrides `OnCreateAutomationPeer` to return a real
`FrameworkElementAutomationPeer`. The whole-shelf aggregate name (`"Shelf, N stacks"` / the
empty-shelf sentence) moved off `Columns` onto `ShelfSurface` itself, the `UserControl` root,
which does have a peer - the same fix `AmbientCardView`/`AudioCardView`/`MediaCardView` already
use for the same reason, recorded in `AmbientCardView.xaml`'s own header comment.

This was the SMALLER of two fixes the review raised. The larger one: these tiles are selectable
and activatable, arrow keys move between them, and a genuine `ListBoxItem` would give a screen
reader the `SelectionItem` pattern for free rather than a name alone. That was not done here,
because it would mean rebuilding drag-out, the hover remove affordance, the context menu and this
same task's keyboard handling on top of a `Selector`'s own model instead of `ShelfModel`'s, all of
it unverified on hardware either way. `NamedBorder` is real (every name below now reaches an
actual automation peer, checked by construction and by the extended lint in §5.6, not merely
argued for) but it is not the richer shape. If the console pass this project still owes finds that
insufficient, the `ListBoxItem` rewrite is the next step, not a surprise; see `NamedBorder`'s own
header comment in `ShelfSurface.xaml.cs` for the same reasoning kept next to the code.

**What carries a peer now:** every tile `NamedBorder`, the overflow tile `NamedBorder`, the new
per-stack `NamedBorder` wrapper, the `ShelfSurface` `UserControl` root (carrying the aggregate
"Shelf, N stacks" / empty-shelf name), and the header's clear and new-stack `Button`s.
**What does not, and carries no name:** the inner `StackPanel`s (the per-stack column, the
per-tile icon/label stack) and the separator `Path` - nothing is named on them, so their being
peerless costs nothing.

**KNOWN LIMIT, not fixed in this task: a tile's SELECTION is invisible to the automation tree.**
`Space` toggles a tile into or out of the current selection and the ring changes on screen, but
`NamedBorder` carries a plain `FrameworkElementAutomationPeer`, not a `SelectionItemPattern`.
Nothing in the automation tree announces that a toggle happened at all: a screen reader hears a
tile's name once, when it is first reached, and nothing when its selection state changes
afterwards. This is the direct cost of choosing `NamedBorder` over a `ListBoxItem` rewrite (see
above); the review that accepted that choice for now still asked for the gap itself to be written
down where the next slice will look for it, not only in a task report nobody re-reads. A real fix
needs the `Selector`/`ListBoxItem` rewrite this section already describes, or some other way to
raise a `SelectionItemPattern` (or a live-region announcement on toggle) without it - not attempted
here.

None of this - whether a screen reader actually reaches any of it, whether the announced text
reads sensibly in sequence, whether Narrator's own quirks change any of it - has been checked with
a screen reader. `check-a11y.ps1` (as extended in §5.6) catches a name on a peerless element and a
missing name on an interactive control; it does not, and cannot, catch whether the resulting
announcement makes sense to a person listening to it.

**The notch's own glance page has the identical defect, and predates this task.**
`src/Plith/Views/Widgets/ShelfWidget.cs` sets `AutomationProperties.SetName` on `Tiles` (a
`StackPanel`, `x:Name`'d in `ShelfWidget.xaml`) and on every tile `Border` its `Tile(...)` method
builds. Both are peerless by the same rule as above, so both names are inert, and this has
shipped since before this branch existed. Not fixed here: this task's job is the shelf surface in
`Plith.DropCatcher`, not the notch's read-only glance page, and the review that found this asked
for it to be reported rather than fixed in the same pass that found it. Filed in `check-a11y.ps1`
itself (§5.6) as a known, named gap with an owner, so it is not rediscovered from scratch later.

Extending the lint to see code-behind at all (§5.6) also surfaced three MORE pre-existing
instances of the same defect, none of them related to the shelf: `MediaWidget.cs`'s
`OpenSourceArea` (a `Border`), `NotchHud.cs`'s `VolumeRow` and `MediaRow` (both a `Grid`), and
`WeatherWidget.cs`'s `Readout` (a `StackPanel`). All three predate this branch and are also filed,
not fixed, in `check-a11y.ps1`'s own `$knownCodeBehindGaps` table. This is worth a look from
whoever owns the notch widgets: `check-a11y.ps1`'s original motivating bug (its own header
comment: "the OSD's live region was completely inert" on `AmbientCardView`/`AudioCardView`/
`MediaCardView`) was fixed on those three views by moving the name to the `UserControl` root, and
these three files did not get the same fix even though they carry the identical pattern.

### 5.5 Harness lessons, carried forward for whoever drives this section at the console

Five of eight attempts at Task 8's drag measurement (slice 1) were lost to the harness rather than
to the question, and every one of them failed silently: a run that aims at the wrong window still
produces a plausible log line. Recorded there, and repeated here because this document is where a
person preparing to test §5.1-§5.4 by hand will actually be looking:

- **Ask `qwinsta`, never `$env:SESSIONNAME`.** The environment variable is stamped when a process
  starts and never updated; it has been caught wrong on this machine before.
- **Stage and aim in one process, and re-verify the aim in the same breath as the press.** Across
  two separate scripts another window came forward in the gap, twice, and took the input meant for
  the staged one. A check that finds the wrong target must abort, not warn.
- **Use windows the harness itself created.** Matching a generic title (`Notepad`, `Explorer`) can
  find and act on the person's own open work instead of the harness's fixture.
- **Topmost is not enough; minimise the competing window.** Pinning a staged window
  `HWND_TOPMOST` still lost to a maximised terminal on two runs. Minimising the competitor for the
  duration of the gesture, and restoring it in a `finally`, is what made runs repeatable.
- **`WindowFromPoint` returns the CHILD under the cursor**, never the top-level handle a window was
  found by. Compare owning process ids, not raw handles.

None of these is specific to a drag; every one of them applies just as much to a script that drives
a keyboard gesture against `ShelfWindow` and then reads `dropcatcher.log` or a screen reader's
output afterwards.

### 5.6 `check-a11y.ps1` could not see any of this, and now can, for code-behind

The lint that should have caught §5.4's defect never ran against the code that carried it.
`check-a11y.ps1`'s dead-property check (the one that fails a `Grid`/`StackPanel`/`Border` named
with `AutomationProperties`) parsed only `.xaml` files, and defaulted its scan root to
`src\Plith`. `ShelfSurface.xaml.cs` was invisible on both axes at once: wrong extension, wrong
directory. Running it and reading "Accessibility check passed" was true and worthless at the same
time - it never read a single line of the file it needed to.

**Before the fix**, the extended check run against the version of `ShelfSurface.xaml.cs` this
task first submitted (recovered from git history and checked in isolation, not left as a
guess):

```
Accessibility check failed:
  AutomationProperties that never reach UI Automation:
    ShelfSurface.xaml.cs: AutomationProperties.SetName(Columns, ...) targets a StackPanel, which WPF gives no automation peer
    ShelfSurface.xaml.cs: AutomationProperties.SetName(wrapper, ...) targets a Border, which WPF gives no automation peer
    ShelfSurface.xaml.cs: AutomationProperties.SetName(tile, ...) targets a Border, which WPF gives no automation peer
```

**After the fix** (the `NamedBorder`/root-name change in §5.4), the same run against the current
file:

```
Accessibility check passed: every interactive control has an accessible name, every AutomationProperties
value sits on an element that can surface it, and no view depends on a system icon font.
  KNOWN GAP, not fixed here: ShelfWidget.cs: AutomationProperties.SetName(tile, ...) targets a Border (...)
  KNOWN GAP, not fixed here: ShelfWidget.cs: AutomationProperties.SetName(Tiles, ...) targets a StackPanel (...)
  KNOWN GAP, not fixed here: MediaWidget.cs: AutomationProperties.SetName(OpenSourceArea, ...) targets a Border (...)
  KNOWN GAP, not fixed here: NotchHud.cs: AutomationProperties.SetName(VolumeRow, ...) targets a Grid (...)
  KNOWN GAP, not fixed here: NotchHud.cs: AutomationProperties.SetName(MediaRow, ...) targets a Grid (...)
  KNOWN GAP, not fixed here: WeatherWidget.cs: AutomationProperties.SetName(Readout, ...) targets a StackPanel (...)
```

`ShelfSurface.xaml.cs` and `ShelfWindow.xaml.cs` carry nothing on that list now. The five findings
that remain are pre-existing, filed as known gaps with an owner rather than fixed or hidden (see
§5.4's last two paragraphs), and are what `check-a11y.ps1`'s own `$knownCodeBehindGaps` table
names them as.

**What the extension covers, and what it cannot.** It is a regex heuristic over C# text, not a
compiler: it resolves a target's type from either a local `var name = new Peerless { ... }` in
the same file, or an `x:Name="name"` in the sibling XAML file (tried both as `Foo.xaml.cs` beside
`Foo.xaml`, and as `Foo.cs` beside `Foo.xaml` - `ShelfWidget.cs` uses the second shape, and the
first draft of this extension only tried the first, which is why it missed `Tiles` until that
was corrected too). A name set through an alias, built in one method and named from another in a
way this cannot re-derive, or set on a collection element, is a gap in this heuristic's own
coverage, not a pass - the same limit the XAML-side check already states for a name set on a
child of a parent it did not itself declare. It now scans `src\Plith.DropCatcher` as well as
`src\Plith`, added as a separate root list rather than by widening `$Root` itself, for the same
reason `$iconFontRoots` already keeps its own list: widening `$Root` would also turn on Check 1
(every interactive control needs a name) for the rest of `Plith.DropCatcher`, which nobody has
reviewed for that yet.

### What HAS been measured, on 2026-09-19

| Measured | Result |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors, 0 warnings |
| Tests | `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`: 472 passed, 0 failed |
| `scripts/check-a11y.ps1` | passed |
| `scripts/check-shared-xaml.ps1` | passed |
| `scripts/check-contrast.ps1` | passed: 288 measurements across 9 accents and both themes (was 234 before this task: +36 from the two new `ShelfSurface.xaml.cs` pairs, +18 from the selection ring) |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#A3E635'` | passed, including the Task 5 second-pass check and the Task 7 menu-survives-render check; `shelf-surface.png` shows both stack captions, no clipping |
| `scripts/render-widgets.ps1 -Theme Light -Accent '#A3E635'` | passed; same layout, legible in light theme |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#FFFFFF'` | passed; selection ring visibly walked off the raw (illegible) white accent |
| Session type | `qwinsta`: `rdp-tcp#0`, active. Not the console, so no key could be pressed and no screen reader could be run |

As with every section before it, none of the row above reaches a single item in §5.1-§5.4. A test
suite that is not STA and a render harness that never constructs `ShelfWindow` can prove that
`ShelfSurface` lays out correctly and that its colours clear their thresholds; neither can prove
that a key press reaches it, that the announced names make sense read aloud, or that the shelf is
usable by someone who cannot see it. That is the console's job, and it is still undone.

---

## 6. The whole-branch review (2026-09-19)

Nine tasks were built and each was reviewed against its own brief. A review of the whole branch
then found four defects that no task-scoped review could have seen, because each task was correct
and the JOINS between them were not. This section records what was wrong, how it was settled, and
what is now measured as against reasoned.

### 6.1 The headline feature could not execute, and now has an automated check

**What was wrong.** `ShelfWindow` answers `ShelfSurface.EntryPressed` by calling `Page.Render`,
and `Render` clears `Columns.Children` and rebuilds every tile. `ShelfSurface` kept the press
(`pressStart`) in a local captured by the pressed tile's own handlers. So the element that took
the press was out of the tree before the button came up, the replacement tile's `PreviewMouseMove`
returned early on a fresh null, and `DragOutRequested` was never raised. **Dragging a file out to
another application and dragging a tile between stacks were both dead**, on the first gesture and
on every one after, because a second press re-rendered again.

One task wrote the press-then-render, another wrote the press-then-drag, and each is correct on
its own. Nothing on this branch could have caught it: the suite is not STA, and the render harness
had never pressed a tile.

**How it was settled.** The press moved off the element and onto the surface, keyed by PATH
(`ShelfSurface.BeginPress` / `ContinuePress` / `EndPress`). The alternative offered was to
re-render only when the selection actually changed and preserve the pressed element; it was not
taken, because it closes only the render this press causes. Plith re-sends the whole shelf after
every mutating verb and a `Palette` message re-renders too, so ANY render can land between a press
and the move that follows it, and a fix aimed at one of them leaves the next person to add a
render call to rediscover the same defect. The origin point also moved from tile coordinates to
the surface's, so a rebuild that puts the same path in a different slot cannot read as a large
pointer movement and start a drag nobody asked for.

**The check, and what it said.** `scripts/render-widgets.ps1` gained a **press-to-drag check**,
in the style of the second-pass icon check and the menu-survives-render check beside it: it
asserts its preconditions first so it cannot pass vacuously.

It raises a real `PreviewMouseLeftButtonDown` on a real tile, with `EntryPressed` wired exactly
the way `ShelfWindow` wires it (select, then re-render), then asserts that the press reached
`EntryPressed`, that a tile for the pressed path still exists, and that it is **not the same
object** as the one pressed (so the teardown this check exists for really happened). Only then
does it drive the move: once under the system drag threshold, which must raise nothing and must
not consume the press, and once past it, which must raise `DragOutRequested` exactly once, for the
element under the pointer, carrying the pressed path.

Before the fix, against a press keyed to the element that took it (which is what a per-tile
closure is):

```
press-to-drag check FAILED: a press that triggered a re-render can no longer become a drag.
The rebuilt tile has no press behind it, so DragOutRequested is never raised - dragging a file
OUT and dragging a tile between stacks are both dead. This is the Critical the whole-branch
review found.
```

After:

```
  press-to-drag check passed: a press that re-rendered the shelf under itself still became a
  drag on the rebuilt tile (threshold 4 x 4 honoured on both sides), carrying the pressed path.
```

**What this check does NOT reach, stated because this document's whole job is that line.** The
move's position and button state come from the mouse device, and neither can be synthesized
offscreen: `MouseDevice.GetPosition` returns (0,0) for an element in no `PresentationSource`, and
`MouseEventArgs.LeftButton` reports the physical button, which no script can hold down. So the
move arrives through `ShelfSurface.ContinuePress`, the same method with the same arguments that
the tile's own `PreviewMouseMove` calls, and the only thing the handler adds is the `e.LeftButton`
test. The press, the render and the tile teardown are all real; the pointer is not. `DoDragDrop`
is still executed by nothing in any gate, so section 4 stays NOT YET RUN in full.

### 6.2 The shelf could close under the pointer, for the third time

**What was wrong.** The leave timer does two jobs: it is the pointer's grace period, and it is
also the clock that re-evaluates a deferred dismissal. Every place that made a deferral moot
therefore owed it a `Stop()`. `OpenAt` and `StartDrag` had both been taught; `Activated` had not.

The sequence: right-click a tile, the menu takes activation, `Deactivated` fires, `Dismiss`
defers and arms the leave clock, the menu closes, `Activated` sets `_pendingDismissal` to null
while the timer keeps running, and within the grace period the tick fires a now-unsuppressed
`Dismiss` and closes the shelf with the pointer sitting on it. `MouseEnter` only saved it when it
happened to arrive after `Activated`.

**How it was settled.** Not by stopping the clock in one more place. **The tick itself now asks
where the pointer is**, through `PointerIsOverShelf` (the window manager, not WPF, for the reason
that method documents), and refuses to dismiss while the pointer is over the shelf, re-arming
instead so a pending deferral is never left without a clock. The only reason this timer ever
closes the shelf is "the pointer left and did not come back", so the pointer being here refutes it
outright, whoever armed the clock and for whatever reason. A fourth instance of this defect cannot
be written by adding a fifth arming site.

`Activated` deliberately does NOT stop the clock: activation says nothing about where the pointer
is, and stopping it from there would leave a shelf whose pointer really had left with no clock, no
pending dismissal, and no second `Deactivated` to come.

**Status: REASONED, NOT RUN.** Nothing automated executes `ShelfWindow`. Section 3.9 is the item
that drives this at the console, and it is still NOT YET RUN. What to add to it when it is run:
right-click a tile, close the menu with the pointer resting on the shelf, and wait out the grace
period twice over. The shelf must still be there.

### 6.3 One end of the pipe was serialized and the other was not

**What was wrong.** `DropChannelServer.SendAsync` (Plith's end) grew a call-ordered chain because
overlapping fire-and-forget sends corrupted lines on the shared stream. The catcher's end still
wrote straight to one shared `StreamWriter`, and every call site there is fire-and-forget too.

Two sends in flight used to be hard to reach, with two verbs on this side. This slice added five
more and made it ordinary: a `Restack` raised from inside `DoDragDrop`, followed milliseconds
later by the dismissal's `ShelfClosed`. A `StreamWriter` throws `InvalidOperationException` for
the overlap, `SendAsync` did not catch it, the task faulted unobserved, and the message was
silently lost. **A lost `ShelfClosed` leaves Plith's window hidden with no notch and no OSD on any
volume key until the catcher dies.**

**How it was settled.** `CatcherClient.SendAsync` got the same treatment the server end has: a
task chain linked under a lock (not a `SemaphoreSlim`, which documents no FIFO guarantee for async
waiters), a `Task.Run` first so a pipe write never runs on the caller's UI thread inside the lock,
and a link that cannot fault. It catches one type the server's does not need to:
`InvalidOperationException`, which is both what a `StreamWriter` raises for the overlap above and
what `PipeStream` raises for a write into a pipe that has gone.

**Status: NOT DIRECTLY TESTED, and this is the honest half.** The server end has
`DropChannelServerTests.Server_DoesNotInterleaveOverlappingSends`, which fails without its fix.
The catcher end has no equivalent: `CatcherClient` is `internal` to a project `Plith.Tests` does
not reference (it links `ShelfModel.cs` by file, precisely because that one class holds no WPF or
pipe types), and this pipe is created with both buffer sizes at zero, so a test that awaits a send
before starting a read deadlocks rather than fails. What exists is the symmetry with a fix that
WAS measured on the other end, and that is weaker than a test.

### 6.4 The accessibility gate was blinded by its own suppression list

**What was wrong.** `check-a11y.ps1` looked its known code-behind gaps up by FILE NAME, so every
dead-property and unresolved-type hit in `ShelfWidget.cs`, `MediaWidget.cs`, `NotchHud.cs` and
`WeatherWidget.cs` was downgraded to a yellow notice. A new inert accessible name added to any of
them would have passed green, including in `ShelfWidget.cs`, which this branch rewrote. The
script's own header rejects exactly that shape for its root exclusion, saying it would hide
anything else the scan ever finds in the same file, forever, and then adopted it one level down.

Two more holes in the same scan. The call finder matched the target as a bare identifier inside
the same regex, so `SetName(BuildTile(), ...)`, `SetName(tiles[i], ...)` and `SetName(this.Foo,
...)` matched nothing at all: not a third answer, **no answer**, invisible to a check whose whole
point is that "could not tell" must never print the same nothing as "this is fine". And Check 1
and the XAML half of Check 2 still never ran on `src/Plith.DropCatcher`, so its two XAML files
were unscanned. They were clean, which is luck rather than coverage.

**How it was settled.** The suppression is keyed `<file>:<target>`, so the known gap is a named
element in a named file and anything else in the same file fails. The finder matches the call and
then reads its first argument by hand, tracking bracket depth, so a target that is not a bare
identifier is seen and lands in the existing "could not resolve a type" failure rather than
vanishing (`this.Foo` is unwrapped to `Foo`, since that is the same field written the long way).
Both name checks now scan `src/Plith.DropCatcher` as well; the exclusion was reviewed rather than
merely dropped, by running the check against it: the two `Button`s in `ShelfSurface.xaml` both
declare `AutomationProperties.Name`, and no other file in the project declares an interactive
control at all, so the exclusion was buying nothing.

**Measured, both holes, on 2026-09-19.** A `Border` named inertly in `ShelfWidget.cs` (an already
suppressed file) and a `SetName(BuildProbe(), ...)` beside it were added temporarily and the check
run:

```
Accessibility check failed:
    ShelfWidget.cs: AutomationProperties.SetName(probeBorder, ...) targets a Border, which WPF gives no automation peer
  AutomationProperties.SetName targets this scan could NOT resolve a type for:
    ShelfWidget.cs: AutomationProperties.SetName(BuildProbe(), ...) - this scan could not determine BuildProbe()'s type
```

Before the fix both were silent. The probes were removed and the check passes again, with the
same six known gaps (now six findings rather than four files) reported as notices.

### 6.5 Minor findings

- **`ShelfModel.SetStack` pre-allocated `total` slots with no cap**, so a hostile `Items` message
  could exhaust memory in the process holding the shelf, the notch's stand-in and the pipe. The
  pipe name is deterministic and any local process can write to it, or squat it before Plith
  starts. Clamped to 20, which is not a taste: `ShelfStore.MaxItems` is 20 across the whole shelf
  and an empty stack is discarded when the surface closes, so 20 stacks is the most Plith can ever
  legitimately send, and the surface draws five. Truncated rather than rejected, so `_expected`
  then disagrees with the `total` its siblings carry and the set assembles NOTHING rather than
  something plausible and wrong. Covered by
  `ShelfModelTests.SetStack_WithAHostileStackCountIsClampedAndAssemblesNothing`.
- **`DropCatcherLauncher.EnsureRunning` leaked a process handle per shelf open.** Disposed, the
  way `FindProcessId` next door already did.
- **One em dash in `ShelfStore.Add`'s doc comment**, in a sentence this branch re-wrapped. Gone.

### 6.6 The stale press, found by the scoped re-review

The press fix in 6.1 was right about the render and wrong about the release. **No mouse capture is
taken**, so a release outside the window raises nothing on `ShelfSurface` at all, and only its
button-up cleared the press. A drag out **ends outside the window by definition**, which is what it
is for, so a press was left recorded after every successful one. The next button-down on anything
that does not run `BeginPress` (the header, the Clear or new-stack button, the gap between two
tiles) followed by a move onto a tile would then start a drag from a press that never landed on a
tile, measured against an origin from the gesture before it. That is the exact failure family this
whole branch exists because of, arriving from the other side.

Settled by one line, clearing on button-DOWN at the root, which tunnels ahead of the tile's own
`BeginPress` so a real tile press re-records immediately and loses nothing.

**The harness check covers it**, as a controlled comparison rather than an assertion on its own:
the same far point, on the same surface, through the same method, raises a drag on a live press and
does NOT raise after a down that landed on no tile. Removing the root clear and rebuilding makes
the second half fail with its own message; the first half is the control that keeps it from passing
quietly.

### 6.7 Known limits, recorded rather than fixed

Three things the fixes above do not cover. Each is real, each is going to the user as a known
limit, and each is written here so it is not rediscovered as a surprise.

**1. `Deactivated` still closes without asking where the pointer is.** 6.2 put the pointer question
in the leave timer's tick, which is the only place the shelf closes *for* the pointer having left.
The other route, `Deactivated` to `Dismiss` to `CloseNow`, does not consult the pointer at all: it
is guarded only by `_dragInFlight` and `_menuOpen` **already being set when `Deactivated`
arrives**. For a context menu that depends on the menu's `Opened` firing before the `Deactivated`
its own appearance causes. **That ordering was inferred from the observed bug sequence and has
never been measured.** If it is ever the other way round, `_menuOpen` is still false when `Dismiss`
runs and the shelf closes under the menu outright, which is a fourth instance of the
close-under-the-pointer defect by the one route 6.2 does not cover. A console item would open a
tile's context menu and look at whether the shelf is still there, and `dropcatcher.log` would
distinguish the two: a `Shelf dismissal deferred` line means the flag was set in time, a `Shelf
closing: another window took focus` means it was not.

**2. A dismissal deferred by a menu can no longer complete while the pointer rests on the shelf.**
This is the price of 6.2 and it is the better of the two outcomes, but it has a visible shape.
Right-click a tile, then Alt+Tab away with the pointer left sitting on the shelf: the deferred
dismissal is now refused by every tick, `Esc` cannot reach a window that no longer has activation,
and no second `Deactivated` will come, so **the shelf stays topmost over the new foreground
application until the mouse moves off it.** Moving the pointer away takes it down within the grace
period. Needs a console item: do exactly that, and record how long it takes to look wrong.

**3. A message queued while the pipe is DOWN is still dropped.** The chain in 6.3 fixes overlapping
sends, not a disconnected one: `CatcherClient.WriteAsync` returns silently when there is no live
writer, so a `ShelfClosed` that arrives after the pipe has gone is lost exactly as before. **The
outcome is nonetheless handled, from the other end**, which is why this is a limit rather than a
defect: `DropChannelServer` raises `Disconnected`, `ShelfSession` answers it by ending the `Shelf`
stand-aside and putting the notch back, and that path is tested
(`ShelfSessionTests.ChannelLost_WithAShelfOpen_PutsTheNotchBack`, against a real connected pipe)
and has its own console item at section 2.7. So the case where a `ShelfClosed` cannot be delivered
is the case where Plith already knows the catcher is gone.

### 6.8 The catcher shipped nowhere, found at the console

**The shelf could not work in any installed build, and every gate this repository has was green
while that was true.**

Found on 2026-09-19 at the physical console, while trying to start Plith the way it really
starts in order to make section 2.1 mean anything. The autostart entry points at
`C:\Program Files\Plith\Plith.exe`. That directory held `Plith.exe`, dated 2026-09-17, and **no
`Plith.DropCatcher.exe` at all**.

The chain, each link measured rather than inferred:

1. `Plith.csproj` copies the catcher beside `Plith.exe` from `CopyDropCatcherBesidePlith`, which
   runs `AfterTargets="Build"` and writes into `$(OutDir)`. That is the BUILD output.
2. `dotnet publish` computes `ResolvedFileToPublish` on its own and does not sweep up files a
   custom target dropped into the build folder. Measured: a fresh publish produced 17 files in
   the stage root and the catcher was in none of them.
3. `scripts/manual-install.ps1` publishes into a stage and copies THAT into
   `C:\Program Files\Plith`. `src/Plith.Installer/Plith.Installer.csproj` does the same publish,
   zips the whole staging directory and embeds it as `PlithBundle.zip`. Both install paths carry
   the publish output, so both carried the hole.
4. `DropCatcherLauncher` resolves the catcher from
   `Path.GetDirectoryName(Environment.ProcessPath)` and nowhere else, and returns
   `CatcherStart.NotFound` when the file is absent.

So an installed Plith answers every shelf drop with `NotFound`. Section 2.6 wrote that sentence
as an edge case reachable by renaming the executable by hand. It was in fact the default state
of the product.

**What no gate could see.** The build is green because the catcher builds. The 473 + 17 tests
are green because none of them looks at an artifact. `check-a11y.ps1`, `check-shared-xaml.ps1`
and `check-contrast.ps1` are green because all three read source. `render-widgets.ps1` is green
because it renders controls in-process. Every one of them looks at the build output or at the
code; not one looks at what ships. That is the gap, and it is a category rather than an
oversight: a file that builds correctly and publishes nowhere is invisible to all of them.

**Fixed** by a second target, `PublishDropCatcherBesidePlith`, hooked to
`ComputeResolvedFilesToPublishList`, which adds the catcher's output to `ResolvedFileToPublish`.
The glob both targets need now lives in one `DropCatcherOutputDir` property, because two copies
of it drifting apart is the same defect class again. Everything is published rather than a
hand-picked list, so the installed layout matches the one dev runs against.

**Gated** by `scripts/check-publish-shape.ps1`, which publishes for real and inspects the
artifact. It names each required file with the consequence of its absence rather than as a
missing path.

**The gate was falsified before it was trusted**, as this document requires. Run against `HEAD`
before the csproj change it failed, naming `Plith.DropCatcher.exe`, `Plith.DropCatcher.dll` and
`Plith.DropCatcher.runtimeconfig.json`, with the stage at 17 files. Run after, it passes with
the stage at 22.

One thing this does NOT prove: no installer has been built and run end to end since the fix. The
chain from publish output to `PlithBundle.zip` is a `ZipDirectory` over the whole staging
directory, so it follows, but it follows by reading rather than by measurement. Worth an actual
installer run before release.

### 6.9 The installer shipped the old catcher, silently, whenever one was running

Found on 2026-09-19 while trying to look at a redesigned shelf and being shown the previous
design by a build that had just reported a successful install.

`scripts/manual-install.ps1` stopped `Plith.exe` before copying and did not stop
`Plith.DropCatcher.exe`. The catcher is a separate process holding its own
`Plith.DropCatcher.dll` open out of `C:\Program Files\Plith`, so the copy over that one file
failed with a sharing violation while the other twenty-one landed.

Every signal said the install had worked. The script's own verification reads `Plith.exe`'s
version, and `Plith.exe` copied fine. The result is a product running new code in one process
and five-hour-old code in the other, across a named pipe whose message format both ends must
agree on.

Measured rather than inferred: the installed `Plith.DropCatcher.dll` was stamped `10:03:55` and
92,672 bytes while `bin/Release` held `15:07:14` and 93,696 bytes, and a fresh publish into a
temporary directory produced the newer one, which ruled out the publish path (6.8) as the cause.

**Fixed**: the script now stops the catcher too, Plith first and the catcher second, because
Plith restarts the catcher when it sees it go and the other order can leave a fresh one holding
the file again by the time the copy starts. A catcher that will not die is reported as a warning
naming the consequence rather than as a silent skip.

**Not fixed, and worth someone's attention**: the verification step still only checks
`Plith.exe`'s version. An install that fails to replace the catcher can still pass it. What would
close this properly is comparing the installed catcher against the staged one, and that is a
small change nobody has written yet.

One honest note about how this was found, because it was nearly missed: the install output WAS
reporting the failure. It prints a `failed: N` line with the file name and the exception message.
It was invisible because the command that ran it piped the output through `Select-Object -Last 6`,
which cut exactly the lines that mattered. The script did its job and the way it was called threw
the answer away.

### 6.10 The notch came back in the corner, and the flags were the reason

Reported from the physical console on 2026-09-19: after the shelf closed, the notch reappeared in
the TOP-LEFT corner of the screen and stayed there for several seconds before returning to
centre.

First instrumented against the wrong window. A watcher polling Plith's main window rect reported
`1088,0 384x130`, dead centre, delta 0, across a full minute: the gesture had simply not been
made during that window, and the run was reported as "not reproduced" rather than as proof of
anything. A second watcher, over EVERY visible window of both processes, caught it on the first
try:

```
12:31:04.494  CATCHER 1088,0 384x224 'Plith shelf'      <- the shelf, centred
12:31:05.753  PLITH      0,0 384x130                    <- the notch, 1088 px left of centre
```

The cause is two `SetWindowPos` calls in `OsdHost`:

```csharp
SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_HIDEWINDOW);   // HideForCatcher
SetWindowPos(Handle, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_SHOWWINDOW);   // RestoreNotch
```

The four zeros are the conventional filler for "not moving or sizing anything", but that meaning
lives in `SWP_NOMOVE` and `SWP_NOSIZE`, which neither call passed. So the hide really did move the
window to 0,0, where nobody could see it because it was hidden, and the restore showed it there.
It stayed until the next `Reposition` ran, which is the several seconds the report described. The
correct call was already in the same file fifty lines below.

A second symptom from the same cause, reported in the same breath and worth recording because it
looks like a different bug: the notch could still be OPENED from the centre of the screen while
the visible strip sat in the corner. Only the window had moved; the hover rectangle and the
positioning arithmetic still described the centre.

**Fixed** by adding both flags, and **gated** by `scripts/check-win32-flags.ps1`, which fails any
`SetWindowPos` passing four zero coordinates without declaring them. The gate was falsified before
it was trusted: removing either flag from either call site fails it with that call site's line
number, and it distinguishes the two spellings in this repository (`SWP_NOMOVE` in `OsdHost`,
`SWP.NOMOVE` in `BandWindow`) after reporting BandWindow's perfectly correct call as a defect on
its first run.

**Verified at the console after the fix**: `1088,0 384x130`, and no window at the left edge.

### What HAS been measured, on 2026-09-19, after these fixes

| Measured | Result |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors |
| Tests | `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`: 473 passed, 0 failed (was 472: +1 for the stack cap) |
| `scripts/check-a11y.ps1` | passed, with six known gaps as notices; both new holes proven closed by temporary probes (6.4) |
| `scripts/check-shared-xaml.ps1` | passed: 3 files compiled into both Plith and the installer, none naming an assembly |
| `scripts/check-publish-shape.ps1` | NEW, see 6.8. Failed before the fix (17 files, catcher absent), passes after (22 files) |
| `scripts/check-contrast.ps1` | passed: 288 measurements across 9 accents and both themes |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#A3E635'` | passed, including the new press-to-drag check |
| `scripts/render-widgets.ps1 -Theme Light -Accent '#A3E635'` | passed, same |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#FFFFFF'` | passed, same |
| Session type | `qwinsta`: `rdp-tcp#0`, active. Not the console, so nothing in sections 1 to 5 was driven |

Every NOT YET RUN in sections 1 to 5 is still NOT YET RUN. Three of the four defects above lived
in code that a green build, a green suite and a green lint had all passed over, which is this
document's recurring finding rather than a new one: 6.1 and 6.6 are the only ones that now have a
gate standing over them.

**Both of those gates were falsified rather than trusted**, which is the part worth keeping. The
press fix: inserting `_press = null` into `Render` and rebuilding makes the check fail with its own
message, and HEAD passes. The stale-press fix: removing the root button-down clear and rebuilding
makes the second half fail with its own message. The stack cap: removing the clamp makes the test
fail with `Assert.Single() Failure: The collection contained 2 items`, a clean assertion rather
than the `OutOfMemoryException` an `int.MaxValue` total would have produced, which is why the test
uses 100,000 and says so. A check nobody has seen fail is a check nobody has seen.

---

## 7. The look, reworked after the first person saw it (2026-09-19)

The first time the shelf was looked at on real hardware by the person it is for, the report was
that the design was bad, and, separately, that the feature might not be needed at all. The second
half is a product decision recorded in `docs/ROADMAP.md`. This section is the first half.

### 7.1 What was wrong, and what turned out not to be

The loudest complaint was that the panel is olive green. **That one is not the shelf's.** The
shelf takes its surface from `AccentTheme.DeriveOsdSurfaces`, the same call every other card in
the product makes, and `ShelfSession.DerivePalette`'s header says why it must: a second derivation
would drift and the shelf would stop looking like the product it belongs to. Rendered side by
side, the clock widget is exactly as green. What makes it olive is the ACCENT: `#CAFF33` is a
saturated yellow-green, and tinting a dark surface toward it lands on olive, while emerald and
sky land on clean deep green and navy from the same code.

Recorded as a product finding rather than acted on here, because acting on it means changing how
every surface derives from every accent and re-measuring nine accents across two themes:
`AccentTheme.Presets`' own comment calls the presets "tuned to look decent on both dark and light
surfaces", and lime on a dark surface falsifies that sentence. **A preset the product ships and
does not look decent in is the product's defect, not the user's choice.** The accent was left as
the user set it.

The media widget looking near-black in the same renders is not a counter-example: it paints its
own dark scrim over album artwork and covers the gradient.

### 7.2 What was changed

| | |
|---|---|
| Dead space | Two stacks used about 45 % of the card and the rest was blank. The area past the last stack is now a drawn, dashed new-stack zone, one column wide, and the whole row is centred |
| The count | "+4" over the word "more", at 18-20 point in full ink, inside a full-size tile, read as another FILE. It is one muted line now on both surfaces, with the sentence kept in the tooltip and the automation name |
| Icons | Every tile on the notch page drew the same document outline. The extension is now drawn inside the glyph |
| Empty state | A sentence alone, instructing without showing where. It sits inside a dashed outline now, on both surfaces |
| Stack captions | "Stack 1" says nothing when there is one stack. The caption is silent then; its band is kept so the separator height and tile alignment do not move |
| Header proportions | **Deliberately not changed.** It was on the list, and once the body calmed down the header no longer read as oversized. Changing something that looks right to finish a list is how a list wins an argument against a screen |

### 7.3 Two things this changed that were not visual at all

**The icon fix was redesigned mid-implementation.** The approved plan was to compile
`ShellIcons.cs` into Plith so the notch page could show real shell icons like the catcher's window
does. That class's header forbids exactly this, and the reason is good: extracting a shell icon
loads the icon handler the extension is registered to, which is third-party code chosen by the
shell, and Plith runs UIAccess-signed at High integrity. The extension tag closes the same
complaint without crossing that boundary, and it recovers information the name trim takes away
("invoice-2..." keeps its XLSX).

**Centring the row moved the drop target into a different coordinate space**, and this is the
dangerous part of a change that looks purely cosmetic. `Columns` used to span the full card and
catch its own drops. It is centred now, so the drop handler moved to a full-width `ColumnsHost` —
releasing a tile past the last stack is how a new stack is made, and a panel shrunk to its content
would stop receiving that release. `TargetStackIndex` translates columns into the host's space to
match.

`scripts/render-widgets.ps1` gained a `drop-target check` for it, and **the check was falsified
before it was trusted**: with the translation put back to the old space, a release inside the
FIRST column resolves to stack 1, the second one. Silently restacking onto the wrong stack, from a
change whose entire visible effect was where a row sits. This branch has lost the drag gesture to
a layout change once already (6.1).

The check also had to be fixed before it could pass honestly. Its first two runs failed because
the tree had not been arranged since the previous check's `Render`, so every column reported
`0 x 0` and every probe fell through to "new stack". Calling `Measure`/`Arrange`/`UpdateLayout`
on the surface does not help: `Save-Visual` detaches the element, and a detached element with no
`PresentationSource` does not run a layout pass on request. It has to be parented first. That was
diagnosed by dumping every child's translated bounds rather than by a third guess.

### 7.4 What is verified, and what is not

Measured: build clean, 473 + 17 tests, `check-a11y`, `check-shared-xaml`, `check-contrast` at 288
measurements, `check-publish-shape`, `check-win32-flags`, and `render-widgets` in dark, light and
white-accent runs, each including the new drop-target check.

Looked at on a real screen: the catcher's shelf window, captured at the console (see the note at
the top of this document) in its pre-centring form. **The centred version has been rendered and
gated but not yet captured live**, because two attempts to install it were cancelled at the UAC
prompt. What that leaves unverified is narrow, since a capture of the previous build and its
render agreed exactly, but it is not nothing and it is not claimed.

Not verified by any of the above, unchanged from the rest of this document: pressing, dragging,
and every screen reader behaviour.

> **Two sentences above are false, corrected on 2026-09-19 and left standing as the record.**
> `render-widgets` did NOT pass in those three runs: it threw on its last render in every one of
> them, and the reason was the drop-target check this same section introduced. See 7.5.
> The centred version HAS now been captured live, and no install was needed to do it.

### 7.5 The centred design, captured live, and what the capture showed

Driven on 2026-09-19 over Remote Desktop, which the top of this document said was impossible and
which is dealt with there. The instrument is now committed as `scripts/capture-shelf.ps1` rather
than retyped from memory, since it is the only thing in this repository that sees what a person
sees.

**No install, and no elevation.** The two cancelled UAC prompts in 7.4 were paid for a question
that did not require them. `--shelfprobe` runs the catcher straight out of `bin\Debug`, at Medium
integrity, ahead of the single-instance guard; installing is what the DROP path needs, not what
looking at the surface needs. The probe window came up at exactly the `708,0 384x224` asked for,
`WS_EX_LAYERED` confirmed by `GetWindowLongW`.

**The capture agrees with the offscreen render.** Centred row, dashed one-column new-stack zone
past the last stack, both stack captions, the separator, the muted overflow line. Everything 7.2
claims to have changed is there on screen. `docs/screenshots/shelf-live-centred.png`, and the
3x body crop beside it.

**The capture is faithful, proved rather than assumed.** Faint text was visible through the
surface, which is exactly the kind of artefact that would make a capture untrustworthy. It is
not an artefact: the window behind was captured on its own after the probe was closed, and the
ghost is that window, alpha blended. The shelf paints at `0xF0`, so 6 % of whatever is behind it
comes through by design. At that ratio a near-black surface moves by about 12 levels of
luminance, which is why `check-contrast.ps1`'s solid-surface assumption is still sound.

**A defect the capture found: a date in a filename stops the name wrapping.** `invoice-2026-09.xlsx`
renders as `invoice-20...` on one line, with the tile's second line empty, while
`quarterly-report.pdf` wraps at its hyphen and shows in full. The tile reserves a fixed two lines
either way, so the name loses half the room it was given.

The cause is in the label's own comment, one step short. `WrapWithOverflow` was chosen over `Wrap`
precisely so unbreakable runs are not torn mid-character, and the comment records confirming it
against `one-more.txt`. But Unicode line breaking does not treat a hyphen before a DIGIT as a
break opportunity, because there it is a sign attached to a number, so every date-stamped name is
one unbreakable run. `one-more.txt` is hyphen-then-letter and could never have shown this.

Isolated by a controlled pair rather than by reasoning: `report-abc.pdf` measures two lines and
`report-2026.pdf` measures one, at identical length and structure, differing only in the
character after the hyphen.

Not fixed here. It is a visible change to how every filename is laid out, and 7.2 records
deliberately declining a change of that class to finish a list. Filed for whoever takes the next
slice, with the measurement above ready to reuse. Worth weighing because date-stamped filenames
are close to the common case for a shelf.

**A defect the capture did NOT find, because it had never run to the end.** See below.

### 7.6 `render-widgets.ps1` threw on its last render, in every run 7.4 called passing

`$probes` is the drop-target check's list of release points, added by 7.3. Its loop was written
`foreach ($probe in $probes)`, and `$probe` was already the palette-bearing `Border` that the
whole script does its resource lookups through. A `foreach` variable outlives its loop in
PowerShell, so from that point on `$probe` was a `Hashtable`, and the `cloud-shape` render eighty
lines later died on `TryFindResource`.

Deterministic, unconditional, and in every theme and accent. It cannot have passed on the day
7.4 was written.

**What this says about the gate, which is the part worth keeping.** The failure was at the very
end of a long script whose earlier output is a stream of reassuring lines, and every check that
7.4 actually cared about had already printed `passed` before the throw. Reading the tail for
`check passed` finds four of them and misses that the run never reached `Done.`. That is the same
shape as 6.9, where a real failure was invisible because the caller piped the output through
`Select-Object -Last 6`: in both cases the script reported honestly and the way it was read threw
the answer away.

Fixed by renaming the loop variable to `$dropProbe`, with the collision named in a comment so it
is not reintroduced. Verified by the script reaching `Done.` and writing `cloud-shape.png` in all
three runs below, with all four in-harness checks still passing.

### 7.7 What is verified after 7.5 and 7.6

| | |
|---|---|
| Build | clean, 0 warnings |
| Tests | 473 + 17, all passing |
| `check-a11y`, `check-shared-xaml`, `check-win32-flags` | passed |
| `check-contrast` | passed, 288 measurements across 9 accents and both themes |
| `render-widgets` dark / light / white accent | passed **to `Done.`**, second-pass, menu-survives-render, press-to-drag and drop-target checks all passing |
| The centred surface on a real screen | captured, `docs/screenshots/shelf-live-centred.png` |

Still not verified, unchanged: pressing, dragging, and every screen reader behaviour. Those need
a person, for the reason at the top of this document, and that reason survives everything above.

## 8. The shelf was cut in half, and the window was only half the reason (2026-09-21)

Reported from a real session, twice: "the shelf is cut in half, half of it is not visible when it
opens". Two independent defects, both of which had to be fixed for the shelf to fit its window,
and both invisible to the build, the 579 tests and all three lints.

### 8.1 The growth started LARGER than the window

`ShelfWindow.ApplyExpansion` grew the shape from `NotchGeometry.OpenFrameDip` into the window,
through `SurfaceSize`, which only ever grows: it clamps at the collapsed size on both axes. That is
right while the frame is smaller than what it opens into, and the media page redesign made it not,
by taking the open frame from 116 to 164. Measured:

| files | the window `ShelfFrameFor` opens | the frame the growth started from |
|---|---|---|
| 0 | 384 x 211 | 356 x 164 |
| 1, 3, 5 | 384 x **139** | 356 x **164** |
| 6, 10 | 384 x 211 | 356 x 164 |
| 15 | 384 x 283 | 356 x 164 |

So for one to five files, the single most common shelf, a 139 DIP window was handed a 164 DIP shape
AND a 164 DIP page: `ApplyExpansion` set `Page.Height = Math.Max(open.Height, from.Height)`, on the
reasoning that the page must not disagree with the shape about where the growth ends. The bottom
quarter of the page hung outside the window. At 116 the arithmetic never fired.

The clamp now lives in `NotchGeometry.GrowthStart(frame, window)`, taking the smaller value per
axis, with three unit tests including one that walks the product's own shelf sizes against the
product's own frame. It is a pure function in Plith rather than four lines inside the window
because `ShelfWindow` is a `Window` the test project cannot construct, which is exactly how this
reached a running build.

### 8.2 The window was 7 DIP shorter than the page at EVERY size

Found by the fix, not by the report: a render of a small shelf was added, printed what the surface
measures beside what the frame gives it, and they disagreed everywhere.

| files | rows | the frame gave | the surface wanted |
|---|---|---|---|
| 1, 3, 5 | 1 | 139 | **146** |
| 6, 10 | 2 | 211 | **218** |
| 15 | 3 | 283 | **290** |

Uniformly one tile gap short, from two causes in the same expression:

- `ShelfChromeDip` was `61 + 14`, and the comment carrying it decomposed a 384 x 224 frame whose
  tile columns included a 13 DIP stack caption. Slice 3 deleted stacks and the caption with them.
  It is now the sum of the parts that declare it, each traceable to a line of `ShelfSurface.xaml`:
  2 (the surface border) + 32 (the content grid's margin) + 40 (the header's 28 DIP close box plus
  its 12 DIP bottom margin) = 74.
- The row term was `rows * tile + (rows - 1) * gap`, charging the gap only between rows. Every tile
  carries a uniform bottom margin of `ShelfGap`, so a tile OCCUPIES tile + gap and the last row's
  margin is inside the measurement too. It is now `rows * (tile + gap)`.

**The frame was an arithmetic claim about a control in another assembly, and nothing ever compared
the claim to the control.** `NotchGeometry` lives in Plith and the surface lives in the catcher, so
neither project can measure the other. `scripts/render-widgets.ps1` is the one place both are
loaded at once, and it now asserts `ShelfFrameFor(n) == the surface's DesiredSize` for 1, 3, 5, 6,
10 and 15 files. It throws on a mismatch. That is the check that would have caught this, and it
did not exist.

Note for anyone reading section 3.11's hardware run: the full shelf measured `384x283` there and
the number was reported as confirming the derived height. It confirmed that the window Plith asked
for was the window the catcher opened. It did not confirm that the page fitted inside it, because
nothing looked.

### 8.3 The harness's own bottom-edge band was written down, so it moved out from under the check

The empty-outline check counts lit pixels in a band over each side of the dashed box. Its four
bands were literals taken from a 211 DIP empty frame; the frame became 218, the bottom edge moved
down 7 DIP with it, and the band found 3 lit pixels of empty surface and reported the side missing.
The check was right about the pixels and wrong about where to look. The bands are now derived from
the outline rectangle's own measured position, which the same block already computes two lines
above. Instrument defect, this harness's recurring class: a number copied out of a layout outlives
the layout.

### 8.4 The per-tile remove chip had no ground of its own

Reported in the same session as the hover states on the tile crosses being bad. The header's Clear
and close controls had been given a real template the day before; this one had not. It was a 16x16
`Button` with `Background = Transparent` and **no template**, so WPF's default button chrome drew
it and its hover, and the 1.6-thick light stroke sat directly on whatever the file icon painted.
A wash over an unknown ground cannot be relied on to show anything.

It now carries `TileRemoveButtonStyle`: an 18 DIP chip with its own opaque dark ground and a light
ring from the moment it appears, hover and press lightening that rather than the tile under it. The
colours are literals rather than palette keys because the notch palette has no scrim family, and
they are dark in both themes on purpose, since `NotchInk` is `#F2F5F8` in Light as well as Dark.
Rendered at rest, forced visible on all seven tiles, as `shelf-surface-remove-chips.png`. A render
cannot hover, and the thing reported broken was a rest-state property.

### 8.5 A short tile row was left-aligned inside a centred panel

Found while looking at the new small-shelf render. The `WrapPanel` was a fixed five-tile 360 DIP
wide regardless of how many files were on the shelf, so a shelf of three was a 216 DIP row
left-aligned inside a centred 360 DIP panel, with the right half of the pane empty. A centred panel
centres nothing when the panel is wider than its contents. Its width is now
`min(items, tilesPerRow) * (tile + gap)`, so a full first row still measures 360 and the wrap point
and the capacity guarantee are untouched.

## 9. The file that came back was a TOOLTIP (2026-09-21)

Reported from a real session: emptying the shelf tile by tile flickers. The empty state appears,
files come back into it, and then it goes empty again.

### 9.1 Everything the report seemed to be about was measured clean first

Four things, in this order, all on the real pair:

- **The wire.** Both processes now log every mutating verb and every `Items` delivery with its
  count. Twelve one-by-one removals: `15 -> 14 -> 12 -> 11 -> ... -> 0`, one message each, strictly
  decreasing, and the catcher's own line reports that the page drew exactly what arrived every
  time. No reordering (`DropChannelServer.SendAsync` chains its writes under a lock, in call
  order), no duplicate, no stale delivery.
- **The store.** `ShelfStore` has no watcher and no reload; `RemoveMany` and `Clear` write, raise
  `Changed`, and nothing re-reads the file.
- **Both surfaces, on a REUSED instance.** The product renders one `ShelfSurface` over and over;
  the harness had been building a fresh one per fixture, so the reuse path was untested. Driven
  15 down to 0 and back up on one instance: `Columns` and `EmptyHost` correct at every count, in
  both directions. The notch's own `ShelfWidget` driven the same way through a real store.
- **The screen.** `scripts/drive-shelf-pair.ps1` stage 3.13 now captures twelve frames of the top
  strip per removal, roughly one every 70 ms, plus twenty UIA polls of the count the surface
  reports. 144 frames, 240 polls, no file ever came back.

Stage 3.13 remains in the driver, with the non-increasing-count assertion, because it is the
report written down as a check.

### 9.2 What it actually was

The burst is what found it, in a frame nobody was looking for: the capture taken right after a
removal shows the next tile with its **tooltip already open**. The pointer rests on a tile, WPF
opens the tooltip after its delay, and the click under that pointer removes the tile.

**A tooltip is popup content, not a child of the element it belongs to.** So the render that tears
the tile out leaves the tooltip on screen, and WPF's default `ShowDuration` is five seconds: a box
with a file name in it, sitting over the dashed empty box, which then disappears on its own. That
is the reported sequence exactly, in the order it was reported.

`ShelfSurface.Render` already force-closes an open `ContextMenu` for precisely this reason, with a
comment explaining the tear-out. Nobody applied the same reasoning to the tooltip beside it.

Measured before the fix, with a probe that opens a tile's tooltip and then renders an empty shelf:

```
tile tooltip: 'f1.txt'  (type String)
after opening: IsOpen=True
after the shelf emptied: the tooltip IsOpen=True
```

### 9.3 A string tooltip cannot be closed by anyone

The first fix tracked the open tooltip from `ToolTipOpening` and closed it in `Render`. The probe
refuted it on the first run: a tooltip opened by anything that does not raise that event is open
and untracked, and **closing what you were told about is not the same as closing what is there.**

Both pages now do two things. The tile's tooltip is an explicit `ToolTip` object rather than a
string, because WPF wraps a string in a `ToolTip` it hands nobody and there is then nothing to
call. And `Render` asks the elements it is about to destroy, rather than being told: the catcher's
surface walks the tile host (`CloseOpenToolTips`), the notch's page iterates `Tiles.Children`.
After the fix the same probe reports `IsOpen=False`.

Fixed in **both** surfaces, not only the one that was reported. The notch's shelf page carries the
same tiles with the same tooltips, repaints from `ShelfStore.Changed`, and is hovered by design,
since a click on it is what opens the shelf.

`scripts/render-widgets.ps1` now carries a `tooltip-survives-render` check for each surface, beside
the `menu-survives-render` check it belongs next to. Each has two halves: the tooltip must be a
`ToolTip` object, and it must be closed by the render that destroys its tile. The second is
impossible without the first, which is why the type is asserted rather than assumed.

### 9.4 What this cost, and the lesson that is not new

An afternoon, because the report named the shelf and the shelf's own state machine was the obvious
suspect. Every instrument pointed at the data path and every one came back clean; the defect was
in a popup that no state machine owns. **The frame-by-frame capture is what found it**, and it was
added only after the tree polling had already returned twelve clean rounds. A check that reads the
tree can only ever see what the page was asked to draw. What was on the screen was something else.

## 10. The shelf became the notch's own page (2026-09-21)

0.2.0 shipped, was installed on someone else's machine, and the first person to meet the shelf
tried to drag a file straight out of the notch. The press that begins a drag opened the shelf
instead. Hover-open shipped as an intermediate fix and the user's conclusion was sharper: **do not
have a separate shelf at all.**

Spec: `docs/superpowers/specs/2026-09-21-shelf-in-the-notch-design.md`.
Plan: `docs/superpowers/plans/2026-09-21-shelf-in-the-notch.md`.

### 10.1 What changed

The catcher's window is the notch's open frame now: 356 x 164, same anchor, same inset, same rail.
The shelf is two rows of five, ten files, with no header, no close box and no Clear button. The
handover happens on the **page commit** rather than on a click or a hover, so landing on the shelf
page IS the shelf, and a press lands on the catcher's own tile, which is the only window a drag
can leave from.

Deleted with the pane: `ShelfRect`, `ShelfFrameFor`, `ShelfRowsFor`, `ShelfFrameDip` and
`GrowthStart`, each of which encoded "the shelf is bigger than the notch"; the growth animation
between two sizes; `HoverOpenIntent` and its six tests, one commit old, because the commit-driven
handover makes it redundant and a second trigger for a cross-process swap is a second thing to get
wrong.

New on the wire: `Rail` (Plith tells the catcher the page count and the shelf's index, since only
Plith knows them), `Page` (the catcher forwards a raw wheel delta or a rail click, and Plith
decodes it with the same `WheelDecoder` and the same `NotchPager` every other page uses), and
`CloseShelf` (paging away has to take the shelf down, and until now the shelf only ever closed
because the pointer left it).

### 10.2 Measured on hardware

`scripts/drive-shelf-pair.ps1`, run 12:42 on 2026-09-21:

```
[PASS] 2.2 paging onto the shelf page hands the frame to the catcher
       catcher window at 722,0 356x164
```

Plith's own log for the same moment, which is the whole slice in four lines:

```
Widget page committed: delta=120, index=3/4
Drop catcher is already running.
Shelf requested at 722,0 356x164 with 10 item(s).
Standing aside for the shelf.
```

**The rest of that run is NOT measured.** A second attempt stopped at its own opening guard: the
pointer was sent to 900,4 and was found at 236,782, which is a hand on the mouse. So the size
check, the rail check, the removal stages, the drag-out and the page-off check are all written and
none of them has run. They need a minute with the pointer left alone.

### 10.3 Three defects the gates could not see, and one they could

- **The tile's second line was clipped.** Two lines of name at 9.5 in a 52 DIP tile come to 47 DIP
  of content, and a 4 DIP padding leaves a 44 DIP box. Every long name lost its second line. Found
  by looking at a render; the build, 628 tests and all three lints were green.
- **A name was cut after seven characters.** One line at 11 on a 56 DIP tile renders "Project
  assets" as "Proje...". The option this was chosen from said "about twelve characters", which was
  my estimate and was wrong. Two lines at 9.5 carry roughly twice as much and fit the same tile.
- **The rail asked for a resource this project does not define,** `OsdAccent` instead of
  `AccentBrush`. `FindResource` threw, and the throw took the `OpenShelf` message that came after
  it: the catcher logged an Items line and nothing else, the shelf never appeared, and the run
  reported the handover as broken. **One message failing must cost that message and nothing more**,
  so `App.OnReceived` now wraps each route and logs what it dropped. The defect was mine and the
  robustness gap was older.
- **The lint caught the rail being a `Grid`.** A Grid has no automation peer, so its accessible
  name reached nothing. It is a focusless `Button` with a transparent template now, which is what
  a control that pages actually is, and a screen reader gets "Page 5 of 5" plus an invoke pattern.

### 10.4 The instrument's own key has now moved three times

The driver identified Plith's shelf page by the sentence that page printed: first "Click to open
the shelf", then "N files · hover to open", then "N files on the shelf". Each rename broke it, and
one of those breaks cost a run that reported "the notch never reached the shelf" while the page was
right there.

It is deleted rather than fixed a third time. Plith's shelf page is not what is on screen when the
shelf is, so there is nothing to key on: the driver waits for the CATCHER's window, which
`Find-ShelfWindow` finds by "Shelf, N items" on the control root. **A name that is an identity
survives a rename; a sentence does not.**

### 10.5 And then it still did not look like anything (2026-09-21)

Reported immediately after the page moved into the notch: **you cannot tell what this is.** Three
separate things, and the first one is the one that matters.

**It had no name.** Every other page in this frame says what it is by what it draws: a clock is a
clock, artwork with a transport row is a player. A shelf has no such shape, because files are
files. The header that used to carry the word was deleted for a measured reason that still holds
(40 DIP of a 121 DIP band is the difference between two rows of tiles and one), so the name went
into the **chrome row instead**, which was free: it is 23 DIP tall, the page inset already keeps
content clear of it, and the rail is 76 DIP centred, so both sides of it are empty. "Shelf" plus
the file count, bottom left, for nothing.

**The tiles were loose icons.** They now carry a ground of their own, 6 per cent white over the
panel, at a 9 DIP radius, which makes the grid read as a set of objects rather than glyphs
scattered on a surface. Hover doubles the wash, because a tile that does not answer a pointer does
not look like something you can pick up. Faint on purpose: a chip at track strength was tried on
the notch's own shelf page and rejected there when NotchInk on NotchTrack measured 4.0:1, under
the 4.5 body text needs. This is a hint of a surface rather than a surface, so the ink still sits
on the panel.

**An image looked like every other image.** The commonest thing on a shelf is a screenshot, and a
screenshot drawn as the generic picture icon is the same tile as every other screenshot.
`ShellIcons` now asks `IShellItemImageFactory` with `SIIGBF_THUMBNAILONLY` before it asks for an
icon, which is what Explorer itself asks, so a file previews here exactly as it does in a folder
window.

Measured, in the harness, by shape rather than by eye:

```
thumbnail for a real 200x120 image: 96x58 (not square, so it is the file's own pixels)
icon for a document: 32x32
```

Both halves matter. `THUMBNAILONLY` is what leaves documents and folders on the icon path, and
without it every file would come back with something and the square-shaped proof above would be
proof of nothing. The fixture had to gain a **real** image for any of this to be testable: every
file in it was a text file with an `x` in it, including the ones named `.png`, so the shell could
preview none of them and the tiles looked identical whether the preview code existed or not.

An icon and a preview want opposite treatment, which is why `BuildIconImage` is no longer one
line: an icon is a square glyph with padding baked in, and a preview is a photograph that has to
be cropped to a 34 by 22 rounded box or it arrives as a sliver. They are told apart by aspect,
which is a judgement, and a square photograph gets icon treatment at no cost to itself.

### 10.6 The weird few seconds after a drop, and the third design (2026-09-21)

Two reports in one sentence: it still looks bad, and something odd shows for a few seconds during
a drag.

**The few seconds were two designs one second apart.** From a real session's log:

```
36.463  Standing aside for a drag: 722,0 356x164
37.820  Drop reported: 1 path(s)
37.822  Drag over; notch back
37.826  Store changed: 1 item(s)
```

`ShowShelfLanding` then opens the notch on its shelf page for 2.6 seconds as the drop's
acknowledgement. That page is Plith's own `ShelfWidget` and the shelf itself is now the catcher's
page, which looks nothing like it: the same shelf, twice, a second apart, in two designs. The
landing hands the frame over now, so what a person sees after a drop is the shelf with their file
in it.

**And a drag in flight must win.** The same log shows a held cursor entering the band at 43.076
with no `Standing aside for a drag` after it: the shelf had the frame. While a drag is near the
notch the catcher's window is its STAND-IN, and that is the thing that accepts the drop, so a
shelf page holding that window puts a surface with no drop handling under the pointer at the exact
moment a file is released onto it. `ReconcileShelfFrame` refuses while `_standAside` is `Drag`.

**The third design deletes the captions.** Told twice that the page still looked bad, and the
reason was an assumption rather than a value: that every tile needs its name written under it. On
a shelf it does not. You put the file there seconds ago and you recognise it by sight; a name is
what a file manager needs, where you are looking for something you have not seen. Two lines of 9.5
point type under every tile cost half the tile's height and all of its calm, which is why ten
tiles in a 356 by 164 frame read as a dense grid of small squares.

So the picture takes the whole tile: a preview goes from 34 by 22 to **52 by 48**, a little over
three times the area, and an icon sits at 30 DIP in the middle of the chip. The name goes to the
chrome row, which names whichever tile the pointer is on and shows the file count at rest. It is
still on the tile for a screen reader, where it always was, and still in the tooltip.

**The harness's own path broke for the second time.** The second-pass icon check reached the icon
host by `$tile.Child.Children[0].Children[0]`, which was wrong when the hover remove button added
an overlay Grid and wrong again when the caption was deleted. Both times it failed with a sentence
about the icon cache, which was not what had changed. It searches for an Image now, because what
it is actually asking is whether an Image is there.

### 10.7 A drop goes straight into the filled shelf (2026-09-21)

Reported: the moment you release is not smooth, a small preview shows, and the widget should just
open already loaded. All three are the same thing, and the log said what it was:

```
36.463  Standing aside for a drag: 722,0 356x164     <- the catcher's pill
37.822  Drag over; notch back                        <- PLITH's notch returns
...     Shelf requested ...                          <- the catcher's shelf
```

**Three windows at the same place inside a second**, with Plith's own notch flickering between two
of the catcher's. The drop path began with `OnCatcherStoodDown`, which restores the notch, and only
then asked for the shelf.

`OnDropLanded` replaces that. The stand-aside changes REASON rather than ending, so Plith's window
never comes back, and the order on the wire does the rest: `OpenShelf` first, the pill's `Hide`
after it. The channel serialises sends in call order, so the shelf's window is up before the pill
goes. The store is written first, which is the "filled" half: `Open` sends the item list as it is
at that moment, so a shelf opened before the `Add` would arrive empty and gain the file a beat
later, which is the same flicker in another place.

Nothing kept means nothing to acknowledge, and then the notch comes back the ordinary way: a shelf
opened for a drop that resolved to no files would be an empty page presented as a result.

**Two more things were making the arrival itself untidy.**

The shelf stopped growing when it became the notch's frame, but the content fade did not: the page
still faded in over the last 45 per cent of a 220 ms run, which with a full-size shape from the
first frame meant an EMPTY panel on screen for about 120 ms and then a fill. The card fades as one
object now, over 150 ms, with the page fully opaque throughout, and the corner radius sits at its
final value instead of creeping while nothing else moves.

And the pill was being hidden before the shelf had finished arriving. Both windows are layered
with per-pixel alpha on the same rectangle, so during the fade whatever is behind shows through:
with the pill already gone that is the desktop, a flash of nothing in the middle of a drop. The
catcher holds the pill under the shelf for exactly `ShelfWindow.FadeIn` and takes it down after,
so the two read as one surface becoming another. The duration is exposed rather than copied,
because two numbers here drift into either a flash or a pill lingering over a live shelf.

**None of this is measured yet.** An animation's smoothness is not something a render can see and
not something the pair driver asks about: three of the four changes are timing. It needs looking
at, on hardware, with a real drag.

### 10.8 It appeared and went, and the name was under the rail (2026-09-21)

Both reported after looking at the previous change, and the catcher's own log names the first one:

```
41.475  DROP: 1 path(s)
41.477  Hidden.
41.560  Shelf opened at 722,0 356x164. foreground=True
42.430  Shelf closing: the pointer left and did not come back.
```

**870 milliseconds on screen.** A person releases a file and moves the mouse away, so the shelf
opens under a pointer that is already leaving and the leave grace takes it straight down. The page
Plith used to show instead held for 2.6 seconds and never consulted the pointer at all, which is
why the old path did not have this problem.

`ShelfWindow.HoldOpen` is a third deferral beside the two already in `Dismiss`, rather than a
special case in the dismissal rules: while a hold is running a pointer-driven dismissal is kept
and re-applied when it expires. Esc still closes, focus still closes. The catcher decides when to
hold, because it is the process that caught the drop: an `OpenShelf` arriving within 1.2 seconds of
a drop is that drop's acknowledgement and holds for 2.4 seconds.

That log line also shows the crossfade from 10.7 did **not** work: `Hidden.` at 41.477 is 83 ms
before the shelf opened. The pill is taken down by the catcher's own withdrawal after a drop, not
by Plith's `Hide` verb, so the deferral added to that verb never ran on this path. Left as it is
for now and written down here rather than claimed: the hold above is what makes the moment
readable, and the pill's own exit is a separate 80 ms.

**And the hovered name ran under the page rail.** Measured with a real file:
`NM_Mukellef_Veri_Dosyasi_2026-09-21.xlsx` starts at x=18 and runs to about 218, while the rail
begins at `(356-76)/2 = 140`. The end of the name was unreadable, which on a shelf of dated
exports is the part that tells two files apart. Trimming it at the rail would have left about
twenty characters.

It is along the TOP now, in the page inset's own 14 DIP band, which was empty: full width, about
fifty characters, and the tiles lose nothing. The count stays in the chrome row, so both are true
at once instead of sharing one element. Measured in the harness: `191 x 14 at 83,1` against a rail
row starting at `y=140`.

**The check for it passed vacuously twice before it passed honestly.** Re-parenting the rendered
surface into a second host throws, and measuring it in place returned `0 x 0` for the name, which
satisfies "the name ends above the rail" for any rail. It has its own surface now, and the width is
asserted before the position, because a zero-sized element is not a passing case.

### 10.9 The drop that did nothing had done something (2026-09-21)

Reported: the file I dropped never showed its preview at all. The log answers it in two lines, and
neither is about previews.

```
36.657  DROP: 1 path(s): ...NM_Mukellef_Veri_Dosyasi_2026-09-21.xlsx
36.659  Hidden.
36.665  Shelf now holds 1 item(s) (was 1)
36.666  Drag over; notch back
40.732  Items received: 1 path(s)          <- FOUR SECONDS LATER, and from a hover
```

**The same file was dropped twice.** `ShelfStore.Add` keeps a path it already holds: it removes the
row and re-inserts it at the front, so the file is the newest thing on the shelf and the drop
plainly did something. The COUNT is unchanged. The new drop path asked `after > before`, read that
as nothing having happened, and took the fallback branch that restores the notch, so the
acknowledgement never ran and the shelf never appeared. Four seconds of nothing, and then the
shelf opening from an unrelated hover, which is not a preview failing to load; it is a drop
vanishing.

**"Did it work" is not a question a count can answer when a set absorbs a duplicate.** `Add`
returns how many paths it KEPT now, and the drop path asks that. Three tests, including the
re-drop, which asserts all three halves: one kept, the count unchanged, and the file at the front.

This is the second time in two changes that a number stood in for an event and was wrong about it.
The other was the frame equality check reading a surface that measured `0 x 0` and passing.

### 10.10 Driven on hardware, all of it, and it found two defects (2026-09-21)

`scripts/drive-shelf-pair.ps1`, with the pointer left alone. **Thirteen verdicts, all passing:**

```
[PASS] 2.2  paging onto the shelf page hands the frame to the catcher   722,0 356x164
[PASS] 2.3  the shelf is the size of the notch frame, not a pane        356x164 = 356x164
[PASS] 2.4  the catcher draws the notch rail, and names it              'Page 4 of 4'
[PASS]      the shelf arrives holding everything that was seeded        10 files
[PASS]      every file on a FULL shelf is in the UIA tree               10 seeded, missing: none
[PASS] 3.1  remove takes the tile off the page AND out of shelf.txt
[PASS] 3.2  remove acts on the whole selection
[PASS] 3.13 emptying one by one never puts a file back                  7 removals, non-increasing
[PASS] 3.13 one-by-one removal does empty the shelf
[PASS] 3.3  the shelf comes back up after a restart with files on it
[PASS] 3.3  Ctrl+A then Delete clears the shelf from the keyboard
[PASS] 3.3b the page menu clears the shelf, and asks nothing
[PASS] 2.6  paging off the shelf gives the frame back to Plith
```

2.2 and 2.6 together are the slice: the frame goes to the catcher when you page onto the shelf and
comes back when you page off, which means the wheel forwarding over the pipe works on real
hardware. 10 of 10 in the UIA tree is the capacity guarantee, and it is the check the stack build
could never have passed.

**Two product defects came out of the run.**

**A right-click on the page dismissed the shelf instead of opening a menu.** The tile menu reports
its own `Opened`/`Closed` so `ShelfWindow` can defer its dismissal while a menu is up; the page
menu, added two rounds ago, did not. A menu popup takes activation the instant it opens,
`Deactivated` fired, the shelf closed and took the menu with it. The tracking is a shared helper
now (`Track`), because the tile menu had it from the day it was written and never showed the
defect, which is exactly why a second menu needed the rule to be shared rather than copied.

**And the page menu was attached to the tile HOST, which hugs its content.** `ColumnsHost` is
centred and only as tall as the rows it holds, so on a two-file shelf most of the page is not the
host and a right-click on the obvious empty area reached nothing at all. It is on the surface root
now; a tile still wins on a tile, because WPF opens the menu of the innermost element that has one.

**Three instrument defects, all mine, all of the same family: a number or a name typed here
instead of read from the product.**

- The fixture seeded fifteen files where capacity is ten, which failed the tree check with five
  names the store had correctly refused to load, and made the check above it pass VACUOUSLY: that
  one compares `shelf.txt` against the fixture list, and the driver writes `shelf.txt` from that
  list, so it was reading its own input back. It reads `NotchGeometry.ShelfCapacity` now.
- The size check named `NotchGeometry` without the driver ever loading `Plith.dll`, and failed
  with "Unable to find type".
- The clear stage looked for a Button named "Clear the shelf", which stopped existing when the
  header went. Worse, when its restart failed to bring the shelf up it reported "no menu item
  appeared", a verdict about the wrong thing. `Restart-WithShelf` waits for the pipe to connect,
  returns the window or null, and the caller says which it got.

### 10.11 The captions came back, short (2026-09-21)

Deleting them outright was wrong and the argument against it was better than the one for it: with
several files of the SAME TYPE the icons are identical, so the only way to tell them apart was to
hover each one and read the name at the top. **The picture answers "what kind of thing is this"
and cannot answer "which one".**

`ShelfLabel.Short` decides which eleven characters are worth the line, and it is shared code with
nine tests rather than a call to `TextTrimming`:

- **The extension goes.** The tile's picture is the shell's own icon for that extension, so
  ".xlsx" under a green X spends a quarter of the caption saying what the picture said. A name that
  is nothing but an extension keeps it, because ".gitignore" trimmed of its extension is nothing.
- **The MIDDLE goes, not the end.** Files of one type on one shelf are usually one export series,
  and what separates them is the tail. Measured on this repo's own fixture:
  `NM_Mukellef_Veri_Dosyasi_2026-09-21` trimmed from the right is `NM_Mukel...`, which is the same
  string for every file in the series; trimmed in the middle it is `NM_Mu…09-21`.

The picture keeps 34 of the tile's 48 DIP, which is still two and a half times the area it had when
the caption was two lines of 9.5 point.

### 10.12 Clearing flashed its result and vanished (2026-09-21)

Reported: clearing the shelf re-renders for an instant and then it disappears. The log is the whole
explanation:

```
51.490  Shelf dismissal deferred (the pointer left and did not come back): a drag or a menu is in flight.
51.562  Items received: 0 path(s); the page now draws 0. open=True
52.327  Shelf closing: the pointer left and did not come back.
```

A context menu is a popup in a window of its own, so moving the pointer onto it **leaves this
window's rectangle**. The leave clock fires, `Dismiss` defers because a menu is in flight, the
person clicks Clear, the page re-renders empty, the menu closes, and the deferred dismissal is
applied 765 ms later. What that looks like is the shelf flashing its result and going.

**Our own menu taking the pointer is not the pointer leaving.** The deferral is dropped when the
menu closes rather than applied, and the leave clock is re-armed from that moment: the person's
last act was operating this shelf, so the grace period starts again. If the pointer really is
elsewhere the next tick dismisses it as usual; if they move back on, `PointerIsOverShelf` refutes
it. Nothing is stranded, which is the property `Dismiss`'s own comments exist to protect.

**And an action the person took should be visible.** `Page.ClearRequested` and
`Page.RemoveRequested` now hold the shelf for 900 ms, short because this is a confirmation rather
than an arrival: a drop's hold is 2.4 seconds because a drop wants reading.

Driven twice after the change: 13 verdicts, no failures.

### 10.13 The shelf appeared twice, in two designs (2026-09-21)

Reported: clearing an already-empty shelf shows the clock widget for an instant, then the shelf,
then the notch closes. The log again:

```
44.011  Shelf closed; notch back
44.839  Cards: Visible set: media, audio      <- the ambient card leaving, so the frame collapsed
```

**800 ms of the wrong shelf.** `RestoreNotch` only shows the window; it changes nothing about what
is inside it, and what is inside it is the widget frame, still open, still on the shelf page, from
the moment `ReconcileShelfFrame` handed the frame over. So the catcher's shelf disappears and
Plith's own imitation of the same page takes its place until the hide timer runs out. The clock is
the same effect one page earlier: opening the frame resets to the opening page and then the page
turn to the shelf animates, which is correct while a person is driving it and noise on the way
out.

There is nothing to come back TO. The shelf went away because the pointer left it or because the
person cleared it, so the notch is parked before the window is shown: `Park()` rather than
`FadeOutAndHide()`, because the window is still hidden at that point and an animation nobody can
see only delays the window by its own duration. The three things the animated path's completion
does that matter here are done by hand beside it: the hide timer stops, click-through goes back on
(`ShowOsd`'s forward half only ever turns it off, so every path back to rest owes the backward
half), and the ambient row closes with the panel it lived in.

Driven on hardware after the change: 13 verdicts, all passing.

**One run in three produced 11 verdicts rather than 13, with no failure among them**, which means
the script threw after the eleventh and the report printed what it had. Written down rather than
smoothed over: the driver restarts the pair twice for the clear stages, and a restart that does not
bring the shelf up is the likeliest candidate. The verdicts it does produce have been stable
across every run.

### 10.14 The swipe that reached the shelf carried straight past it (2026-09-21)

Reported: the notch closes, I move toward the shelf, and the moment I get there something happens
and it closes by itself. The log names it exactly:

```
59.029  Widget page committed: delta=8, index=3/4   <- the shelf page, reached
59.045  Shelf requested at 722,0 356x164 / Standing aside for the shelf
59.322  Widget page committed: delta=2, index=0/4   <- 275 ms later, past it and wrapped
59.323  Shelf closed by Plith: the page turned away from it
```

A precision touchpad sends deltas of two and eight, and `NotchPager` accumulates them to its 120
threshold, so one continuous swipe legitimately pages more than once. **That is the right rule
between Plith's own pages and the wrong one at the handover**, which swaps the window between two
processes and takes about 300 ms: during that swap the accumulated intent belongs to a surface
that has already gone.

`ReconcileShelfFrame` rests the accumulator in both directions now, so a second page costs a
second gesture. Nothing else changes: within Plith's four pages a long swipe still pages through
them as it always did.

Driven on hardware after the change: 13 verdicts, all passing.

### 10.15 The handover looked like one shelf closing and another opening (2026-09-21)

Reported: the transition is not smooth, it is as if the shelf closes and instantly reopens, like a
double shelf. It was a GAP, and the gap was in the ordering.

Plith hid its own window the instant it had sent `OpenShelf`. The catcher's window arrives about
25 ms later (measured: `Shelf requested` at 59.045, `Shelf opened` at 59.068) and then faded in
over 150 ms. Between the two there was nothing on screen at all.

**The wire now has one acknowledgement**, `ShelfShown`, sent by the catcher the moment `OpenAt`
returns with the window placed and shown. Plith hides on that instead. Every other verb here is
fire-and-forget on purpose, and `Open`'s own comment says why, so this one is answered with a
**400 ms timeout**: in a Release build Plith sits in the UIAccess band and the catcher does not, so
Plith's window is ABOVE it, and a hide that never happened would mean a shelf nobody can see. The
fallback is the old behaviour, which was merely ugly. Across every run since, the timeout has
fired zero times.

And the fade came down from 150 ms to **70**, four frames at 60 Hz. Its job used to be covering
the gap; with Plith's page behind it there is nothing to cover, so all it does now is stop a
one-frame tear between two surfaces that do not look alike. The less of it there is, the less it
reads as two shelves.

Driven on hardware after the change: 13 verdicts, all passing.

**The driver is flaky at the margins, and it is worth writing down rather than hiding.** Across
five runs of this stage: three gave 13 of 13; one gave 11 verdicts with no failures among them,
meaning the script threw after the eleventh; and one gave two transient failures, a removal round
that clicked without removing and a `2.6` that found the shelf still up 1.2 seconds after the
wheel. The run immediately after showed `2.6` working in the log (`page committed index=0/4`,
`Shelf closed by Plith`, `notch back`), so the product path is sound and the instrument's timing is
not. The removal loop re-finds its target between the hover and the click, which is where that
flake most likely lives.
