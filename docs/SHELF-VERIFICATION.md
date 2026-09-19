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
works, because the return value turned out to say nothing. See §2.8, where it returned `True`
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

### Status: NOT YET RUN

**And §3.5 and §3.6 could not have passed before 2026-09-19 regardless of who ran them.** The
whole-branch review found the tile drag could never start at all (see §6.1), so the two items
that restack by dragging were describing a gesture the product did not have. They are unchanged
here and still NOT YET RUN; what changed is that running them is now capable of the result they
ask for.

Same constraint as §1 and §2: the shelf is a layered window in a second process and cannot be
captured over Remote Desktop, so none of the items below has been looked at. What has been
measured without a console is listed at the end of this section, and it is not a substitute for
any of these: build, tests and lint all stayed green through every defect §1 and §2 found on
hardware, and would have stayed green through these too.

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

### 3.4 New stack, then a drag lands in it

Click the header's plus control. Expected: an additional, empty stack appears. Drag a tile from
another stack onto the new one. Expected: the tile moves, `shelf.txt` reflects the new grouping,
and the tile's origin stack no longer lists it.

### 3.5 Dragging a tile onto another existing stack restacks it

With at least two non-empty stacks, press a tile, drag it onto a different stack, release.
Expected: the tile joins the target stack and leaves its old one, on screen and in `shelf.txt`.
Try this once with a single tile and once with a multi-selection (Ctrl+click two tiles first,
then drag one of the selected ones): the whole selection should move together, again by
`ShelfModel.DragPaths`.

### 3.6 Dragging past the last stack starts a new one

Drag a tile into the empty area to the right of the last visible stack (not onto any column).
Expected: the same result as making a new stack by hand and dragging into it (§3.4), reached by
one gesture instead of two.

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

### What HAS been measured, on 2026-09-19

| Measured | Result |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors |
| Tests | `dotnet test Plith.slnx -m:1`: 472 + 17 passed, 0 failed |
| `scripts/check-a11y.ps1` | passed: every interactive control has an accessible name, no icon-font use |
| `scripts/check-shared-xaml.ps1` | passed |
| `scripts/check-contrast.ps1 -STA` | passed: 234 measurements across 9 accents and both themes |
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

### 4.5 The restack still works, through the same call

Task 7's internal drag is now the same `DoDragDrop` with an extra format on it, so §3.4, §3.5 and
§3.6 have to be re-run rather than assumed to still hold.

Expected, unchanged from those items: a tile dragged onto another stack joins it, a tile dragged
past the last stack starts a new one, and `shelf.txt` reflects both. The cursor now shows a COPY
badge during a restack rather than a move badge, which is deliberate (Move is not offered at all,
by anyone, for the reason in §4.1) and is worth a line in the result either way: whether it reads
as confusing on screen is a judgement only a person at the console can make.

### 4.6 The shelf does not close under a restack

Drag a tile onto another stack, release, and then **do not move the mouse**.

Expected: the shelf stays up. This is the one behaviour in this task that is reasoned rather than
measured. Any drag can take the pointer off the window as far as WPF is concerned, which arms the
leave timer, which fires during the drag and defers a dismissal; `StartDrag` therefore settles
that deferral when the drag ends, asking `WindowFromPoint` where the pointer actually is rather
than asking WPF, whose answer can be stale until the next mouse message arrives. If the shelf
closes about half a second after a restack with the pointer sitting on it, that re-evaluation is
wrong and this item is what found it.

Then the opposite: drag a tile OUT to another application and release there, again without moving
the mouse afterwards. Expected: the shelf goes away within the leave grace period (500 ms), and
the notch comes back.

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

### 4.9 Did the shelf keep its activation across a restack

Drag a tile onto another stack, release, do not move the mouse (this is §4.6's setup), and then
press `Esc`. **Both outcomes are results. Record which one happened rather than looking for a
pass.**

- **The shelf closes and the notch comes back.** The window still had activation when the drag
  ended, so `Esc` reached it. Nothing further to check.
- **`Esc` does nothing.** The window lost activation during the drag and never got it back, which
  is a real and expected possibility: `Esc` needs keyboard focus, and a window that is not
  foreground has none. That is not a bug by itself. What matters next is whether anything is left
  that CAN close it, so move the pointer off the shelf and leave it off. Expected: the shelf goes
  down within the leave grace period (500 ms) and the notch comes back.

Why this is its own item rather than part of §4.6: when a drag ends with the pointer over the
shelf, `StartDrag` clears `_pendingDismissal` and stops the leave clock. `Deactivated` cannot fire
a second time on a window that is already deactivated, so after that the ONLY closer guaranteed to
still exist is the pointer leaving. This item finds out whether `Esc` is a second one, which
depends on something the log does not record: whether the drag left the window foreground. A shelf
that answers neither `Esc` nor the pointer leaving is stranded, and that is the failure the whole
deferral machinery exists to prevent, arriving by yet another route.

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
| `scripts/check-contrast.ps1 -STA` | passed, 234 measurements across 9 accents and both themes |
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
| `scripts/check-contrast.ps1 -STA` | passed: 288 measurements across 9 accents and both themes (was 234 before this task: +36 from the two new `ShelfSurface.xaml.cs` pairs, +18 from the selection ring) |
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

### What HAS been measured, on 2026-09-19, after these fixes

| Measured | Result |
|---|---|
| Build | `dotnet build Plith.slnx -m:1`: succeeded, 0 errors |
| Tests | `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`: 473 passed, 0 failed (was 472: +1 for the stack cap) |
| `scripts/check-a11y.ps1` | passed, with six known gaps as notices; both new holes proven closed by temporary probes (6.4) |
| `scripts/check-shared-xaml.ps1` | passed: 3 files compiled into both Plith and the installer, none naming an assembly |
| `scripts/check-contrast.ps1` | passed: 288 measurements across 9 accents and both themes |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#A3E635'` | passed, including the new press-to-drag check |
| `scripts/render-widgets.ps1 -Theme Light -Accent '#A3E635'` | passed, same |
| `scripts/render-widgets.ps1 -Theme Dark -Accent '#FFFFFF'` | passed, same |
| Session type | `qwinsta`: `rdp-tcp#0`, active. Not the console, so nothing in sections 1 to 5 was driven |

Every NOT YET RUN in sections 1 to 5 is still NOT YET RUN. Three of the four defects above lived
in code that a green build, a green suite and a green lint had all passed over, which is this
document's recurring finding rather than a new one: 6.1 is the only one of the four that now has a
gate standing over it.
