# Shelf Single Flat List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the shelf's stack model with one flat list of at most 15 files, where every file is drawn, in the UIA tree and reachable by key.

**Architecture:** The cap and the grid become shared constants in `NotchGeometry.cs`, which is already compiled into both projects, so Plith's store and the catcher's surface cannot disagree about how many files fit. `ShelfStore` becomes a `List<ShelfItem>`, the `Items` wire message collapses from one-per-stack to one, `ShelfSurface` draws a wrapping grid instead of columns, and the `+N` chip disappears from the shelf.

**Tech Stack:** C# / .NET 10, WPF, xUnit, PowerShell 7 lint and driver scripts.

**Spec:** `docs/superpowers/specs/2026-09-20-shelf-single-list-design.md`

## Global Constraints

- All code, comments, identifiers, test names and commit messages in **English**.
- **No em dash** (`—`) anywhere: not in code, comments, documents or commit messages. Use a comma, a colon, parentheses, or a second sentence.
- No AI attribution of any kind in commits, code or docs.
- Conventional Commits, subject at most 50 characters.
- Build: `dotnet build Plith.slnx -m:1`
- Tests: `dotnet test Plith.slnx -m:1`
- Lints run under **PowerShell 7**: `pwsh -NoProfile -File scripts/<name>.ps1`. They carry `#requires -Version 7.0` and fail under Windows PowerShell 5.1.
- Baseline before this plan starts: build 0 errors 0 warnings, tests 473 + 17 passing, `check-a11y`, `check-shared-xaml`, `check-contrast` all exit 0.

---


## Status, and two defects this plan had (2026-09-20)

**Task 1 done** (`310ef19`). **Task 2 done** (`29ca8df`), having absorbed work this plan had
put in Task 3. Build 0 errors 0 warnings, 471 + 17 tests passing, all three lints exit 0.

Executing the plan found two things wrong with it, both about where the task boundaries fall.

1. **Task 2 could not compile on its own.** `ShelfSession` uses the store in five places (the
   log line, `SendStacks`, and the `NewStack`, `Restack` and `PruneEmptyStacks` cases). This plan
   put them in Task 3, which would have made Task 2 a commit that does not build. They moved into
   Task 2.

2. **Tasks 3 and 4 cannot be separated, and are now one task.** Flattening `ShelfModel` drags the
   surface with it: `ShelfSurface.Render` reads `model.Stacks`, `App.xaml.cs` raises the two
   verbs, and `ShelfWindow` bridges `SetStack`. There is no ordering of the two that leaves the
   branch compiling and running in between. They are merged below as Task 3.

**Carried deliberately, and both say so in the code:** `DropVerb.NewStack` and `DropVerb.Restack`
are still declared, marked inert, because the catcher still raises them; and `SendItems` declares
a stack total of 1 rather than 0, because the current `ShelfModel` discards a delivery that
declares 0. Both disappear in Task 3.

### Task 1: Shared shelf geometry

Put the cap and the grid in one place that both processes compile, and make the frame height an expression over the row count instead of a second literal.

**Files:**
- Modify: `src/Plith/Views/Presentation/NotchGeometry.cs` (around line 98, `ShelfFrameDip`)
- Test: `tests/Plith.Tests/NotchGeometryTests.cs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `NotchGeometry.ShelfTilesPerRow` (int, 5), `NotchGeometry.ShelfRowCount` (int, 3), `NotchGeometry.ShelfCapacity` (int, 15), `NotchGeometry.ShelfTileSize` (double, 64), `NotchGeometry.ShelfGap` (double, 8), `NotchGeometry.ShelfFrameDip` (Size, unchanged name, height now computed).

- [x] **Step 1: Write the failing test**

Append to `tests/Plith.Tests/NotchGeometryTests.cs` (create the file with `using Plith.Views.Presentation; namespace Plith.Tests;` and a `public sealed class NotchGeometryTests` if it does not exist):

```csharp
/// <summary>
/// The guarantee the flat shelf rests on: the cap is exactly what the surface can draw, so no
/// file can be on the shelf and off the screen. Written as a test rather than left to the
/// definition because a later edit could reintroduce a free-standing number.
/// </summary>
[Fact]
public void ShelfCapacityIsExactlyWhatTheGridDraws()
{
    Assert.Equal(NotchGeometry.ShelfTilesPerRow * NotchGeometry.ShelfRowCount,
                 NotchGeometry.ShelfCapacity);
    Assert.Equal(15, NotchGeometry.ShelfCapacity);
}

/// <summary>
/// The frame must be tall enough for every row it promises. A height typed as a literal is free
/// to stop matching the row count above it, which is the failure this derivation exists to make
/// impossible.
/// </summary>
[Fact]
public void ShelfFrameIsTallEnoughForEveryRow()
{
    var rows = NotchGeometry.ShelfRowCount * NotchGeometry.ShelfTileSize
             + (NotchGeometry.ShelfRowCount - 1) * NotchGeometry.ShelfGap;

    Assert.True(NotchGeometry.ShelfFrameDip.Height >= rows,
        $"frame {NotchGeometry.ShelfFrameDip.Height} is shorter than its {NotchGeometry.ShelfRowCount} rows ({rows})");
}

