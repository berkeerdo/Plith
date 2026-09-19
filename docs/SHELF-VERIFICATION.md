# Shelf verification

What has actually been looked at on a running build, and what has not.

The shelf lives in a **layered window with per-pixel alpha**, in a second process. That is a
deliberate choice (it is the only way the shape can grow out of the notch rather than cut into
place) and it carries a price that this document exists to keep honest: **a layered window cannot
be captured over Remote Desktop by any means.** No screenshot tool, no `PrintWindow`, no render
harness reaches it. So every visual claim about the shelf is either something a person looked at
at the physical console, or it is not a claim at all.

Session type is read from `qwinsta`, not from `$env:SESSIONNAME`. The environment variable is
stamped at process start and was wrong on this machine once already.

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

### Status: NOT YET RUN

**None of the nine items above has been looked at.** The session that built this is
`rdp-tcp#0` (`qwinsta`, 2026-09-19), a Remote Desktop session, where the window in question cannot
be captured or judged. Nothing in this section may be reported as passing until a person has run
the command above at the physical console.

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
works, because the return value turned out to say nothing. See §2.7, where it returned `True`
from a process that plainly did not hold the foreground. The line to check in the log is still
`foreground=` on every `Shelf opened` entry; it is now paired with a grant line in `plith.log`.

It is also genuinely unknown whether the real gesture hits this at all: the shelf opens in
response to a physical click, and user input relaxes the foreground lock in ways a scripted
`Start-Process` does not. That is a thing to measure at the console, not to assume in either
direction.

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
§2.7.

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

### 2.7 The foreground hand-over

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

### 2.8 Shelf actions come back and are answered

Not reachable until Task 7 wires the catcher's `EntryPressed`, `ClearRequested` and
`NewStackRequested` to the wire. Listed here so it is not mistaken for something Task 6 covered:
`ShelfSession.HandleMessage` routes `RemoveItems`, `ClearShelf`, `NewStack` and `Restack` to
`ShelfStore` and re-sends the whole shelf after each, but nothing on the catcher's side sends any
of them yet.

### What HAS been measured, on 2026-09-19

| Measured | Result |
|---|---|
| The catcher still starts at Medium when Explorer launches it, with this build | `Started. Integrity: MEDIUM` |
| `AllowSetForegroundWindow` against the real catcher pid, from a process not holding the foreground | returned `True` |
| `ForegroundLockTimeout` on this machine | `150000`, so the lock is enabled |
| Overlapping sends on one pipe cannot interleave | `DropChannelServerTests.Server_DoesNotInterleaveOverlappingSends`, 20 messages queued unawaited, all 20 lines decode in order |
| The same test fails without the fix | one line came back with 96 fields instead of 5, and DECODED as a plausible `Items` message |
| The shelf page's three states, rendered offscreen at frame size, dark and light | `scripts/render-widgets.ps1`: `widget-shelf`, `widget-shelf-empty`, `widget-shelf-unavailable` |

The render harness reaches the notch's shelf PAGE because it is an ordinary WPF control rendered
to a bitmap. It does not reach the shelf SURFACE in place: `shelf-surface.png` is that control
rendered offscreen too, not the window on screen. Nothing in this document's §1 or §2 visual items
is answered by a render.
