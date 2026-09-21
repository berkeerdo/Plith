# Shelf Slice 2: A Catcher-Owned Shelf Surface

**Date:** 2026-09-18
**Branch it builds on:** `feature/shelf-drop-catcher` (slice 1, Tasks 1 to 8)
**Spec this refines:** `docs/ROADMAP.md`, Phase 7, the Shelf card entry

## Why this slice has the shape it has

Slice 1 delivered the inbound half: a file dropped on the notch lands on a shelf page.
It got there through a second process, `Plith.DropCatcher`, at Medium integrity, because
Plith runs at High integrity for UIAccess and UIPI refuses Explorer's cross-integrity drag.

Slice 1 then measured the outbound half twice, and both measurements came back negative.

1. **Task 7.** The same binary returns `None` from High with nothing copied, and
   `Copy, Move` from Medium with the file landing. Integrity is the cause. So Plith can
   never be the drag source, and the catcher has to be.
2. **Task 8.** The catcher cannot start a drag for a press that landed on Plith's window.
   Three runs at Medium, press verified by `WindowFromPoint` to belong to another process,
   released over a window built to accept files and log what it received: no `DragEnter`
   ever arrived, and `DoDragDrop` returned `None`, once only after not returning at all for
   seventeen seconds.

Task 8 is what dictates this design. The stand-aside that carries a drop cannot carry a
drag out, because the gesture starts before the hand-over. The only shape left is the one
Task 7 already proved works: a press on the catcher's **own** window. So the shelf's
interactive surface moves into the catcher process.

That ordering is also why this slice does the surface before the actions. Clear, remove,
open, stacks and real icons all live on whichever window the person actually touches. Built
into Plith's `ShelfWidget` first, they would be built again after the move.

## Goal

A person can open the shelf from the notch, see what is on it with real file icons, select
one or several items, drag them out to any ordinary application, remove single rows, clear
the shelf, and keep items in separate stacks that do not mix.

## Inherited constraints

These come from the repo and from slice 1. They are not up for renegotiation in this slice.

- All code, comments, documentation and commit messages in English. Conventional Commits.
  No AI attribution anywhere.
- `Plith.exe` keeps `uiAccess="true"`. That is what lets the OSD draw over a full-screen
  game, and it is also the reason this design exists.
- `Plith.DropCatcher.exe` ships **without** `uiAccess` in every configuration, and is never
  launched as a child of `Plith.exe`. It is handed to the running Explorer, which gives it
  Explorer's Medium token.
- Target `net10.0-windows10.0.22000.0`, x64, `Nullable=enable`. Build with `-m:1`, never
  delete `obj/`.
- `scripts/check-a11y.ps1`, `scripts/check-shared-xaml.ps1` and `scripts/check-contrast.ps1`
  must pass before any release build.
- Everything arriving from the catcher is a claim about paths, never a command. `ShelfStore`
  is the single place that is enforced.

## 1. Two windows, one process

`Plith.DropCatcher` gains a second window. It does not extend the first one.

| | `CatcherWindow` (exists) | `ShelfWindow` (new) |
|---|---|---|
| Purpose | stands in for the notch during a drop | the shelf the person operates |
| Activation | `WS_EX_NOACTIVATE`, never takes focus | activates on open, takes keyboard |
| Alt+Tab | `WS_EX_TOOLWINDOW`, hidden | `WS_EX_TOOLWINDOW`, hidden |
| Lifetime | about 450 ms, withdraws on its own | until dismissed |
| Input | drop events only | press, drag, click, context menu, keys |

One window cannot serve both. The drop stand-in must not take focus, or appearing under a
carried file would pull focus away from whatever the person was doing. The shelf must take
focus, or `Esc`, arrow keys and a visible selection have nothing to arrive at. Those two
requirements cancel each other out in a single window.