/// <summary>
/// A row of tiles plus the gaps between them must fit the frame's width, which is unchanged at
/// 384. Five 64 DIP tiles with four 8 DIP gaps is 352, leaving 32 for the horizontal chrome.
/// </summary>
[Fact]
public void ShelfFrameIsWideEnoughForARow()
{
    var row = NotchGeometry.ShelfTilesPerRow * NotchGeometry.ShelfTileSize
            + (NotchGeometry.ShelfTilesPerRow - 1) * NotchGeometry.ShelfGap;

    Assert.True(NotchGeometry.ShelfFrameDip.Width >= row,
        $"frame {NotchGeometry.ShelfFrameDip.Width} is narrower than one row ({row})");
}
```

- [x] **Step 2: Run the test and verify it fails**

Run: `dotnet test Plith.slnx -m:1 --filter "FullyQualifiedName~NotchGeometryTests"`
Expected: FAIL, compile error, `ShelfTilesPerRow` and the other members do not exist.

- [x] **Step 3: Add the constants and derive the height**

In `src/Plith/Views/Presentation/NotchGeometry.cs`, replace the `ShelfFrameDip` declaration with:

```csharp
/// <summary>Tiles in one row of the shelf grid.</summary>
public const int ShelfTilesPerRow = 5;

/// <summary>Rows the shelf draws. Every row is drawn; nothing folds and nothing scrolls.</summary>
public const int ShelfRowCount = 3;

/// <summary>
/// The most files the shelf holds, defined as exactly what the grid can draw.
///
/// This is the whole guarantee of the flat shelf: a file that is on the shelf is on the screen,
/// in the UIA tree, and reachable by a key. The stack model had a cap of 20 against a surface
/// that could draw 10, and the other ten sat behind count chips that no screen reader could see.
/// Defining the cap as the product rather than typing 15 is what keeps the two from drifting.
/// </summary>
public const int ShelfCapacity = ShelfTilesPerRow * ShelfRowCount;

/// <summary>One tile, square, in DIP.</summary>
public const double ShelfTileSize = 64;

/// <summary>The gap between tiles, in DIP, horizontally and vertically.</summary>
public const double ShelfGap = 8;

/// <summary>
/// Everything in the shelf frame that is not the tile grid: the header with its clear control,
/// the window padding and the border.
///
/// DERIVED, but from two measured parts rather than from a guess. The frame was 384 x 224, and
/// the comment that number carried decomposed it: the surface wants 210 DIP of content plus 14
/// DIP of margin. Of that 210, the tile columns were 149 (a 13 DIP stack caption, two 64 DIP rows
/// and one 8 DIP gap), which leaves 61 for the header and padding. So 61 + 14.
/// </summary>
private const double ShelfChromeDip = 61 + 14;

/// <summary>
/// The shelf frame, in DIP.
///
/// It lives in NotchGeometry because both ends of the wire need it and this file is already
/// linked into the catcher: Plith computes the rectangle it hands over, the catcher's
/// ShelfWindow.xaml declares the same numbers as its design size, and a third copy in a
/// service on Plith's side would be the one free to drift.
///
/// The height is an EXPRESSION over the grid above, not a literal. A literal is free to stop
/// matching the number of rows beside it, silently, and the result is a row drawn outside the
/// window.
/// </summary>
public static readonly Size ShelfFrameDip = new(
    384,
    ShelfRowCount * ShelfTileSize + (ShelfRowCount - 1) * ShelfGap + ShelfChromeDip);
```

- [x] **Step 4: Run the test and verify it passes**

Run: `dotnet test Plith.slnx -m:1 --filter "FullyQualifiedName~NotchGeometryTests"`
Expected: PASS. `ShelfFrameDip.Height` is now `3*64 + 2*8 + 75 = 283`.

- [x] **Step 5: Match the window's design size**

`ShelfWindow.xaml` declares the same numbers as its design size (see the comment quoted above). Find them and update the height to `283`:

Run: `grep -n "224\|384" src/Plith.DropCatcher/Shelf/ShelfWindow.xaml`

Change the height literal from `224` to `283`. Leave the width at `384`.

- [x] **Step 6: Build and commit**

```bash
dotnet build Plith.slnx -m:1
git add src/Plith/Views/Presentation/NotchGeometry.cs src/Plith.DropCatcher/Shelf/ShelfWindow.xaml tests/Plith.Tests/NotchGeometryTests.cs
git commit -m "feat(shelf): define the cap as what the grid draws"
```

---

### Task 2: ShelfStore becomes one flat list

> **Absorbed `ShelfSession` while being executed.** The store has five consumers in that file and
> the task could not compile without them. See the status section above.

**Files:**
- Modify: `src/Plith/Services/Shelf/ShelfStore.cs`
- Modify: `src/Plith/Services/Shelf/ShelfSession.cs` (the log line, `SendStacks`, and the `NewStack`, `Restack` and `PruneEmptyStacks` cases)
- Modify: `src/Plith/Services/Shelf/DropChannel.cs` (mark the two verbs inert)
- Test: `tests/Plith.Tests/ShelfStoreTests.cs`, `tests/Plith.Tests/ShelfSessionTests.cs`

**Interfaces:**
- Consumes: `NotchGeometry.ShelfCapacity` from Task 1.
- Produces: `ShelfStore.Items` (`IReadOnlyList<ShelfItem>`, newest first), `ShelfStore.MaxItems` (int, now `NotchGeometry.ShelfCapacity`), `Add(IEnumerable<string>)`, `Remove(string)`, `RemoveMany(IEnumerable<string>)`, `Clear()`. **Removed:** `Stacks`, `NewStack()`, `Restack(int, IEnumerable<string>)`, `PruneEmptyStacks()`.

- [x] **Step 1: Write the failing tests**

Add to `tests/Plith.Tests/ShelfStoreTests.cs`:

```csharp
/// <summary>
/// One flat list, newest first. The stack model put a new item at the front of the front stack;
/// with no stacks there is one front.
/// </summary>
[Fact]
public void Add_PutsTheNewestFirst()
{
    var store = new ShelfStore(_storePath);
    var a = MakeFile("a.txt");
    var b = MakeFile("b.txt");

    store.Add([a]);
    store.Add([b]);

    Assert.Equal([b, a], store.Items.Select(i => i.Path));
}

