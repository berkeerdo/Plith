# The shelf in the notch: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:executing-plans. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The shelf stops being a separate surface. The catcher draws it inside the notch's own
open frame, a person drags a file straight out of the notch, and paging still works while the
catcher holds the frame.

**Architecture:** The existing stand-in mechanism is kept; only its size, framing and trigger
change. The catcher's window becomes the notch's open frame rectangle, `ShelfSurface` becomes a
two-row notch page with no header, the handover happens on a page commit rather than a click, and
the catcher forwards wheel and rail gestures to Plith over the pipe so the paging rules stay in
one place.

**Tech Stack:** WPF, .NET 10, two processes over a named pipe with an explicit ACL, xunit,
PowerShell harnesses.

**Spec:** `docs/superpowers/specs/2026-09-21-shelf-in-the-notch-design.md`

## Global Constraints

- All code, comments and commits in English. No AI attribution anywhere.
- No em dash, and no hyphen used as a sentence separator, in any prose.
- A file can only be dragged out of the catcher's window, and a press cannot be delegated between
  processes. Both are measured: `docs/SHELF-VERIFICATION.md` section 4.
- Capacity must equal what the surface draws. A folded tile is in no UIA tree.
- `NotchGeometry.cs` is compiled into BOTH projects, so every shared number goes there and
  nowhere else.
- Gates before any claim: `dotnet build`, `dotnet test`, `scripts/check-a11y.ps1`,
  `scripts/check-contrast.ps1`, `scripts/check-shared-xaml.ps1`, `scripts/render-widgets.ps1`.
- The pair must be driven on hardware before this is called done:
  `scripts/drive-shelf-pair.ps1`. A green gate has never once seen a wrong shape in this repo.

---

### Task 1: The numbers, in the one file both projects compile

**Files:**
- Modify: `src/Plith/Views/Presentation/NotchGeometry.cs`
- Test: `tests/Plith.Tests/NotchGeometryTests.cs`