Both windows keep the layered, per-pixel-alpha form the notch uses, for the reason recorded
on `CatcherWindow.xaml`: an opaque window cannot grow out of a resting strip, it can only
appear, and a hard cut between two processes is what a person reads as the shelf being
broken. The cost is also unchanged and is restated in section 8: a layered window cannot be
captured over Remote Desktop by any route.

### Growth

Plith hands over the notch's own open rectangle. `ShelfWindow` starts there and grows to the
shelf rectangle, driven by `NotchGeometry`, which is already linked into this project so the
two ends cannot drift into different curves.

**Provisional sizes, to be corrected against the render harness and marked as provisional in
the code that carries them:** 384 x 264 DIP surface, five columns, 64 DIP tiles, 8 DIP gaps.

These are arithmetic, and arithmetic is exactly what this repo has already been caught by:
the first `ShelfWidget` overflowed the frame on both axes at once, and a green build saw
none of it. The binding numbers come from `pwsh -STA -File scripts/render-widgets.ps1`, in
both themes with a tinted accent, before the surface is called done.

## 2. Who owns the shelf's state

**Plith owns it. The catcher renders it and asks for changes.**

`ShelfStore` stays in Plith: it stats what it is told about, keeps nothing it cannot see,
and is the only writer of `shelf.txt`.

The deciding reason is durability rather than security. The catcher is launched through
Explorer, can be killed, and may not be running at all in a given session. The shelf's
memory must not depend on it. A second consequence follows for free: the notch's shelf page
keeps working with no catcher connected, which is today's behaviour and worth keeping.

The rejected alternative is recorded so it is not re-proposed: moving `ShelfStore` into the
catcher removes every round trip and makes the surface snappier, at the price of the notch
being unable to show the shelf at all when the catcher is down, and of the lower-integrity
process becoming the sole authority over a file Plith reads.

### What the new verbs widen

The pipe's ACL is open to Everyone, by measurement and by necessity. Until this slice the only
verb that did anything was `Dropped`, which adds paths that `ShelfStore` then stats. `ClearShelf`,
`RemoveItems` and `Restack` mean any local process can now rearrange or empty the shelf.

This is accepted rather than redesigned, and the bound is why: the shelf holds references, nothing
is copied or moved, and clearing it deletes no file. Anything running at Medium can already read
and write the user's own files. It belongs in `DropChannelServer`'s summary so the next reader does
not have to rediscover the reasoning.

One consequence is not optional. `Enum.TryParse` accepts a number for any enum, so a line reading
`9` decodes to whatever verb happens to sit at 9, and one past the end decodes to a verb that does
not exist. `TryDecode` must check `Enum.IsDefined` as well. A verb reachable by counting is the
wrong field to leave guessable on a pipe anything can write.

## 3. Wire protocol

The format does not change: `Verb \t X \t Y \t W \t H \t path...`, with the existing escaping
of `\`, tab and newline inside paths. The four numbers are reused rather than a new separator
being invented, because a new in-field separator would need its own escaping and would be a
second thing to get wrong.

**Plith to catcher**

| Verb | Numbers | Paths | Meaning |
|---|---|---|---|
| `Show` | x, y, w, h | none | existing: stand in for a drop |
| `Hide` | zero | none | existing: stand down |
| `OpenShelf` | x, y, w, h | none | the notch is down; grow the shelf from this rectangle |
| `Items` | stack index, stack count, 0, 0 | that stack's paths, newest first | one message per stack |
| `Palette` | zero | `#AARRGGBB` values in a fixed order | the resolved theme |

`Items` is sent once per stack, in order, with index 0 being the front stack. The catcher knows
the set is complete when it has seen `stackCount` of them. An empty shelf is a single
`Items 0 0 0 0` with no paths.

`Palette` carries exactly eight values, in this order and no other: surface gradient start,
surface gradient end, ink, muted ink, track, accent, selection ring, and `1` for a dark theme or
`0` for a light one. The order is the contract; a named format would mean a parser on the far side and a second
thing to keep in step.