/// <summary>
/// The cap is the surface's capacity, so a file that is on the shelf is on the screen. Past it
/// the oldest falls off, which is the end of the list.
/// </summary>
[Fact]
public void Add_EvictsTheOldestPastTheCap()
{
    var store = new ShelfStore(_storePath);
    var paths = Enumerable.Range(0, ShelfStore.MaxItems + 1)
                          .Select(i => MakeFile($"f{i}.txt"))
                          .ToList();

    foreach (var path in paths) store.Add([path]);

    Assert.Equal(ShelfStore.MaxItems, store.Items.Count);
    Assert.DoesNotContain(paths[0], store.Items.Select(i => i.Path));
    Assert.Contains(paths[^1], store.Items.Select(i => i.Path));
}

/// <summary>
/// A file written by the stack build separated its groups with blank lines. Loading skips them,
/// so an existing shelf survives the change as one flat list in the order it already had, and
/// no migration code is needed.
/// </summary>
[Fact]
public void Load_FlattensAFileWrittenWithStackSeparators()
{
    var a = MakeFile("a.txt");
    var b = MakeFile("b.txt");
    var c = MakeFile("c.txt");
    File.WriteAllLines(_storePath, [a, b, string.Empty, c]);

    var store = new ShelfStore(_storePath);

    Assert.Equal([a, b, c], store.Items.Select(i => i.Path));
}

/// <summary>
/// And it writes the new form: one path per line, no blank lines at all.
/// </summary>
[Fact]
public void Save_WritesOnePathPerLineWithNoBlankLines()
{
    var store = new ShelfStore(_storePath);
    store.Add([MakeFile("a.txt")]);
    store.Add([MakeFile("b.txt")]);

    var lines = File.ReadAllLines(_storePath);

    Assert.Equal(2, lines.Length);
    Assert.DoesNotContain(lines, string.IsNullOrWhiteSpace);
}
```

- [x] **Step 2: Run the tests and verify they fail**

Run: `dotnet test Plith.slnx -m:1 --filter "FullyQualifiedName~ShelfStoreTests"`
Expected: FAIL. The new tests fail and the existing stack tests still compile against `Stacks`.

- [x] **Step 3: Flatten the store**

In `src/Plith/Services/Shelf/ShelfStore.cs`:

Replace the field and cap:

```csharp
/// <summary>
/// The shelf is a staging area, not an archive. The cap is exactly what the surface can draw
/// (see NotchGeometry.ShelfCapacity), so nothing is ever on the shelf and off the screen. Past
/// it the oldest falls off.
/// </summary>
public const int MaxItems = NotchGeometry.ShelfCapacity;

private readonly string _storePath;
private readonly List<ShelfItem> _items = [];
```

Add `using Plith.Views.Presentation;` at the top.

Delete the `Stacks` property. Replace `Items`:

```csharp
/// <summary>Everything on the shelf, newest first.</summary>
public IReadOnlyList<ShelfItem> Items => _items;
```

Rewrite `Add`'s body:

```csharp
public void Add(IEnumerable<string> paths)
{
    var kept = false;

    foreach (var path in paths)
    {
        if (!TryResolve(path, out var item)) continue;

        // Removed before inserting, so a repeat drop moves the row to the front rather than
        // leaving a second copy behind.
        _items.RemoveAll(i => string.Equals(i.Path, item.Path, StringComparison.OrdinalIgnoreCase));
        _items.Insert(0, item);
        kept = true;
    }

    if (!kept) return;

    TrimToCap();
    Save();
    Changed?.Invoke();
}
```

Rewrite `RemoveMany`:

```csharp
public void RemoveMany(IEnumerable<string> paths)
{
    var removed = 0;
    foreach (var path in paths)
        removed += _items.RemoveAll(i => string.Equals(i.Path, path, StringComparison.OrdinalIgnoreCase));

    if (removed == 0) return;

    Save();
    Changed?.Invoke();
}
```

Delete `NewStack`, `Restack` and `PruneEmptyStacks` entirely.

Rewrite `Clear`, `TrimToCap`, `Save` and `Load`:

```csharp
public void Clear()
{
    if (_items.Count == 0) return;

    _items.Clear();
    Save();
    Changed?.Invoke();
}

/// <summary>Oldest first off, which is the end of a newest-first list.</summary>
private void TrimToCap()
{
    if (_items.Count <= MaxItems) return;
    _items.RemoveRange(MaxItems, _items.Count - MaxItems);
}

