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
   the surface on the way to a tile at the edge.
9. **It does not appear in Alt+Tab** while it is open, and it does not steal the taskbar.

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
| `Closed` fires exactly once per dismissal | the probe's subscriber logged one exit line, not two, even though `Hide()` raises `Deactivated` a millisecond after an `Esc` had already closed it |

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

The fix belongs on Plith's side and is not written yet: **Plith must call
`AllowSetForegroundWindow(catcherProcessId)` before it sends `OpenShelf`**, which is the
documented way one process hands its foreground privilege to another. Until that exists, the line
to check in the log is `foreground=` on every `Shelf opened` entry.

It is also genuinely unknown whether the real gesture hits this at all: the shelf opens in
response to a physical click, and user input relaxes the foreground lock in ways a scripted
`Start-Process` does not. That is a thing to measure at the console, not to assume in either
direction.