Two surface colours rather than one, found while planning: `OsdSurfaceBrush` is a
`LinearGradientBrush`, and a single flat stand-in for it would be visibly not the product's
surface, which is the exact drift this message exists to prevent.

The selection ring is the eighth field, and it was found by building the surface rather than by
designing it. Drawn as the raw accent it measures 1.25:1 against the panel for a near-white
accent, where the product holds non-text surfaces to 3:1. Every other accent-bearing element in
the product routes through a contrast-derived ink or track colour, which is exactly why
`check-contrast.ps1` passes on a white accent today; a raw accent stroke on the panel would have
made the shelf the one place that breaks. Plith derives it with `ContrastInk.RingOn` against the
surface and sends the answer, which is this message's whole principle.

`RingOn` rather than `TrackOn`, and the difference was measured rather than argued. `TrackOn` is a
function of the surface alone, so it throws the accent away: it took a vivid lime ring from 8.87:1
down to a dull grey-olive at 3.03:1, paying the common case to fix a rare one. `RingOn` returns the
accent untouched whenever the accent already clears 3:1, and otherwise walks its lightness, keeping
hue and saturation, in the direction the surface's own luminance chooses. That is the shape
`AccentTheme.Derive` already uses for a light background, generalised to both.

**Catcher to Plith**

| Verb | Numbers | Paths | Meaning |
|---|---|---|---|
| `Dropped` | zero | dropped paths | existing |
| `Hide` | zero | none | existing: withdrew without a drop |
| `RemoveItems` | zero | paths to remove | |
| `ClearShelf` | zero | none | |
| `NewStack` | zero | none | put an empty stack at the front for the next drop |
| `Restack` | target stack index, 0, 0, 0 | paths to move | index equal to the stack count appends a new stack |
| `ShelfClosed` | zero | none | the surface is gone; put the notch back |

The catcher receives paths only. Display names and icons are its own work, because it is the
side that displays them.

## 4. Stacks

`ShelfStore` holds a list of stacks; each stack holds items, newest first. `Items` stays as a
flat derived view so the notch's glance page and the existing tests keep working unchanged.

**A drop batch joins the front stack.** A new stack is started by the person, with an explicit
control on the surface. Time-window grouping was considered and rejected: a rule that groups
two drops made within N seconds and separates two made either side of it is unpredictable at
the moment it matters, and unpredictable is the wrong property for a container holding
someone's files.

The new stack is created empty and at the front, so the next drop joins it. That is what makes
the control useful before a drop rather than only after one. An empty stack that is still empty
when the surface closes is discarded, so the control cannot leave litter behind.

Items move between stacks by being dragged onto another stack, which sends `Restack`.

The existing cap of 20 items is a cap on the total across all stacks, not per stack. Past it
the oldest item falls off, as today.

### Persistence

`shelf.txt` keeps its promise of being readable and editable in Notepad. A blank line
separates stacks:

```
C:\report.pdf
C:\notes.txt

C:\photo.jpg
```

An existing file with no blank lines loads as a single stack, so backward compatibility costs
nothing and needs no version marker.

## 5. Dragging out

A press on a tile in `ShelfWindow`, past the system drag threshold
(`SystemParameters.MinimumHorizontalDragDistance` / `MinimumVerticalDragDistance`), calls
`DoDragDrop` with a `DataObject` carrying `DataFormats.FileDrop`. This is the same call Task 7
measured landing a file from Medium.

**Effects are `Copy | Link`, never `Move`.** The shelf stages references; it does not take
files away from where they are. `Move` on a `FileDrop` invites the destination to delete the
original, and a shelf that loses someone's work the first time they mistake it for a pocket
is worse than no shelf.