/// <summary>
/// One path per line, newest first. Still a text file a person can read and edit in Notepad,
/// which was a deliberate property before stacks and survives their removal.
/// </summary>
private void Save()
{
    try
    {
        var directory = Path.GetDirectoryName(_storePath);
        if (directory is not null) Directory.CreateDirectory(directory);

        File.WriteAllLines(_storePath, _items.Select(i => i.Path));
    }
    catch (IOException) { /* a shelf that cannot persist still works for this session */ }
    catch (UnauthorizedAccessException) { }
}

private void Load()
{
    string[] lines;
    try
    {
        if (!File.Exists(_storePath)) return;
        lines = File.ReadAllLines(_storePath);
    }
    catch (IOException) { return; }
    catch (UnauthorizedAccessException) { return; }

    foreach (var line in lines)
    {
        // A blank line was the stack separator in the previous format. Skipping it is the whole
        // of the migration: an existing shelf loads as one flat list in the order it had.
        if (string.IsNullOrWhiteSpace(line)) continue;
        if (_items.Count >= MaxItems) break;

        // The same check as on the way in, because a staged file can be deleted or moved between
        // sessions and a row that opens nothing is worse than no row.
        if (!TryResolve(line, out var item)) continue;

        _items.Add(item);
    }
}
```

- [x] **Step 4: Delete the stack tests**

In `tests/Plith.Tests/ShelfStoreTests.cs`, delete every test that calls `NewStack`, `Restack`, `PruneEmptyStacks` or reads `Stacks`. Find them with:

Run: `grep -n "NewStack\|Restack\|PruneEmptyStacks\|\.Stacks" tests/Plith.Tests/ShelfStoreTests.cs`

Keep every test that exercises `Add`, `Remove`, `RemoveMany`, `Clear`, `Items`, `TryResolve` behaviour and persistence, adjusting any that assert through `Stacks` to assert through `Items`.

- [x] **Step 5: Run the tests and verify they pass**

Run: `dotnet test Plith.slnx -m:1 --filter "FullyQualifiedName~ShelfStoreTests"`
Expected: PASS.

- [x] **Step 6: Commit**

```bash
git add src/Plith/Services/Shelf/ShelfStore.cs tests/Plith.Tests/ShelfStoreTests.cs
git commit -m "feat(shelf): make the store one flat list"
```

---

### Task 3: The catcher goes flat, in one step

The model, the wiring and the surface move together. This is Tasks 3 and 4 of the original plan,
merged for the reason in the status section above: no ordering of them leaves the branch working
in between.

**Files:**
- Modify: `src/Plith.DropCatcher/Shelf/ShelfModel.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs` (constants around lines 35 to 57, `VisibleRowsShown` around 366, `Render` around 271 and 397, `BuildColumn` around 742, `BuildOverflowTile` around 1105, `BuildNewStackZone` around 631, `TargetStackIndex` around 593, `OnColumnsDrop` around 585)
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml` (the new-stack button around line 68)
- Modify: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs` (`SetStack` around 486, the two event bridges around 276)
- Modify: `src/Plith.DropCatcher/Shelf/ShelfActions.cs`
- Modify: `src/Plith.DropCatcher/App.xaml.cs` (the probe fixture around 88, the `Items` handler around 151, the two wire bridges around 183)
- Modify: `src/Plith/Services/Shelf/DropChannel.cs` (delete the two inert verbs)
- Modify: `src/Plith/Services/Shelf/ShelfSession.cs` (the stack total in `SendItems`)
- Test: `tests/Plith.Tests/ShelfModelTests.cs`, `tests/Plith.Tests/DropChannelTests.cs`

**Interfaces:**
- Consumes: `NotchGeometry.ShelfCapacity` / `ShelfTilesPerRow` / `ShelfRowCount` / `ShelfTileSize` / `ShelfGap` (Task 1), `ShelfStore.Items` (Task 2).
- Produces: `ShelfModel.SetItems(IReadOnlyList<string>)`, `ShelfModel.Items`, a surface with one named element per file and no count chip, and a `DropVerb` of ten members.

- [ ] **Step 1: Write the failing model tests**

Use the three tests written out in the original Task 3 Step 1 below, unchanged: `SetItems_ReplacesTheListOutright`, `SetItems_TruncatesToTheShelfCapacity`, `SetItems_DropsASelectionThatIsNoLongerPresent`.

- [ ] **Step 2: Run them and verify they fail**

Run: `dotnet test Plith.slnx -m:1 --filter "FullyQualifiedName~ShelfModelTests"`
Expected: FAIL, compile error, `SetItems` does not exist.

- [ ] **Step 3: Flatten the model**

Use the `SetItems` implementation written out in the original Task 3 Step 3 below, deleting `MaxStacks`, `_slots`, `_expected`, `Stacks`, `IsComplete` and `SetStack`.

- [ ] **Step 4: Follow the compiler through the catcher**

Run: `dotnet build Plith.slnx -m:1`

Fix each error in turn. `ShelfWindow.SetStack` becomes `SetItems`; `App.xaml.cs`'s `Items` handler calls it with `message.Paths` alone; the probe fixture becomes one flat list; the `NewStackRequested` and `RestackRequested` bridges and their `ShelfActions` members go.

- [ ] **Step 5: Replace the columns with a wrapping grid**

Use the original Task 4, Steps 1, 2, 3, 5 below, which are written out in full: delete the column machinery, build the `WrapPanel`, make keyboard navigation linear, and remove the within-surface drop handling. **Keep `Background = Brushes.Transparent` on the tile** for the reason §3.10 records.

- [ ] **Step 6: Remove the new-stack control**

Use the original Task 4 Step 4 below.

- [ ] **Step 7: Delete the two inert verbs and fix the stack total**

In `DropChannel.cs` delete `NewStack` and `Restack`. In `ShelfSession.SendItems`, change `y: 1` to `y: 0` and delete the comment explaining why it was 1.

- [ ] **Step 8: Run everything and commit**

```bash
dotnet build Plith.slnx -m:1
dotnet test Plith.slnx -m:1
pwsh -NoProfile -File scripts/render-widgets.ps1
pwsh -NoProfile -File scripts/check-contrast.ps1
git add -A
git commit -m "feat(shelf): draw one wrapping grid, not columns"
```

Expected: build 0 errors, all tests pass, `render-widgets.ps1` green including its `tile-hit` check.

---

#### Reference: the original Task 3 and Task 4, for the code the steps above point at


##### Original Task 3: One Items message, on both ends of the wire

The protocol change and the model that reads it ship together, because a one-message sender against a multi-message reader compiles and fails only at runtime.

**Files:**
- Modify: `src/Plith/Services/Shelf/DropChannel.cs` (the `DropVerb` enum, around line 6)
- Modify: `src/Plith/Services/Shelf/ShelfSession.cs` (`SendStacks` around line 219, `Open` around line 109, the log line around 126, and the `NewStack` / `Restack` cases around 175 to 185)
- Modify: `src/Plith.DropCatcher/Shelf/ShelfModel.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfActions.cs` (drop the restack and new-stack actions)
- Test: `tests/Plith.Tests/ShelfModelTests.cs`, `tests/Plith.Tests/ShelfSessionTests.cs`, `tests/Plith.Tests/DropChannelTests.cs`

**Interfaces:**
- Consumes: `NotchGeometry.ShelfCapacity` (Task 1), `ShelfStore.Items` (Task 2).
- Produces: `ShelfModel.SetItems(IReadOnlyList<string> paths)`, `ShelfModel.Items` (`IReadOnlyList<ShelfEntry>`). **Removed:** `ShelfModel.SetStack`, `ShelfModel.Stacks`, `ShelfModel.IsComplete`. `DropVerb` without `NewStack` and `Restack`.

- [ ] **Step 1: Write the failing tests**

Replace the multi-message assembly tests in `tests/Plith.Tests/ShelfModelTests.cs` with:

```csharp
/// <summary>One message carries the whole shelf, so there is no assembly to get wrong.</summary>
[Fact]
public void SetItems_ReplacesTheListOutright()
{
    var model = new ShelfModel();

    model.SetItems([@"C:\a.txt", @"C:\b.txt"]);
    model.SetItems([@"C:\c.txt"]);

    Assert.Equal([@"C:\c.txt"], model.Items.Select(e => e.Path));
}

