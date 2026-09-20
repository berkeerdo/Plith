# Shelf Slice 3: One Flat List

**Date:** 2026-09-20
**Branch it builds on:** `feature/shelf-drop-catcher` (slice 1 Tasks 1 to 8, slice 2)
**Spec this refines:** `docs/superpowers/specs/2026-09-18-shelf-slice-2-design.md`

## Why this change has the shape it has

Slice 2 shipped a shelf whose files live in named stacks: five columns, each column drawing
up to two tiles, the remainder folded into a `+N` chip. Alcove and Dropover, the two
applications this feature is modelled on, have no such concept. They have one storage area.

Measured against the code on 2026-09-20, the stack model costs more than it returns:

| Piece | What stacks cost it |
|---|---|
| `ShelfStore` | `List<List<ShelfItem>>` plus `NewStack`, `Restack`, `PruneEmptyStacks`, and a `TrimToCap` that walks backwards through stacks |
| `DropChannel` | two of the six catcher-to-Plith verbs exist only for stacks |
| `shelf.txt` | blank-line-separated groups instead of one path per line |
| `ShelfSurface` (1191 lines) | per-column building, `VisibleRowsShown`, the `+N` chip, and keyboard navigation that counts rows within columns |

The decisive number is the capacity mismatch. `ShelfStore.MaxItems` is 20, and the surface
can draw at most 10 tiles: `VisibleColumns = 5` times `VisibleRows = 2`. The other ten files
sit behind count chips, and **a folded tile is in no UIA tree**, so it is unreachable by
keyboard and invisible to a screen reader. The cap is twice what the surface can show.

That fold has also been the most expensive thing on this branch to reason about. Its rule is
`stackCount > VisibleRows ? VisibleRows - 1 : Math.Min(stackCount, VisibleRows)`, which means
a stack of two draws both tiles and a stack of **three draws exactly one**, because the chip
costs a slot. `docs/SHELF-VERIFICATION.md` stated this as "a column draws at most two tiles",
the driving script's fixture was built on that wrong statement, and two verification items
reported the product broken when the fixture was. See §3.11's banner and §3.12.

Removing stacks removes the fold, and removing the fold retires that whole class of problem
along with the accessibility gap.

## Goal

A person opens the shelf from the notch and sees every file on it at once, in one flat list,
newest first. Nothing is hidden behind a count and nothing scrolls. They can select one or
several, drag them out to any application, remove rows, and clear the shelf.

## Decisions taken, and why

1. **One flat list. No stacks, no grouping.** The model the two reference applications use.

2. **Nothing is ever hidden behind a count, on the shelf itself.** This is the point of the
   change rather than a side effect. Every file is drawn, in the UIA tree, and reachable by
   key.

3. **The cap equals what the surface shows: 15.** Five per row, three rows. Setting the cap
   to exactly the visible capacity turns "nothing is hidden" from a policy into a guarantee
   that cannot be violated by a large shelf, and it means **no scrolling is ever needed**, so
   no scroll machinery is written and keyboard navigation never has to agree with a scroll
   offset. A sixteenth file evicts the oldest, which is what `TrimToCap` already does.

4. **Newest first, with no hand reordering.** The order follows one rule and is predictable.
   The within-surface drag gesture is therefore deleted rather than repurposed: `Restack`, the
   drop-on-column hit testing and the drag-target highlight all go, and dragging becomes
   one-way, out of the shelf only. A shelf of at most 15 short-lived items does not earn a
   manual ordering, and a manual order would be disturbed by every new drop landing at the
   front anyway.

5. **The height is chosen once, when the shelf opens.** It does not change while the shelf is
   open. Removing a file reflows the content and leaves the window alone. This matters because
   the window is sized by Plith and sent over the pipe while it stands the notch aside, so
   resizing per item would spread an animation across two processes, and every cross-process
   coordination on this branch has cost several runs to get right. With decision 3 the height
   is in fact constant, but the rule is stated because it is what keeps a future "grow to fit"
   idea from being adopted without noticing the cost.

