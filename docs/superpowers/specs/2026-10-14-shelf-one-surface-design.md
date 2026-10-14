# Shelf: one surface, drawn by whichever process is holding it

Date: 2026-10-14. Status: proposed, not built.

## Why

Reported from the running build, twice, in the owner's own words: the shelf page is not fluid like
the other widgets, it "closes and then that one opens", and it does not come in at the same size.
The instruction that opened this spec was to rewrite the structure there if that is what it takes.

The colours were a separate defect and are already fixed (`SHELF-VERIFICATION.md` 10.30). What is
left is the SEAM, and the seam is structural rather than cosmetic.

## What a person sees today, and why

Paging onto the shelf runs this sequence:

1. The outgoing page slides away inside Plith's own `WidgetFrame`, 260 ms.
2. What slides IN is blank. `ShelfWidget` draws nothing at all, by design, and says so in its own
   header: it used to draw a 552-line second shelf and a person reported that three times, ending
   with "why are there two shelves", so it was deleted.
3. `OsdHost.ReconcileShelfFrame` hands the frame over. Plith stands aside.
4. The catcher's window arrives over the top and fades in over 70 ms.

So the notch empties, yields, and a separate surface appears. That is the reported sentence almost
word for word, and no amount of colour work reaches it.

**One phantom ruled out while writing this.** The arrival is NOT a growth. `ShelfWindow` sets
`Shape.Width/Height` and `Page.Width/Height` to the final size on the first frame and animates
opacity alone; `Expansion` and `GrowDuration` are historical names. So the reported size change is
NOT the catcher resizing itself, and this spec does not design against that. What it is remains
open, below.

## The constraints that cannot move

Each was measured, and each is why the obvious fixes are not available:

- **A file can only be dragged out of the catcher's window.** Plith runs at high integrity in a
  Release build (UIAccess, which the band window needs for fullscreen) and `DoDragDrop` returns
  `None` from there. `SHELF-VERIFICATION.md` section 4.
- **A press cannot be delegated between processes.** So the window under the pointer at press time
  must be the one that starts the drag.
- **Two processes cannot animate one page turn.** Tried, reported as stuttering, reverted:
  `SHELF-VERIFICATION.md` 10.26. Whatever moves during the turn must be moved by ONE process.

## The design

**One surface, one implementation, drawn by whichever process is currently holding the frame.**

Plith's shelf page stops being blank and starts drawing the real shelf, using the SAME control the
catcher draws, from the same source, with the same palette and the same store contents. The page
turn then belongs entirely to Plith: it is the same slide, in the same frame, at the same size, as
the other four pages, because it IS the other four pages' slide. The handover still happens, but
it happens between two pictures that are identical, so there is nothing to see.

This is explicitly NOT the deleted second shelf. That one was a different implementation that
looked different, which is what produced "why are there two shelves". One implementation drawn
twice cannot produce two looks.

### Where the shared code lives, and why not the obvious place

The obvious move is to LINK `ShelfSurface.xaml`, `ShellIcons.cs` and `ShelfModel.cs` into Plith,
the way `ShelfPaletteWire.cs` is already linked into the catcher. **That would be a mistake, and
the repo has already paid for it once.** Two compiles of one source produce two CLR types that
merely share a name. `render-widgets.ps1` carries a paragraph about exactly this: it cannot write
`[Plith.Services.Shelf.ShelfPalette]`, because the literal binds to whichever assembly loaded
first, and `ShelfSurface.Apply` demands the catcher's own copy. Linking four more files multiplies
that hazard and moves it from a harness into product code.

So: **a third assembly, `Plith.Shelf`, referenced by both processes.** It holds the shelf's UI and
the types that cross the boundary:

**Corrected while writing the plan, and the correction matters.** The catcher does not link one
file today, it links FIVE, and `ShelfSurface` depends on three of them. A shared assembly that
left those behind would have to link them itself, which makes a THIRD copy of each type and
leaves the codebase worse than it started. So the shared assembly takes all of them, and every
`<Compile Include="..\Plith\...">` link in `Plith.DropCatcher.csproj` is deleted:

| Moves into `Plith.Shelf` | From | Linked into the catcher today |
|---|---|---|
| `NotchGeometry.cs` | `Plith` | yes |
| `ShelfPaletteWire.cs` (`ShelfPalette`, the wire format) | `Plith` | yes |
| `ShelfLabel.cs` | `Plith` | yes |
| `DropChannel.cs` | `Plith` | yes |
| `WheelDecoder.cs` | `Plith` | yes |
| `ShelfModel.cs` (`ShelfModel`, `ShelfEntry`) | `Plith.DropCatcher` | no, its own |
| `ShellIcons.cs` | `Plith.DropCatcher` | no, its own |
| `ShelfSurface.xaml` and `.xaml.cs` | `Plith.DropCatcher` | no, its own |

Both `Plith` and `Plith.DropCatcher` reference it. Neither references the other, so the rule that
matters is kept: **the catcher still never depends on Plith's assembly**, and the reason given for
linking (a reference "would pull Plith's assembly into a process that must stay small") does not
apply to a small assembly that holds only what both ends already share.

**Namespaces do not change.** `NotchGeometry` stays `Plith.Views.Presentation`, the palette stays
`Plith.Services.Shelf`. A type moving between assemblies with its namespace intact changes no call
site in either process, which is what keeps this a move rather than a rewrite of everything that
touches geometry.

The duplicate-type hazard disappears entirely rather than being contained: after this there is one
`NotchGeometry`, one `ShelfPalette`, one `ShelfSurface`. `render-widgets.ps1`'s workaround for it
(building the palette through the catcher assembly's own `Type` object) can be deleted, and that
deletion is the cheapest proof that the hazard is gone.

### What each process does

- **Plith** draws the shelf page inside `WidgetFrame` like any other page. It already owns
  `ShelfStore`, so it has the data without asking anyone. It does not handle presses that start a
  drag, and it never shows a tile as being dragged.
- **The catcher** keeps the window, the press, the drag and the drop, unchanged. It draws the same
  control with the same palette.
- **The handover** keeps its current trigger and timing. The only change is that what it covers is
  no longer blank.

### The open question this spec does not close

The reported size change is not the catcher resizing (ruled out above). It needs a frame-by-frame
CAPTURE of one turn onto the shelf to pin down, which no instrument in this repo takes today: the
page-turn trace samples Plith's UI thread and cannot see two windows. Candidates worth capturing
for: a one-frame difference between Plith's panel and the catcher's `Shape` (the latter is the
window less the shadow margin), a doubled shadow during the 70 ms crossfade, or the corner radius.

**This design makes the question cheaper rather than answering it.** With both sides drawing the
same control, any remaining difference is in the window geometry alone, which is one comparison
rather than two surfaces' worth.

## Not in scope

- Changing when the handover fires, or its 70 ms fade.
- `NotchGeometry` moving assemblies.
- The shelf's behaviour: selection, the remove button, the context menu, the drop flow and the
  rail all keep today's semantics.
- The page-turn smoothness measurement still owed in `PERF-VERIFICATION.md` 13.4. It is a separate
  job and this change does not settle it.

## How it will be verified

Nothing here counts as done on a green build, which is this repo's most expensive lesson.

1. **The same control, twice, pixel for pixel.** `render-widgets.ps1` renders Plith's shelf page
   and the catcher's shelf surface in the same run, both themes, and asserts the two trees match
   on the properties that would betray a divergence (tile count, tile rects, brushes). A diff here
   is the whole design failing.
2. The existing shelf checks keep passing: `tile-hit`, `floor`, `literal`, `hover`, the edge
   check, the tooltip and menu checks.
3. `check-a11y` and `check-contrast`.
4. **On hardware, at the console:** page onto the shelf and watch the moment of the handover. This
   is the only check that can confirm the reported symptom is gone, and it cannot be automated.
5. Both test suites: `Plith.Tests` and `Plith.Installer.Tests`.

## Risks

- **Icon extraction now happens in two processes.** `ShellIcons` caches per process, so the first
  paint of a page in Plith pays what the catcher already paid. Measure it; if it costs, Plith can
  be handed the icons it already extracts for nothing else, or the page can be warmed the way
  `ShelfWindow.Warm()` already warms the catcher's.
- **A new assembly is a new thing to ship.** The installer and `check-shared-xaml.ps1` both need
  to know about it; that script exists because 0.1.6 shipped a broken installer over exactly this
  class of mistake.
- **Two pictures that must not drift.** The verification's first item is the guard, and it has to
  be written before the design is trusted.