/// <summary>
/// The pipe's ACL is open to every process on the machine, so the path list is a claim. It is
/// truncated to what the surface can draw rather than trusted: a message declaring thousands of
/// paths must not become thousands of tiles.
/// </summary>
[Fact]
public void SetItems_TruncatesToTheShelfCapacity()
{
    var model = new ShelfModel();
    var many = Enumerable.Range(0, NotchGeometry.ShelfCapacity + 50)
                         .Select(i => $@"C:\f{i}.txt")
                         .ToList();

    model.SetItems(many);

    Assert.Equal(NotchGeometry.ShelfCapacity, model.Items.Count);
}

/// <summary>
/// A path that has gone away since Plith sent it must not stay selected, or a drag would carry
/// a file that is not on the shelf.
/// </summary>
[Fact]
public void SetItems_DropsASelectionThatIsNoLongerPresent()
{
    var model = new ShelfModel();
    model.SetItems([@"C:\a.txt", @"C:\b.txt"]);
    model.Select(@"C:\a.txt", additive: false);

    model.SetItems([@"C:\b.txt"]);

    Assert.Empty(model.Selection);
}
```

Add `using Plith.Views.Presentation;` to the test file.

- [ ] **Step 2: Run the tests and verify they fail**

Run: `dotnet test Plith.slnx -m:1 --filter "FullyQualifiedName~ShelfModelTests"`
Expected: FAIL, compile error, `SetItems` and `Items` do not exist.

- [ ] **Step 3: Flatten the model**

In `src/Plith.DropCatcher/Shelf/ShelfModel.cs`, delete `MaxStacks`, `_slots`, `_expected`, `Stacks`, `IsComplete` and `SetStack`. Replace with:

```csharp
private readonly List<ShelfEntry> _items = [];
private readonly HashSet<string> _selection = new(StringComparer.OrdinalIgnoreCase);

/// <summary>What the catcher believes is on the shelf, newest first.</summary>
public IReadOnlyList<ShelfEntry> Items => _items;

public IReadOnlyCollection<string> Selection => _selection;