6. **The notch page keeps its count chip.** `ShelfWidget`'s frame is 356 wide and cannot show
   15 tiles. Unlike the shelf, a file hidden there is **not unreachable**: it is one click away
   in the full list. That asymmetry is deliberate and is the reason decision 2 is scoped to the
   shelf surface.

## The design

### Store and persistence

`ShelfStore._stacks`, today `List<List<ShelfItem>>`, becomes a single `List<ShelfItem>`.

Deleted: `NewStack`, `Restack`, `PruneEmptyStacks`, and the `Stacks` property. `Items` becomes
the primary interface rather than a `SelectMany` over stacks. `Add` prepends, as it does today
into the front stack. `TrimToCap` drops from the end, which is the oldest, and loses its
backwards walk over stacks.

`MaxItems` goes from 20 to 15, and stops being a free-standing number.

**The cap and the grid must not be able to drift apart**, because decision 3's guarantee is
exactly the statement that they are equal. They live on opposite sides of a process boundary:
the cap is `ShelfStore`'s, in Plith, and the grid is `ShelfSurface`'s, in `Plith.DropCatcher`.
A constant copied into both is a constant that will disagree eventually, and the disagreement
would be silent, showing up as a file that is on the shelf and not on the screen.

The repository already has the mechanism. `Plith.DropCatcher.csproj` compiles three files
straight out of Plith's tree with `<Compile Include="..\Plith\...">`: `DropChannel.cs`,
`ShelfPaletteWire.cs` and `NotchGeometry.cs`. A small `ShelfGeometry.cs` joins them, holding
`TilesPerRow = 5`, `RowCount = 3` and `Capacity = TilesPerRow * RowCount`. `ShelfStore.MaxItems`
becomes `ShelfGeometry.Capacity`, and the surface builds its grid from the same two numbers.

The invariant then holds **by construction, not by assertion**, and a unit test for it would
be comparing a value to itself. What is worth testing is the store honouring the cap at all
(a sixteenth file evicts the oldest), and what catches the surface failing to draw what the
store holds is the runtime check under Testing below. `Plith.Tests` needs no reference to the
catcher, which it does not have today and should not acquire for this.

`shelf.txt` becomes one path per line with no blank-line groups.

**No migration code is required.** Loading skips blank lines, so a file written by the old
build loads as one flat list in the order it was already in. The first save writes the new
form. This is worth stating because it is the reason this change needs no version marker and
no upgrade path.

### Wire protocol

`DropVerb` loses `NewStack` and `Restack`, going from twelve members to ten. The
catcher-to-Plith subset, which is the one the table above counts (`Dropped`, `RemoveItems`,
`ClearShelf`, `NewStack`, `Restack`, `ShelfClosed`), goes from six to four.

The verb crosses the pipe **by name**, not by ordinal: `DropChannel` writes
`message.Verb.ToString()` and reads it with `Enum.TryParse<DropVerb>`. Removing members
therefore renumbers nothing on the wire, and a stale catcher from an older build that sent a
removed verb would fail the parse and be rejected, which is a safe failure rather than a
misread message. This was checked rather than assumed, because a stale process surviving into
a new run is a thing that actually happened on this branch (§3.12, defect 5).

`ShelfSession` stops reporting stack counts in its log line.

### The shelf surface

Deleted from `ShelfSurface`: `BuildColumn`, `BuildOverflowTile`, `VisibleRowsShown`, the
column captions, and the `CaptionHeight` constant.

Replaced by a wrapping grid: five tiles per row, three rows, `TileSize = 64`, `Gap = 8`.
Content width is unchanged at 5 times 64 plus 4 times 8, which is 352.

**`VisibleColumns` changes meaning and must be renamed rather than reused.** Today it is the
number of STACKS a surface draws; in the new design the same number, 5, is the count of TILES
in a row. Keeping the old name for a new meaning is how a constant quietly stops matching what
it is called. It is replaced by `ShelfGeometry.TilesPerRow` and `ShelfGeometry.RowCount`, the
shared file described under Store and persistence, so the surface and the cap read the same
two numbers.