**The Task 8 hazard becomes a guard in code, not a comment.** `DoDragDrop` is callable only
from a mouse event raised on this window's own element tree. It is never called from a timer,
never from a pipe message, and never for a press whose origin cannot be established. Task 8
measured the call failing to return at all for a foreign press, and in this design the thread
that would block is the catcher's UI thread, which is the thread the whole shelf depends on.
A dead end is a design constraint; a hang is a hazard.

Selection interacts with the drag the way file managers do: pressing a tile that is part of
the current selection drags the whole selection, and pressing one that is not drags that tile
alone and resets the selection to it. Pressing a stack's header drags the stack.

While a drag is in flight the surface stays up and is not dismissed by focus loss. After a
successful drop the rows stay on the shelf, because `Copy` is what was offered.

## 6. Palette

The catcher's hand-written `#F2141414` and `#FFFFFFFF` go away. Plith sends the resolved
palette with `Palette` when it opens the shelf: surface background, ink, muted ink, track,
accent, and a light/dark flag.

Plith derives its inks from the colour behind them through `Services/ContrastInk.cs`. A second
copy of that derivation in the catcher would drift, and the drift would show as the shelf not
looking like the product it belongs to.

**`scripts/check-contrast.ps1` is extended to cover `src/Plith.DropCatcher`.** It scans only
`src\Plith` today, which is why the catcher's hard-coded colours have never been checked by
anything. Coherent appearance is the stated goal of this slice, and this script is the only
part of it that can be enforced automatically.

## 7. Entry, exit, and the catcher not being there

The shelf stays a page in the notch, as a read-only glance. It keeps what `ShelfWidget` shows
today, which is the flat newest-first view: up to five slots, the last spent on a `+N` count
whenever there is more than fits. Stacks are not drawn on the glance, because five slots
cannot show grouping and a half-drawn grouping is worse than none. The page gains only a hint
that it can be opened, and it works with no catcher connected, which is today's behaviour.

**Entry.** A click on the page hands over: Plith's window goes down, the catcher opens
`ShelfWindow` from the same rectangle. The hand-over happens on button release, so no press is
ever split across two processes. That is deliberate and it is the whole lesson of Task 8.

Dragging an item out therefore takes two gestures: open, then press and drag. A hover-based
hand-over would make it one, and was rejected for this slice because it adds a swap that has
to be timed against a moving cursor, on a surface that cannot be captured for review.

**Exit.** `Esc`, loss of activation, or the cursor leaving followed by a short grace period.
None of them fire while a drag is in flight or while a context menu is open. On exit the
catcher hides the window and sends `ShelfClosed`; Plith restores the notch through the path
`OnCatcherStoodDown` already uses.

**No catcher.** Plith calls `DropCatcherLauncher.EnsureRunning` and the page shows a brief
starting state; if it still does not connect, the page says the shelf is unavailable. A click
that does nothing is not an acceptable outcome, because it is indistinguishable from the
product being broken.

This requires a small change to `EnsureRunning`, which today returns `false` both for "one is
already running" and for "it could not be started". Those two have to be distinguishable
before anything can decide what to tell the person.

## 8. Testing and verification

**Unit tests** (the suite is not STA, so nothing here touches a window):

- `ShelfStore` with stacks: add to the front stack, restack, remove, clear, the 20-item cap
  across stacks, most-recent-first ordering.
- Persistence round trip, including a legacy file with no blank lines loading as one stack.
- The new verbs through `DropChannel.Encode` / `TryDecode`.
- Hostile paths containing tabs and newlines still cannot forge a second message on any of
  the new verbs. This is the test that protects the trust boundary, and it must grow with the
  verb list rather than stay pointed at `Dropped`.
- Palette encoding and decoding.

**What no test reaches:** the surface, the drag out, the hand-over, the growth, the contrast
of the rendered result. That list is the point of this section rather than an aside. Slice 1
found three defects by running a build that had a green build, green tests and a green lint
behind it, and the accessibility pass in Phase 5 shipped green with four defects in it.