/// <summary>
/// Replace the shelf with what Plith just sent.
///
/// One message carries the whole shelf, which is why this is a replace and not an assembly. The
/// stack build sent one message per stack and had to defend against two deliveries interleaving
/// on a pipe any local process may write: position came from a declared index rather than from
/// arrival order, a message was accepted only into the delivery being assembled, and the stack
/// total was clamped so a stranger could not make the catcher allocate on its say-so. None of
/// that has anything to defend any more.
///
/// The hostile input does not go away, it gets smaller. The list is TRUNCATED to what the
/// surface can draw, which bounds a list that has already arrived rather than an allocation made
/// ahead of it.
/// </summary>
public void SetItems(IReadOnlyList<string> paths)
{
    _items.Clear();
    foreach (var path in paths)
    {
        if (_items.Count >= NotchGeometry.ShelfCapacity) break;
        _items.Add(Describe(path));
    }

    // A path that has gone away since Plith sent it stays selected otherwise, and a drag would
    // then carry a file that is not on the shelf any more.
    _selection.RemoveWhere(p => !_items.Any(e => PathEquals(e.Path, p)));
}
```

Add `using Plith.Views.Presentation;`.

- [ ] **Step 4: Remove the two verbs and send one message**

In `src/Plith/Services/Shelf/DropChannel.cs`, delete the `NewStack` and `Restack` members from `DropVerb`, with their doc comments.

In `src/Plith/Services/Shelf/ShelfSession.cs`:

Replace `SendStacks` with:

```csharp
/// <summary>
/// The whole shelf in one message. Empty is a message too: the surface must be told the shelf is
/// empty, not left holding what it had.
/// </summary>
private void SendItems()
    => Send(DropVerb.Items, [.. _store.Items.Select(item => item.Path)], x: 0, y: 0);
