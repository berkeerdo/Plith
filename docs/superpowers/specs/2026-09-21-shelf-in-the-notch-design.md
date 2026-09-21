# The shelf IS the notch page

**Date:** 2026-09-21
**Status:** approved, implementation planned in `docs/superpowers/plans/2026-09-21-shelf-in-the-notch.md`

## The report

0.2.0 shipped and was installed on someone else's machine. The first person to meet the shelf
tried to drag a file straight out of the notch and got the shelf opening instead. Told about it,
the user's own conclusion: **do not have a separate shelf at all, let it be pulled straight from
the notch.**

An intermediate fix shipped first (hover opens the shelf, commit `bbceedf`) and it is not enough.
It makes the gesture work, but what appears under the pointer is still a second surface: 384 DIP
wide against the notch's 356, up to 290 tall against 164, with its own header, its own Clear and
its own close box. It reads as a window the notch produced rather than as the notch.

## The constraint this design is shaped by

**A file can only be dragged out of the catcher's window.** Plith runs at high integrity in a
Release build so that it can sit over fullscreen games, and `DoDragDrop` from there returns `None`
with nothing copied: measured by controlled comparison, the same binary returning `Copy, Move`
from medium integrity with the file landing (`docs/SHELF-VERIFICATION.md` section 4).

**And the press cannot be delegated.** Task 8 measured that too, three times: a press that lands
on Plith's window cannot become a drag in the catcher, no `DragEnter` ever arrives, and one call
did not return for seventeen seconds. So a hazard, not a retry.

Therefore the tiles a person drags **must belong to the catcher's window**, and that window must
be there before the press. The mechanism for this already exists and works: the catcher stands in
for the notch. What is wrong is only its **size and framing**.

## The design

**The catcher's window takes the notch's open frame, exactly.** Same rectangle
(`NotchGeometry.DropTargetRect`-derived open frame, 356 x 164, same anchor, same top edge), same
corner radius, same surface brush, same content inset. There is no growth from one size to
another because there is nothing to grow into: the shape is already the notch's.

**The shelf page becomes the catcher's whenever it is the page on screen.** Not on a click, not on
a hover: on the page commit. Paging onto the shelf hands the frame over; paging off it hands the
frame back. A person sees one surface with five pages, one of which happens to be drawn by another
process.

### Layout

Two rows of five, ten files, and the capacity is defined as the product of that grid the way it
already is for the pane. The arithmetic, from the numbers the frame already fixes:

- Content band: `164 - 14` (inset top) `- 29` (inset bottom, which clears the page rail) = **121**.
- Two rows plus one gap must fit: `2h + 8 <= 121`, so a tile is at most 56 tall. It is **52**,
  leaving the pair at 112 and a little slack rather than none.
- Row width: five tiles at `w + 8` must fit 320, so `w = 56`. Five tiles and their gaps come to
  exactly 320.

A tile carries a 24 DIP icon over **one** line of name, ellipsis-trimmed, with the full name in the
tooltip. Two lines do not fit in 52 and the pane is gone, so the tooltip is the only place a long
name survives. This is a real loss and it is the price of the surface being the notch.

### What happens to the header

There is no header. The 40 DIP it took would leave room for one row.

- **Close** is not needed: the frame goes away when the pointer leaves, like every other notch page.
- **Clear** moves to a context menu on the page background, plus `Ctrl+A` and `Delete` from the
  keyboard, which is the idiom every file manager already uses and needs no space at all.
- **The count** moves to nothing: ten of ten files are drawn, so the number is the row itself.

### Paging while the catcher holds the frame

The catcher forwards the gesture rather than deciding it. It handles the wheel, `Shift`+wheel and a
tilt wheel on its own window and sends the RAW delta over the pipe; Plith decodes it with the same
`WheelDecoder` and the same `NotchPager` every other page uses, so the commit threshold and the
idle rearm live in one place. A commit to a different page closes the shelf and shows Plith's own
page.

The catcher draws the page rail itself, from the shared `NotchGeometry`, with the shelf's dot
active, and a click on a dot sends a page index. Without this the wheel would be dead on one page
out of five, which reads as broken rather than as absent.

### What is deleted

- **The pane.** `ShelfSurface`'s header, its Clear button, its close box, and `ShelfFrameFor`'s
  whole row-count arithmetic, since the frame no longer depends on how many files there are.
- **Hover-open and `HoverOpenIntent`,** with its tests. The commit-driven handover makes it
  redundant, and a redundant trigger for a cross-process window swap is a second thing to get
  wrong. It lasted one commit; the design moved.
- **The growth animation** between two sizes. The content still fades in, because the swap between
  two processes is not instantaneous and a hard cut reads as a flash.

### What Plith's own shelf page is still for

`ShelfWidget` stays, and it is not a leftover. It occupies the pager slot, it is what is drawn
while the catcher is being asked, and it is the only thing that can say why the shelf is
unavailable when the helper is missing. It keeps its empty state. It loses its hover handling and
its "hover to open" line, because the page it describes is no longer the page that opens.

## What this costs, stated plainly

- **Ten files instead of fifteen**, and one line of name instead of two. A cap that the surface
  cannot draw is the defect slice 3 was written to remove: a folded tile is in no UIA tree, so it
  is invisible to a screen reader and reachable by no key. Ten drawn beats fifteen with five
  hidden.
- **A cross-process window swap on every page turn onto the shelf.** Every cross-process step on
  this branch has cost runs to get right, and this one happens during an animation. It is the
  highest risk in the slice and it is why the rail and the wheel forwarding are in scope rather
  than deferred: a swap that loses the wheel is a swap a person cannot get out of.
- **No Clear button.** Discoverable only by right-click or by `Ctrl+A` then `Delete`.

## Deliberately not built

- **Expanding to a larger surface for more files.** That is the pane, and the pane is what the
  report is about.
- **Reordering tiles by dragging inside the shelf.** Deleted in slice 3 for the same reason it
  stays deleted: newest first is the only order, and a drag inside the surface competes with the
  drag out of it.
- **A count badge on the resting notch.** The resting shape is between 2 and 24 DIP tall, which is
  not a surface that can carry a number.