These go in `docs/SHELF-VERIFICATION.md`, written as this slice is built rather than after,
and a console-session pass is a condition of the slice being called done. Remote Desktop
cannot see a layered window by any route.

The harness lessons from Task 8 move into that document rather than staying in a plan nobody
opens next: ask `qwinsta` and never `$env:SESSIONNAME`; stage and aim in one process and
re-verify the aim in the same breath as the press; use windows the harness itself created;
minimise the competing maximised window for the duration and restore it in a `finally`;
`WindowFromPoint` returns the child, so compare owning process ids.

## 9. Non-goals

Stated so they are not discovered as omissions.

- Dropping onto a High-integrity target. Still blocked by UIPI, in this direction as in the
  other. A known limit, not a defect.
- The shelf over a full-screen game. The catcher cannot enter the UIAccess band, and the
  shelf is a desktop tool.
- `Move` semantics.
- Naming stacks.
- Any change to how a drop arrives. Slice 1 works and is not reopened here.

## 10. Risks

1. **Nothing automatic can verify the result.** The whole surface is reachable only from a
   physical console session. This is the largest risk in the slice and the reason section 8
   is a gate rather than a checklist.
2. **The catcher becomes a real application.** Until now it was a 230-line stand-in whose
   crash cost one drop. It will now be holding a surface mid-gesture, and its crash is
   visible. Plith relaunching it on the next open is the whole recovery story in this slice,
   deliberately.
3. ~~**Paging versus clicking inside the notch frame.**~~ **Resolved while planning, and left
   here as the record.** `OsdHost.OnNotchClicked` is wired at `PreviewMouseLeftButtonDown` but
   returns early once the frame is open (`src/Plith/Views/OsdHost.cs:877`), and the rail marks
   its own clicks handled on the rail element (`src/Plith/Views/Widgets/WidgetFrame.cs:218`). A
   click on the page body reaches the page with no change to either. Read rather than run, so it
   is still confirmed by clicking.
4. **Focus.** Opening the shelf takes focus from the person's application. That is what
   comparable tools do, and Windows returns focus on close, but it is a behaviour change from
   a notch that never took focus at all.
5. **Provisional geometry.** Three numbers in section 1 were chosen without hardware. They
   are marked as such in the code, and the render harness corrects them.

## 11. Build order, and the size of this slice

This is a large slice, and saying so is part of the spec rather than a caveat on top of it.
It carries a data model change, a new interactive surface in a second process, a new drag
direction, shell icon extraction and a palette crossing a process boundary.

The order below exists because two of those cannot be reordered without work being done
twice.

1. **Stacks in `ShelfStore`, with persistence and its tests.** First, because the surface is
   built against this model. Built after, the surface is built against the flat list and then
   changed.
2. **The wire: the new verbs, their tests, and the hostile-path test extended to cover them.**
3. **`ShelfWindow`: the surface, its growth, the palette, its geometry against the render
   harness.** No actions yet, drawing only.
4. **Real shell icons**, in the catcher. This belongs at Medium integrity for a reason worth
   stating: icon extraction loads third-party shell handlers into the process doing it, and
   the process that must not be doing that is the one with UIAccess.
5. **The hand-over**: `OpenShelf`, `ShelfClosed`, the glance page's click, the notch going
   down and coming back, and the no-catcher path.
6. **Actions**: remove, clear, new stack, restack, open, show in the file manager.
7. **Selection and dragging out**, with the Task 8 guard. Last, because it is the only step
   whose failure mode is a hung UI thread, and it should meet a surface that already works.
8. **Accessibility, the three lint scripts including the extended contrast script, and the
   console-session verification pass.**

Steps 1, 2 and 4 are testable without a window. Steps 3, 5, 6 and 7 are not, and each one
should be looked at on a running build as it lands rather than at the end. Slice 1's three
defects were each found the moment something was actually run, and each had a green build
behind it.