Keyboard navigation becomes linear over the grid rather than counting rows inside columns.
This removes the constraint that construction and navigation must independently agree on how
many rows a column shows, which is what `VisibleRowsShown` existed to enforce.

`ShelfWindow` loses the "start a new stack" control from its header and keeps clear.

### The notch page

`ShelfWidget` takes a flat list instead of stacks. It keeps `BuildOverflowTile` for the reason
in decision 6.

One existing defect is fixed as part of this work, because the file is being changed anyway
and the fix is in the same lines: `AutomationProperties.SetName(Tiles, "Shelf, N items")` sets
a name on a `StackPanel`, which WPF gives no automation peer, so the count reaches no screen
reader at all. This was filed in `docs/SHELF-VERIFICATION.md` §5.4 from reading the code and
confirmed against the live UIA tree in §3.11. The name moves to an element that has a peer,
and the corresponding suppression is removed from `scripts/check-a11y.ps1`.

The sibling suppression for `SetName(tile, ...)` targeting a `Border` is **not** in scope: it
predates this work and is not touched by it.

### Geometry

Content height today is `CaptionHeight` 13 plus 2 rows of 64 plus one 8 gap, which is 149,
inside a window measured at 384 by 224.

New content height is 3 rows of 64 plus two 8 gaps, which is 208, with no caption row.

That puts the window near 283 tall at unchanged width. **The 283 is derived, not measured**:
it assumes the chrome around the content is the 75 left over from today's numbers. The plan
must confirm it against the real window rather than adopt it, since a derived constant trusted
without measurement is the exact failure this branch keeps recording.

## What gets deleted

| Where | What |
|---|---|
| `tests/Plith.Tests/ShelfStoreTests.cs` | 37 references to stacks |
| `tests/Plith.Tests/ShelfModelTests.cs` | 10 |
| `tests/Plith.Tests/ShelfSessionTests.cs` | 8 |
| `tests/Plith.Tests/DropChannelTests.cs` | 4 |
| `scripts/drive-shelf-pair.ps1` | the §3.4, §3.5 and §3.6 steps, and the two-stack fixture |
| `docs/SHELF-VERIFICATION.md` | §3.4, §3.5, §3.6, and the restack items §4.5, §4.6, §4.9 |

§3.4 and §3.6 were driven and passed on 2026-09-20, hours before this spec, and are being
deleted. That is not waste: §3.12 records what the code did on the day, and four of the five
instrument defects that run uncovered are independent of stacks, so those fixes survive.

## Testing

Unit tests cover the store's flat behaviour: prepend on add, oldest evicted at 15, remove by
path, clear, and the load path flattening a blank-line-separated file written by the old
build. `ShelfModel` keeps its selection and `DragPaths` rules, which never depended on
columns.

The driving script keeps §3.1, §3.2 and §3.3, whose fixture becomes a flat list. One new
check earns its place: with 15 files on the shelf, every one of the 15 names is present in
the UIA tree. That is the guarantee decision 3 buys, and it is exactly the property the `+N`
chip broke, so it should fail the build if the fold ever returns.

`check-a11y.ps1`, `check-contrast.ps1`, `check-shared-xaml.ps1` and `render-widgets.ps1` must
stay green, with the one suppression removed as described.

## Risks and open items

1. **The window height is derived.** See Geometry. Confirm before adopting.
2. **A taller shelf sits lower on the screen.** At roughly 283 against today's 224, the
   surface reaches further down from the notch. Nothing measured says this is wrong, and
   nothing measured says it is right either. It should be looked at on hardware once, since
   the shelf can be captured live (§7.5).
3. **15 may prove too few in use.** The cap is one constant and the guarantee in decision 3
   survives any value as long as rows times columns matches it. Raising it means adding a row
   and re-deriving the height, not adding scrolling.
4. **`render-widgets.ps1` has structural assumptions about tile nesting.** It needed a
   `.Children[0]` adjustment when slice 2 wrapped tile content in a Grid. The flat grid will
   move that nesting again.