```

Change the call in `Open` from `SendStacks();` to `SendItems();`, and every other call site. Find them with:

Run: `grep -n "SendStacks" src/Plith/Services/Shelf/ShelfSession.cs`

Change the log line around 126 from `with {_store.Stacks.Count} stack(s)` to `with {_store.Items.Count} item(s)`.

Delete the `case DropVerb.NewStack:` and `case DropVerb.Restack:` blocks from the verb switch.

- [ ] **Step 5: Update the catcher's read side and actions**

Run: `grep -rn "SetStack\|IsComplete\|\.Stacks\|NewStack\|Restack" src/Plith.DropCatcher/`

Replace every `SetStack(index, total, paths)` call with `SetItems(paths)`, every `model.Stacks` read with `model.Items`, and delete the restack and new-stack members of `ShelfActions.cs` and their raisers. Anything reading `IsComplete` to decide whether to draw now draws on every message, because one message is the whole shelf.

- [ ] **Step 6: Run the tests and verify they pass**

Run: `dotnet test Plith.slnx -m:1`
Expected: PASS. Delete any remaining test in `ShelfSessionTests.cs` or `DropChannelTests.cs` that names `NewStack` or `Restack`; find them with `grep -n "NewStack\|Restack" tests/Plith.Tests/*.cs`.

- [ ] **Step 7: Commit**

```bash
git add src/Plith/Services/Shelf/DropChannel.cs src/Plith/Services/Shelf/ShelfSession.cs src/Plith.DropCatcher/Shelf/ShelfModel.cs src/Plith.DropCatcher/Shelf/ShelfActions.cs tests/Plith.Tests/
git commit -m "feat(shelf): send the whole shelf in one message"
```

---

##### Original Task 4: The surface draws a wrapping grid

**Files:**
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs` (constants around lines 35 to 57, `VisibleRowsShown` around 366, `BuildColumn` around 742, `BuildOverflowTile` around 1105)
- Modify: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs` (keyboard navigation, the new-stack control)
- Modify: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml` (header)

**Interfaces:**
- Consumes: `ShelfModel.Items`, `NotchGeometry.ShelfTilesPerRow` / `ShelfRowCount` / `ShelfTileSize` / `ShelfGap` (Task 1).
- Produces: a surface whose UIA tree contains one named element per file and no count chip.

- [ ] **Step 1: Delete the column machinery**

In `ShelfSurface.xaml.cs` delete: `VisibleColumns`, `VisibleRows`, `CaptionHeight`, `TileSize`, `Gap` (the last two move to `NotchGeometry` in Task 1, so replace their uses with `NotchGeometry.ShelfTileSize` and `NotchGeometry.ShelfGap`), `VisibleRowsShown`, `BuildColumn`, `BuildOverflowTile`, the stack caption `TextBlock`, and the column wrapper `NamedBorder` that carries the `Stack N, M items` automation name.

- [ ] **Step 2: Build the grid**

Replace the per-column construction in `Render` with a `WrapPanel`, which wraps at the width the frame already gives it:

```csharp
// A WrapPanel rather than a Grid with computed row and column indices: the row break is a
// function of width, and letting the panel do it keeps ONE definition of where a row ends.
// The stack build had that arithmetic in two places (construction and keyboard navigation) and
// a rule that said "at most two tiles" while the code drew one, which is what made two
// verification items report the product broken when the fixture was wrong.
var grid = new WrapPanel
{
    Width = NotchGeometry.ShelfTilesPerRow * NotchGeometry.ShelfTileSize
          + (NotchGeometry.ShelfTilesPerRow - 1) * NotchGeometry.ShelfGap,
    Orientation = Orientation.Horizontal,
};

for (var i = 0; i < model.Items.Count; i++)
    grid.Children.Add(BuildTile(model.Items[i], model.Selection.Contains(model.Items[i].Path), i));
```

Change `BuildTile`'s signature from `BuildTile(ShelfEntry entry, bool selected, bool last, int stackIndex, int rowIndex)` to `BuildTile(ShelfEntry entry, bool selected, int index)`, and give every tile a uniform right and bottom margin of `NotchGeometry.ShelfGap` so the wrap spacing is the gap:

```csharp
Margin = new Thickness(0, 0, NotchGeometry.ShelfGap, NotchGeometry.ShelfGap),
```

**Keep `Background = Brushes.Transparent` on the tile.** WPF hit-tests a Transparent brush and not a null one, and its absence is the defect §3.10 records: a tile that answered a pointer only where its icon or label painted and was dead at its exact centre. `render-widgets.ps1` has a `tile-hit` check that fails the build if it returns.

- [ ] **Step 3: Make keyboard navigation linear**

In `ShelfWindow.xaml.cs`, replace the column-and-row navigation with an index over `Items`: Left and Right move by one, Up and Down move by `NotchGeometry.ShelfTilesPerRow`, all clamped to the list. Find the current handlers with:

Run: `grep -n "Key.Left\|Key.Right\|Key.Up\|Key.Down\|ResolveFocus" src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`

Keyboard focus lives on `ShelfWindow`, not on the page, and the page is driven through forwarded keys. That arrangement is deliberate and does not change here; only the arithmetic does.

- [ ] **Step 4: Remove the new-stack control**

Run: `grep -n "Start a new stack\|NewStack" src/Plith.DropCatcher/Shelf/ShelfWindow.xaml src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs src/Plith.DropCatcher/Shelf/ShelfSurface.xaml`

Delete the control, its handler and its automation name. Keep the clear control.

- [ ] **Step 5: Remove within-surface drop handling**

The only drag out of a tile now goes to another application. Delete the drop-on-column hit testing and the drag-target highlight; keep `DoDragDrop` and the `ShelfModel.DragPaths` call that feeds it.

Run: `grep -n "DragOver\|DragEnter\|Drop\b\|AllowDrop" src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`

- [ ] **Step 6: Build, render and commit**

```bash
dotnet build Plith.slnx -m:1
pwsh -NoProfile -File scripts/render-widgets.ps1
pwsh -NoProfile -File scripts/check-contrast.ps1
git add src/Plith.DropCatcher/Shelf/
git commit -m "feat(shelf): draw one wrapping grid, not columns"
```

Expected: build 0 errors, `render-widgets.ps1` green including its `tile-hit` check, `check-contrast.ps1` exit 0.

---


---

### Task 4: The notch page, and the count that reached nobody

**Files:**
- Modify: `src/Plith/Views/Widgets/ShelfWidget.cs` (`BuildOverflowTile` around line 382, the `Tiles` automation name)
- Modify: `scripts/check-a11y.ps1` (remove one suppression)

**Interfaces:**
- Consumes: `ShelfStore.Items` (Task 2).
- Produces: a notch page whose item count is in the UIA tree.

- [ ] **Step 1: Take the flat list**

`ShelfWidget` reads `ShelfStore.Items` already. Remove any remaining stack awareness:

Run: `grep -n "Stacks\|stack" src/Plith/Views/Widgets/ShelfWidget.cs`

Keep `BuildOverflowTile`. The notch frame is 356 DIP wide and cannot show 15 tiles, and unlike the shelf a file hidden here is one click from the full list, so a count is honest here and was not on the shelf.

- [ ] **Step 2: Write the failing check**

The defect: `AutomationProperties.SetName(Tiles, "Shelf, N items")` sets a name on a `StackPanel`, which WPF gives no automation peer, so the count reaches no screen reader. Confirmed against the live UIA tree in `docs/SHELF-VERIFICATION.md` §3.11 and filed in §5.4.

Run: `pwsh -NoProfile -File scripts/check-a11y.ps1`
Expected: the output still lists `KNOWN GAP, not fixed here: ... AutomationProperties.SetName(Tiles, ...) targets a StackPanel`.

- [ ] **Step 3: Move the name onto an element with a peer**

Wrap `Tiles` in the `Border` that already surrounds the row, or give the row a `Border` host, and set the name there instead. A `Border` gets an automation peer; a `StackPanel` does not.

```csharp
// The name goes on a Border and not on the StackPanel it wraps. WPF gives a StackPanel no
// automation peer at all, so a name set on one is in no UIA tree and no screen reader ever
// hears it. This was filed from reading the code and then confirmed against the running tree:
// with five files on the shelf page the names present were the five file names, their type
// chips and the open hint, and "Shelf, 5 items" appeared nowhere.
AutomationProperties.SetName(TilesHost, announced);
```

- [ ] **Step 4: Remove the suppression**

In `scripts/check-a11y.ps1`, find the suppression naming `ShelfWidget.cs` and `SetName(Tiles, ...)` and delete that entry. Leave the sibling suppression for `SetName(tile, ...)` targeting a `Border`, which predates this work and is not touched by it.

Run: `grep -n "Tiles" scripts/check-a11y.ps1`

- [ ] **Step 5: Verify and commit**

```bash
pwsh -NoProfile -File scripts/check-a11y.ps1
dotnet build Plith.slnx -m:1
git add src/Plith/Views/Widgets/ShelfWidget.cs scripts/check-a11y.ps1
git commit -m "fix(shelf): let the notch page count reach a reader"
```

Expected: `check-a11y.ps1` exit 0, and the `SetName(Tiles, ...)` line is gone from its output.

---

### Task 5: The driver, and the documents

**Files:**
- Modify: `scripts/drive-shelf-pair.ps1`
- Modify: `docs/SHELF-VERIFICATION.md`
- Modify: `CLAUDE.md`, `docs/ROADMAP.md`

**Interfaces:**
- Consumes: everything above.
- Produces: a driver whose fixture is a flat list and which asserts the guarantee the change exists for.

- [ ] **Step 1: Flatten the fixture**

In `scripts/drive-shelf-pair.ps1`, the fixture seeds `shelf.txt` with two stacks of two separated by a blank line. Make it a flat list of four paths. Delete `Format-Shelf`'s bracket grouping and `Read-Shelf`'s blank-line handling, so both speak in one list.

The overflow-rule comment block above the steps goes with the stacks; so does the note about a stack of three drawing one tile.

- [ ] **Step 2: Delete the stack steps**

Delete the `3.5`, `3.6` and `3.4` step blocks entirely. Keep `3.1`, `3.2` and `3.3`, adjusting their target file names since the fixture no longer restacks anything before they run.

- [ ] **Step 3: Add the check the change exists for**

```powershell
# --- the guarantee: every file on the shelf is IN THE TREE ------------------------------------
# This is the whole point of the flat shelf. The stack build could hold 20 files against a
# surface that drew 10, and a folded tile was in no UIA tree, unreachable by key and invisible
# to a screen reader. Seed the shelf to capacity and require every name to be present.
$names = Get-Names -Hwnd (Find-ShelfWindow).Hwnd
$missing = @($fixtureNames | Where-Object { $names -notcontains $_ })
Add-Verdict 'every file on a full shelf is in the UIA tree' ($missing.Count -eq 0) `
    "$($fixtureNames.Count) seeded, missing: $(if ($missing) { $missing -join ', ' } else { 'none' })"
```

Seed the fixture to `NotchGeometry.ShelfCapacity` files for this check.

- [ ] **Step 4: Update the documents**

In `docs/SHELF-VERIFICATION.md`, delete §3.4, §3.5, §3.6 and the restack items §4.5, §4.6, §4.9, and strike the §3 status table rows for them. §3.12 stays as written: it records what the code did on the day, and it already says this section would outlive the behaviour.

In `CLAUDE.md`, replace the "Stacks are slated for removal" paragraph with what actually shipped.

In `docs/ROADMAP.md`, Phase 7 describes slice 2 as delivering stacks. Add the slice 3 entry rather than rewriting history.

- [ ] **Step 5: Run everything**

```bash
dotnet build Plith.slnx -m:1
dotnet test Plith.slnx -m:1
pwsh -NoProfile -File scripts/check-a11y.ps1
pwsh -NoProfile -File scripts/check-shared-xaml.ps1
pwsh -NoProfile -File scripts/check-contrast.ps1
pwsh -NoProfile -File scripts/render-widgets.ps1
```

Expected: build 0 errors 0 warnings, all tests pass, every lint exit 0.

- [ ] **Step 6: Drive it on hardware**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/drive-shelf-pair.ps1`

**Preconditions, each of which has cost a run (see the script's own header):** the session must be Active, nothing may cover the monitor, injected input must reach the desktop, and **the pointer must be free**. The script drives the real cursor and cannot share it with a person. A fullscreen game fails three of the four at once.

Expected: §3.1, §3.2, §3.3 and the new tree check all PASS.

- [ ] **Step 7: Confirm the height, which nobody has measured**

`ShelfChromeDip = 75` is derived from the old frame, so the 283 is arithmetic rather than observation. Capture the shelf and look at it:

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/capture-shelf.ps1`

Check that the third row is fully inside the window and that the header is not crowded. If the chrome is not 75, correct `ShelfChromeDip` and note the measured value in its comment.

- [ ] **Step 8: Commit**

```bash
git add scripts/drive-shelf-pair.ps1 docs/ CLAUDE.md
git commit -m "docs(shelf): retire the stack items, record the flat list"
```

---

## Self-Review

**Spec coverage.** Store and persistence: Task 2. Shared constants and the derived frame height: Task 1. Wire protocol, the one-message `Items` collapse and the shelf surface: Task 3, which the two originally separate tasks were merged into. The notch page and its automation-name defect: Task 4. Geometry: Task 1 defines it, Task 5 Step 7 confirms it. What gets deleted: Tasks 2, 3 and 5. Testing: the unit tests live in Tasks 1 to 3, the runtime guarantee check in Task 5 Step 3.

**Risk 4 from the spec** (`render-widgets.ps1` has structural assumptions about tile nesting) is covered by Task 3 Step 8, which runs it and expects green; the `.Children[0]` path may need adjusting when the tile loses its column parent.

**Type consistency.** `SetItems(IReadOnlyList<string>)` and `Items` are named identically in Task 3's test, implementation and consumers, which now sit in the same task. `NotchGeometry.ShelfCapacity`, `ShelfTilesPerRow`, `ShelfRowCount`, `ShelfTileSize` and `ShelfGap` are spelled the same in Tasks 1, 2 and 3. `ShelfStore.MaxItems` keeps its name and changes only its value source.

**Open and deliberately not decided here:** whether 15 proves too few in use. Raising it means adding a row and letting the frame height follow, not adding scrolling.