**Interfaces:**
- Produces: `ShelfRowCount = 2`, `ShelfTilesPerRow = 5`, `ShelfCapacity = 10`,
  `ShelfTileWidth = 56`, `ShelfTileHeight = 52`, `ShelfGap = 8` (unchanged),
  `ShelfPageRect(Rect hoverRect) -> Rect` (the notch's open frame, which the catcher now takes).
- Consumes: `OpenFrameDip`, `PageInsetDip`, `PageRailRowDip`, `DropTargetRect`.

- [ ] **Step 1: Write the failing tests**

```csharp
[Fact]
public void TheShelfGridFitsTheNotchsContentBand()
{
    // The band is the frame less the page inset, and the inset's bottom already clears the rail.
    var band = NotchGeometry.OpenFrameDip.Height
             - NotchGeometry.PageInsetDip.Top - NotchGeometry.PageInsetDip.Bottom;
    var rows = NotchGeometry.ShelfRowCount * NotchGeometry.ShelfTileHeight
             + (NotchGeometry.ShelfRowCount - 1) * NotchGeometry.ShelfGap;

    Assert.True(rows <= band, $"{rows} DIP of tiles in a {band} DIP band");
}

[Fact]
public void AFullRowFitsTheNotchsContentWidth()
{
    var band = NotchGeometry.OpenFrameDip.Width
             - NotchGeometry.PageInsetDip.Left - NotchGeometry.PageInsetDip.Right;
    var row = NotchGeometry.ShelfTilesPerRow
            * (NotchGeometry.ShelfTileWidth + NotchGeometry.ShelfGap);

    Assert.True(row <= band, $"{row} DIP of row in a {band} DIP band");
}

[Fact]
public void CapacityIsTheGridAndNothingElse()
{
    Assert.Equal(NotchGeometry.ShelfRowCount * NotchGeometry.ShelfTilesPerRow,
                 NotchGeometry.ShelfCapacity);
}

[Fact]
public void TheShelfPageTakesTheOPENFRAMEAndNotAnInventedRectangle()
{
    var hover = new Rect(700, 0, 190, 8);

    Assert.Equal(NotchGeometry.DropTargetRect(hover), NotchGeometry.ShelfPageRect(hover));
}
```

- [ ] **Step 2: Run them and watch them fail**

`dotnet test --filter "FullyQualifiedName~NotchGeometryTests"`. Expected: does not compile,
`ShelfPageRect` and the tile constants do not exist.

- [ ] **Step 3: Make them pass**

Add the constants and `ShelfPageRect` (a one-line delegation to `DropTargetRect`, with a comment
saying why it exists at all: the name is the intent, and a second caller computing the open frame
itself is how the two would drift). Change `ShelfCapacity` to the product of the new grid.

- [ ] **Step 4: Delete `ShelfFrameFor`, `ShelfRowsFor`, `ShelfFrameDip` and `ShelfRect`**

They size a pane that no longer exists. Deleting them is the point of the task rather than tidying
after it: left in place, the next person sizes something with them. Their tests go too, including
the `GrowthStart` cases that walk every shelf size, and `GrowthStart` itself if nothing else calls
it. Expect the two projects, the harness and the driver to stop compiling or running; the
following tasks fix each.

- [ ] **Step 5: Gates, then commit**

`dotnet build` will fail until Task 2. Commit only when Tasks 1 and 2 are both green.

---

### Task 2: The catcher draws a notch page, not a pane

**Files:**
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml`, `ShelfSurface.xaml.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs`
- Modify: `src/Plith/Services/Shelf/ShelfSession.cs`

**Interfaces:**
- Consumes: Task 1's constants and `ShelfPageRect`.
- Produces: a surface whose content is a two-row grid inside `PageInsetDip`, with a page rail
  along the bottom; `ShelfSurface.PageRequested(int index)` and
  `ShelfSurface.PageWheel(int delta)`, which Task 3 carries over the wire.

- [ ] **Step 1: Strip the header**

Delete the header grid, the `Shelf` title, `HeaderCount`, `ClearButton`, `CloseButton` and
`HeaderButtonStyle`. Delete `CloseRequested` and its subscription in `ShelfWindow`. Keep
`ClearRequested`, which Step 3 gives a new home.

- [ ] **Step 2: Two rows of five, at Task 1's sizes**

`Columns` becomes the tile grid directly: a `WrapPanel` of `ShelfTilesPerRow *
(ShelfTileWidth + ShelfGap)`, tiles at `ShelfTileWidth x ShelfTileHeight`, icon 24, name ONE line
at 11 with `CharacterEllipsis`, tooltip unchanged (an explicit `ToolTip` object, see the tooltip
rule in `Render`). The root grid takes `Margin="{x:Static p:NotchGeometry.PageInsetDip}"`, which is
what puts this page in the same box as every other one.

- [ ] **Step 3: Clear, without a button**

A `ContextMenu` on the page background with one item, "Clear the shelf", raising `ClearRequested`.
Plus `Ctrl+A` selecting every tile in `ShelfWindow`'s key handling, so `Delete` then clears from the
keyboard. Both need an accessible name; `check-a11y.ps1` fails the build otherwise.

- [ ] **Step 4: The rail**

Draw `PageCount` dots in the rail row, the shelf's one filled, from the same geometry
`WidgetFrame` uses. A click on a dot raises `PageRequested(index)`. The page count and the shelf's
index arrive with the `OpenShelf` message (Task 3), because only Plith knows how many pages there
are.

- [ ] **Step 5: The window stops growing**

`ShelfWindow.ApplyExpansion` keeps the content fade and drops the size interpolation: the shape is
the window. `ShelfSession.Open` sends `ShelfPageRect` instead of `ShelfRect`.

- [ ] **Step 6: Gates**

`dotnet build`, `dotnet test`, the three lints. `render-widgets.ps1` will fail on the deleted
fixtures; Task 5 rewrites them.

- [ ] **Step 7: Commit**

```bash
git commit -m "refactor(shelf): draw the shelf inside the notch's own frame"
```

---

### Task 3: The wire carries the frame, the pages, and the gesture

**Files:**
- Modify: `src/Plith/Services/Shelf/DropChannel.cs` (the verb enum)
- Modify: `src/Plith/Services/Shelf/ShelfSession.cs`
- Modify: `src/Plith.DropCatcher/App.xaml.cs`
- Modify: `src/Plith/Views/OsdHost.cs`
- Test: `tests/Plith.Tests/` (the decode of a page message, if any logic lands outside a window)

**Interfaces:**
- Produces: `DropVerb.Page`, carried as `x = delta` and `y = index` with one of the two zero, and
  `ShelfSession.PageRequested(int delta, int index)`; `OpenShelf` gains the page count and the
  shelf's page index in `w`/`h` of a second message rather than being overloaded.
- Consumes: `OsdHost.OnHorizontalWheel(delta)` and `OsdHost.OnWidgetPageRequested(index)`, which
  already exist and which this must reuse rather than reimplement.

- [ ] **Step 1: The verb, in both directions**

Add `Page`. The catcher sends it; Plith answers it by feeding its own pager. Nothing in the
catcher decides what a delta means: `WheelDecoder` and `NotchPager` stay in Plith, which is the
whole reason the raw delta travels.

- [ ] **Step 2: The catcher raises it**

`ShelfWindow` handles `MouseWheel` (and the tilt wheel through `WM_MOUSEHWHEEL`, the same way
Plith does) and `ShelfSurface.PageRequested`, and `App` bridges both onto the client exactly as it
bridges `RemoveItems`.

- [ ] **Step 3: Plith answers it**

`ShelfSession` raises `PageRequested`; `OsdHost` feeds a delta to `OnHorizontalWheel` and an index
to `OnWidgetPageRequested`. A commit that lands on another page closes the shelf.

- [ ] **Step 4: Gates, then commit**

---

### Task 4: The handover happens on the page commit

**Files:**
- Modify: `src/Plith/Views/OsdHost.cs`
- Modify: `src/Plith/Views/Widgets/ShelfWidget.cs`
- Delete: `src/Plith/Views/Presentation/HoverOpenIntent.cs`,
  `tests/Plith.Tests/HoverOpenIntentTests.cs`

**Interfaces:**
- Consumes: `_pager.Index`, `_shelfPageIndex`, `ShelfSession.Open/Close`.

- [ ] **Step 1: One place decides**

A single method, called after every page commit, after the frame opens, and when the frame
collapses: if the frame is open and the current page is the shelf, ask the catcher for the frame;
otherwise take it back. It must be idempotent, because it is called from three places and the
answer is usually "no change".

- [ ] **Step 2: Delete the hover path**

`ShelfWidget` loses `OnHoverMove`, `CancelHoverOpen`, the dwell timer and the "hover to open" line;
its line becomes the file count alone. `HoverOpenIntent` and its six tests are deleted, and the
commit message says why: the design moved and a redundant trigger for a cross-process swap is a
second thing to get wrong.

- [ ] **Step 3: The driver's page key**

`scripts/drive-shelf-pair.ps1` keys the shelf page on "hover to open", which this step deletes.
Key it on the count line instead, and write down that the key has now moved twice, both times
because it was a sentence rather than an identity.

- [ ] **Step 4: Gates, then commit**

---

### Task 5: The harness and the driver measure the new shape

**Files:**
- Modify: `scripts/render-widgets.ps1`
- Modify: `scripts/drive-shelf-pair.ps1`

- [ ] **Step 1: Renders**

`shelf-surface` at the NOTCH frame (356 x 164) with 1, 5, 10 files and empty. Delete
`shelf-surface-small`, `shelf-surface-cleared` and the frame-equality check, which all measure a
pane that no longer exists. Keep the tile-hit check, the tooltip check and the menu check, and
point them at the new tiles.

- [ ] **Step 2: A capacity assertion that cannot drift**

Every one of `ShelfCapacity` files must be in the UIA tree, which is the check the stack build
could never have passed and which is the reason the cap is the grid.

- [ ] **Step 3: The driver**

Replace the click-to-open and hover-to-open stages with a page-commit stage: wheel onto the shelf
page and assert the CATCHER's window is what holds the frame, at the notch's own rectangle. Then
wheel again and assert the frame comes back to Plith. Keep every removal, drag-out and keyboard
stage.

- [ ] **Step 4: Run it on hardware and record the result**

`docs/SHELF-VERIFICATION.md` gets a section. It says which half was measured and which was not,
in the words of what actually ran.

- [ ] **Step 5: Commit**

---

### Task 6: The record

**Files:**
- Modify: `CLAUDE.md`, `docs/ROADMAP.md`, `docs/SHELF-VERIFICATION.md`

- [ ] **Step 1: Say what changed and what it cost**

Ten files instead of fifteen, one line of name instead of two, no Clear button, and a
cross-process swap on every page turn onto the shelf. The costs go next to the gain, not in a
separate place from it.

- [ ] **Step 2: Commit**
