# Shelf Slice 2 Implementation Plan

> **A tick below means the step was taken, NOT that nothing was deviated from.** Every task in
> this plan carries a real deviation somewhere: a placeholder replaced with a measured value, a
> guard added that the step text did not ask for, a file touched that the task's own file list did
> not name. None of that is hidden by a checked box. Deviations, what was measured versus reasoned
> about, and what still has not been driven on hardware all live in `docs/SHELF-VERIFICATION.md`
> and in the code that carries them, not in this checklist. Read a checked box as "this step was
> executed", never as "this step's plan text turned out to be exactly right."

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A person opens the shelf from the notch, sees what is on it with real file icons, selects one or several items, drags them out to any ordinary application, removes rows, clears the shelf, and keeps items in stacks that do not mix.

**Architecture:** The shelf's interactive surface moves into `Plith.DropCatcher`, because Task 8 measured that `DoDragDrop` never delivers a drag for a press that landed in another process, and the catcher is the only process that can be the source. Plith keeps `ShelfStore` and stays the only writer of `shelf.txt`, so the shelf survives the catcher not running. The two talk over the existing named pipe, with new verbs in the existing text format.

**Tech Stack:** WPF / .NET 10, `System.IO.Pipes`, `SHGetFileInfo` via P/Invoke, the existing `DropChannel` wire and `NotchGeometry`, xUnit for the parts a headless suite can reach.

**Spec:** `docs/superpowers/specs/2026-09-18-shelf-slice-2-design.md`

## Global Constraints

- All code, comments, documentation and commit messages in English. Conventional Commits. No AI attribution anywhere.
- Target `net10.0-windows10.0.22000.0`, x64, `Nullable=enable`.
- Build with `-m:1`. Never delete `obj/`, it silently omits BAML.
- `tests/Plith.Installer.Tests` must not be modified (three known pre-existing CA1861 warnings).
- `scripts/check-a11y.ps1`, `scripts/check-shared-xaml.ps1` and `scripts/check-contrast.ps1` must pass before any release build.
- Never delete the `CN=Plith Self-Signed` certificate.
- `Plith.exe` keeps `uiAccess="true"`.
- `Plith.DropCatcher.exe` ships **without** `uiAccess` in every configuration, and is never launched as a child of `Plith.exe`.
- Every icon in the product is drawn geometry, never a Segoe MDL2 glyph. The accessibility lint fails the build on glyphs, in code-behind as well as XAML. The real shell icons in Task 5 are file thumbnails supplied by the shell, which is a different thing from an icon font and is the one exception; Task 5 says how to keep the lint honest about it.

## What changed from the spec while planning

Recorded here rather than left as a silent divergence. The spec is amended to match in the same commit as this plan.

1. **The palette carries seven values, not six.** `OsdSurfaceBrush` is a `LinearGradientBrush`, not a solid colour. One colour cannot reproduce it, and a flat stand-in would be the exact drift the palette section exists to prevent. The payload is surface start, surface end, ink, muted ink, track, accent, theme flag.
2. **A `NewStack` verb exists.** The spec has new stacks created empty and at the front, and `Restack` appending at the end. Those are two different operations and overloading one verb to mean both would be a puzzle at the decode site.
3. **Spec risk 3 is resolved and needs no investigation task.** `OsdHost.OnNotchClicked` is wired at `PreviewMouseLeftButtonDown` but returns early once `notch.IsOpenEnoughToShowContent` (`src/Plith/Views/OsdHost.cs:877`), so an open frame keeps its own clicks. The rail handles its clicks on the rail element with `e.Handled = true` (`src/Plith/Views/Widgets/WidgetFrame.cs:218`). A click on the shelf page body therefore reaches the page. Task 6 uses button **up** on the page.

## What changed while executing

Recorded here rather than by rewriting a task that has already shipped and been reviewed.

1. **The palette carries eight values, not seven.** Task 3 measured the selection ring at 1.25:1
   against the panel for a near-white accent, where this product holds non-text surfaces to 3:1.
   The ring becomes an eighth field that Plith derives with `ContrastInk.RingOn`, which keeps
   the accent when it already clears 3:1 and otherwise walks its lightness with hue and saturation
   preserved. An intermediate round used `TrackOn` and was reverted: it is a function of the
   surface alone and flattened a vivid lime ring from 8.87:1 to 3.03:1. Task 2's text
   below still says seven, which was true when it was written and executed; the spec and
   `ShelfPaletteWire.FieldCount` are the current authority.
2. **The verb check needs both `Enum.IsDefined` and a name round trip.** My pre-flight ruling said
   the round trip alone was enough. It is not: `ToString` on a value with no name prints the
   number, so `"99"` round trips. Task 2's text was corrected before it shipped.
3. **Task 9 also touched `ShelfWindow.xaml.cs`, which its own file list did not name.**
   `ShelfWindow`'s own header comment already says the window holds keyboard focus rather than
   the page inside it; `OnPreviewKeyDown` was already there for Escape. A key cannot reach
   `ShelfSurface` without the window forwarding it, so the plan's file list for that task could
   not be honoured literally. `OnPreviewKeyDown` now forwards every key that is not Escape to
   `ShelfSurface.HandleKey`. See `docs/SHELF-VERIFICATION.md` §5.1.
4. **Task 9's "stack caption" contrast pair needed real code behind it, not just the script
   entry.** The pair the task handed over named `NotchInkMuted` for something `ShelfSurface.cs`
   did not draw yet. Rather than add a measurement of a colour nothing used, `BuildColumn` now
   draws a small "Stack N" caption in that colour, verified by render at 384 x 224 in both themes
   and a white accent before it shipped. See `docs/SHELF-VERIFICATION.md` §5.3.
5. **The selection ring is now measured, not left exempt.** `check-contrast.ps1` calls
   `AccentTheme.Derive`, `AccentTheme.DeriveOsdSurfaces` and `ContrastInk.RingOn` directly (the
   same calls `ShelfSession.DerivePalette` makes) for the same accent/theme matrix as everything
   else it checks, rather than leaving the ring as a colour the sweep cannot see. See
   `docs/SHELF-VERIFICATION.md` §5.2.

## Threat note, to be repeated in code

The pipe's ACL is open to Everyone, by measurement and by necessity. Until this slice the only verb that did anything was `Dropped`, which adds paths that `ShelfStore` then stats. This slice adds `ClearShelf`, `RemoveItems` and `Restack`, so any local process can now rearrange or empty the shelf.

The impact is bounded and is the reason this is accepted rather than redesigned: the shelf holds references, nothing is copied or moved, and clearing it deletes no file. Anything running at Medium can already read and write the user's own files. It is stated in `DropChannelServer`'s summary so the next reader does not have to rediscover the reasoning.

## File Structure

**Plith, shared wire (linked into both projects)**
- `src/Plith/Services/Shelf/DropChannel.cs`, modify: the new verbs, and reject verbs that are not defined.
- `src/Plith/Services/Shelf/ShelfPaletteWire.cs`, create: the seven colours to and from the path list. Linked into the catcher.

**Plith only**
- `src/Plith/Services/Shelf/ShelfStore.cs`, modify: stacks, batch remove, restack, prune.
- `src/Plith/Services/Shelf/ShelfSession.cs`, create: owns the open and close conversation with the catcher, so `OsdHost` gains a call rather than a protocol.
- `src/Plith/Services/Shelf/DropCatcherLauncher.cs`, modify: a distinguishable result.
- `src/Plith/Views/OsdHost.cs`, modify: a reason for standing aside, and the shelf open path.
- `src/Plith/Views/Widgets/ShelfWidget.cs`, modify: the hint, and the click that opens.
- `src/Plith/App.xaml.cs`, modify: construct and wire `ShelfSession`.

**Catcher**
- `src/Plith.DropCatcher/Shelf/ShelfModel.cs`, create: stacks and selection, no WPF types.
- `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml` / `.cs`, create: every visual, as a `UserControl` so the render harness can photograph it.
- `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml` / `.cs`, create: the HWND, activation, growth, dismissal, and the drag source.
- `src/Plith.DropCatcher/Shelf/ShellIcons.cs`, create: `SHGetFileInfo` and a cache.
- `src/Plith.DropCatcher/Shelf/ShelfActions.cs`, create: open, and show in the file manager.
- `src/Plith.DropCatcher/App.xaml.cs`, modify: route the new verbs.
- `src/Plith.DropCatcher/Plith.DropCatcher.csproj`, modify: link `ShelfPaletteWire.cs`.

**Tests**
- `tests/Plith.Tests/ShelfStoreTests.cs`, modify.
- `tests/Plith.Tests/DropChannelTests.cs`, modify.
- `tests/Plith.Tests/ShelfPaletteWireTests.cs`, create.
- `tests/Plith.Tests/ShelfModelTests.cs`, create. `ShelfModel` has no WPF types precisely so it can live here; the test project references Plith, not the catcher, so this file is added to the catcher project as a linked compile item the same way the wire is.

**Scripts and docs**
- `scripts/check-contrast.ps1`, modify: scan the catcher too.
- `scripts/render-widgets.ps1`, modify: render `ShelfSurface`.
- `docs/SHELF-VERIFICATION.md`, create.

---

### Task 1: Stacks in the store

The surface is built against this model, so it goes first. Built afterwards, the surface is built against the flat list and then changed.

**Files:**
- Modify: `src/Plith/Services/Shelf/ShelfStore.cs`
- Test: `tests/Plith.Tests/ShelfStoreTests.cs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ShelfStore.Stacks -> IReadOnlyList<IReadOnlyList<ShelfItem>>` (front stack first, newest item first within a stack); `ShelfStore.Items -> IReadOnlyList<ShelfItem>` (unchanged shape, now the flattening of `Stacks` in order); `ShelfStore.Add(IEnumerable<string>)` (unchanged signature, now joins the front stack); `ShelfStore.RemoveMany(IEnumerable<string>)`; `ShelfStore.NewStack()`; `ShelfStore.Restack(int targetStackIndex, IEnumerable<string> paths)`; `ShelfStore.PruneEmptyStacks()`. `Remove(string)` and `Clear()` keep their signatures.

- [x] **Step 1: Write the failing tests**

Add to `tests/Plith.Tests/ShelfStoreTests.cs`:

```csharp
[Fact]
public void Add_JoinsTheFrontStack()
{
    var store = new ShelfStore(_storePath);

    store.Add([MakeFile("a.txt")]);
    store.Add([MakeFile("b.txt")]);

    Assert.Single(store.Stacks);
    Assert.Equal(2, store.Stacks[0].Count);
    Assert.Equal("b.txt", store.Stacks[0][0].Name);
}

/// <summary>The new stack is empty and at the front, so the NEXT drop joins it. A stack created
/// behind the existing one would only be useful after a drop had already gone to the wrong
/// place.</summary>
[Fact]
public void NewStack_GoesToTheFrontAndTakesTheNextDrop()
{
    var store = new ShelfStore(_storePath);
    store.Add([MakeFile("a.txt")]);

    store.NewStack();
    store.Add([MakeFile("b.txt")]);

    Assert.Equal(2, store.Stacks.Count);
    Assert.Equal("b.txt", store.Stacks[0][0].Name);
    Assert.Equal("a.txt", store.Stacks[1][0].Name);
}

[Fact]
public void Restack_MovesAnItemIntoAnExistingStack()
{
    var store = new ShelfStore(_storePath);
    var a = MakeFile("a.txt");
    store.Add([a]);
    store.NewStack();
    store.Add([MakeFile("b.txt")]);

    store.Restack(0, [a]);

    Assert.Equal(2, store.Stacks[0].Count);
    Assert.Empty(store.Stacks[1]);
}

/// <summary>A target one past the end appends a stack. That is the only way to create a stack at
/// the back, and it is what dragging a tile onto empty space means.</summary>
[Fact]
public void Restack_PastTheEndAppendsANewStack()
{
    var store = new ShelfStore(_storePath);
    var a = MakeFile("a.txt");
    store.Add([a, MakeFile("b.txt")]);

    store.Restack(store.Stacks.Count, [a]);

    Assert.Equal(2, store.Stacks.Count);
    Assert.Single(store.Stacks[1]);
    Assert.Equal("a.txt", store.Stacks[1][0].Name);
}

[Fact]
public void Restack_IgnoresATargetThatIsOutOfRange()
{
    var store = new ShelfStore(_storePath);
    var a = MakeFile("a.txt");
    store.Add([a]);

    store.Restack(7, [a]);
    store.Restack(-1, [a]);

    Assert.Single(store.Stacks);
    Assert.Single(store.Stacks[0]);
}

[Fact]
public void RemoveMany_TakesOutSeveralAndRaisesOneEvent()
{
    var store = new ShelfStore(_storePath);
    var a = MakeFile("a.txt");
    var b = MakeFile("b.txt");
    store.Add([a, b, MakeFile("c.txt")]);
    var events = 0;
    store.Changed += () => events++;

    store.RemoveMany([a, b]);

    Assert.Single(store.Items);
    Assert.Equal(1, events);
}

/// <summary>An empty stack that is still empty when the shelf closes is litter. The control that
/// creates one is useful before a drop, so it cannot refuse to create it, which means something
/// has to clean up after the drop that never came.</summary>
[Fact]
public void PruneEmptyStacks_DropsThemAndKeepsTheRest()
{
    var store = new ShelfStore(_storePath);
    store.Add([MakeFile("a.txt")]);
    store.NewStack();

    store.PruneEmptyStacks();

    Assert.Single(store.Stacks);
    Assert.Equal("a.txt", store.Stacks[0][0].Name);
}

/// <summary>The cap is on the shelf, not on a stack. Twenty items spread over four stacks is the
/// same twenty items.</summary>
[Fact]
public void Add_CapsTheTotalAcrossStacks()
{
    var store = new ShelfStore(_storePath);
    for (var i = 0; i < ShelfStore.MaxItems + 5; i++)
    {
        if (i % 4 == 0) store.NewStack();
        store.Add([MakeFile($"f{i}.txt")]);
    }

    Assert.Equal(ShelfStore.MaxItems, store.Items.Count);
}

[Fact]
public void Save_SeparatesStacksWithABlankLine_AndLoadsThemBack()
{
    var store = new ShelfStore(_storePath);
    store.Add([MakeFile("a.txt")]);
    store.NewStack();
    store.Add([MakeFile("b.txt")]);

    var text = File.ReadAllText(_storePath);
    Assert.Contains(Environment.NewLine + Environment.NewLine, text, StringComparison.Ordinal);

    var reloaded = new ShelfStore(_storePath);
    Assert.Equal(2, reloaded.Stacks.Count);
    Assert.Equal("b.txt", reloaded.Stacks[0][0].Name);
}

/// <summary>Backward compatibility, and it costs nothing: a file written before stacks existed
/// has no blank lines in it, so it loads as exactly one stack with no version marker anywhere.
/// </summary>
[Fact]
public void Load_ReadsAPreStacksFileAsASingleStack()
{
    var a = MakeFile("a.txt");
    var b = MakeFile("b.txt");
    File.WriteAllLines(_storePath, [a, b]);

    var store = new ShelfStore(_storePath);

    Assert.Single(store.Stacks);
    Assert.Equal(2, store.Stacks[0].Count);
}
```

- [x] **Step 2: Run them and watch them fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter ShelfStoreTests -v q -m:1`
Expected: FAIL to compile, `Stacks`, `NewStack`, `Restack`, `RemoveMany` and `PruneEmptyStacks` do not exist.

- [x] **Step 3: Rework `ShelfStore` around a list of stacks**

Replace the `private readonly List<ShelfItem> _items = [];` field and the members that use it. Keep `TryResolve`, `MaxItems`, `DefaultStorePath` and the constructors exactly as they are.

```csharp
    private readonly List<List<ShelfItem>> _stacks = [];

    /// <summary>
    /// Front stack first, newest item first inside a stack.
    ///
    /// A list of lists rather than a ShelfStack type: a stack has no property other than the
    /// items in it, and a wrapper carrying nothing would be a type to keep in step for no
    /// information.
    /// </summary>
    public IReadOnlyList<IReadOnlyList<ShelfItem>> Stacks => _stacks;

    /// <summary>
    /// Every item, flattened in stack order. Kept because the notch's glance page and the
    /// existing tests are written against it, and because the flat view is genuinely what a
    /// five-slot row wants: five slots cannot show grouping, and half-drawn grouping is worse
    /// than none.
    /// </summary>
    public IReadOnlyList<ShelfItem> Items => _stacks.SelectMany(s => s).ToList();

    public void Add(IEnumerable<string> paths)
    {
        var kept = false;

        foreach (var path in paths)
        {
            if (!TryResolve(path, out var item)) continue;

            // The front stack is created LAZILY, on the first path that resolves. Created up
            // front, a drop of nothing but dead paths leaves an empty stack behind that nobody
            // asked for.
            if (_stacks.Count == 0) _stacks.Add([]);
            var front = _stacks[0];

            // Removed from EVERY stack before inserting, not just the front one. A repeat drop
            // moves the row to the front rather than leaving a second copy in another stack,
            // which is the same rule as before stacks existed, applied across all of them.
            foreach (var stack in _stacks)
                stack.RemoveAll(i => string.Equals(i.Path, item.Path, StringComparison.OrdinalIgnoreCase));

            front.Insert(0, item);
            kept = true;
        }

        if (!kept) return;

        TrimToCap();
        Save();
        Changed?.Invoke();
    }

    public void Remove(string path) => RemoveMany([path]);

    public void RemoveMany(IEnumerable<string> paths)
    {
        var removed = 0;
        foreach (var path in paths)
        {
            foreach (var stack in _stacks)
                removed += stack.RemoveAll(i => string.Equals(i.Path, path, StringComparison.OrdinalIgnoreCase));
        }

        if (removed == 0) return;

        Save();
        Changed?.Invoke();
    }

    /// <summary>An empty stack at the front, so the next drop joins it rather than the one
    /// before it. Nothing is persisted for it: an empty stack has no lines to write.</summary>
    public void NewStack()
    {
        if (_stacks.Count > 0 && _stacks[0].Count == 0) return;   // one empty front stack is enough

        _stacks.Insert(0, []);
        Changed?.Invoke();
    }

    /// <summary>
    /// Move items into <paramref name="targetStackIndex"/>. A target equal to the stack count
    /// appends a new stack; anything else out of range is ignored, because the index arrives
    /// from the catcher and is a claim like everything else that does.
    /// </summary>
    public void Restack(int targetStackIndex, IEnumerable<string> paths)
    {
        if (targetStackIndex < 0 || targetStackIndex > _stacks.Count) return;

        var moving = new List<ShelfItem>();
        foreach (var path in paths)
        {
            foreach (var stack in _stacks)
            {
                var found = stack.FindIndex(i => string.Equals(i.Path, path, StringComparison.OrdinalIgnoreCase));
                if (found < 0) continue;

                moving.Add(stack[found]);
                stack.RemoveAt(found);
                break;
            }
        }

        if (moving.Count == 0) return;

        if (targetStackIndex == _stacks.Count) _stacks.Add([]);
        _stacks[targetStackIndex].InsertRange(0, moving);

        Save();
        Changed?.Invoke();
    }

    public void PruneEmptyStacks()
    {
        if (_stacks.RemoveAll(s => s.Count == 0) == 0) return;

        Save();
        Changed?.Invoke();
    }

    public void Clear()
    {
        if (_stacks.Count == 0) return;

        _stacks.Clear();
        Save();
        Changed?.Invoke();
    }

    /// <summary>The cap is on the shelf as a whole. Oldest first, which means from the back of
    /// the last non-empty stack.</summary>
    private void TrimToCap()
    {
        var total = _stacks.Sum(s => s.Count);
        for (var i = _stacks.Count - 1; i >= 0 && total > MaxItems; i--)
        {
            while (_stacks[i].Count > 0 && total > MaxItems)
            {
                _stacks[i].RemoveAt(_stacks[i].Count - 1);
                total--;
            }
        }
    }
```

- [x] **Step 4: Rework `Save` and `Load` for the blank-line format**

```csharp
    /// <summary>
    /// One path per line, newest first, with a blank line between stacks. Still a text file a
    /// person can read and edit in Notepad, which was a deliberate property before stacks and is
    /// not given up for them: a blank line is the one separator that needs no explaining.
    ///
    /// Empty stacks write nothing, so a stack created and never filled leaves no trace in the
    /// file even if PruneEmptyStacks has not run yet.
    /// </summary>
    private void Save()
    {
        try
        {
            var directory = Path.GetDirectoryName(_storePath);
            if (directory is not null) Directory.CreateDirectory(directory);

            var lines = new List<string>();
            foreach (var stack in _stacks.Where(s => s.Count > 0))
            {
                if (lines.Count > 0) lines.Add(string.Empty);
                lines.AddRange(stack.Select(i => i.Path));
            }

            File.WriteAllLines(_storePath, lines);
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

        var current = new List<ShelfItem>();
        var total = 0;

        foreach (var line in lines)
        {
            if (string.IsNullOrWhiteSpace(line))
            {
                // A blank line ends a stack. Consecutive blanks, or a leading one, produce no
                // empty stack: a file edited by hand should not be able to make litter.
                if (current.Count > 0) _stacks.Add(current);
                current = [];
                continue;
            }

            if (total >= MaxItems) break;

            // The same check as on the way in, because a staged file can be deleted or moved
            // between sessions and a row that opens nothing is worse than no row.
            if (!TryResolve(line, out var item)) continue;

            current.Add(item);
            total++;
        }

        if (current.Count > 0) _stacks.Add(current);
    }
```

Add `using System.Linq;` if the file does not already have it through implicit usings.

- [x] **Step 5: Run the tests and watch them pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter ShelfStoreTests -v q -m:1`
Expected: PASS, including the tests that existed before this task.

- [x] **Step 6: Run the whole suite**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`
Expected: PASS. `ShelfWidget` reads `Items`, whose shape did not change.

- [x] **Step 7: Commit**

```bash
git add src/Plith/Services/Shelf/ShelfStore.cs tests/Plith.Tests/ShelfStoreTests.cs
git commit -m "feat(shelf): keep stacks that do not mix, and a file still readable in Notepad"
```

---

### Task 2: The wire

**Files:**
- Modify: `src/Plith/Services/Shelf/DropChannel.cs`
- Create: `src/Plith/Services/Shelf/ShelfPaletteWire.cs`
- Create: `tests/Plith.Tests/ShelfPaletteWireTests.cs`
- Modify: `tests/Plith.Tests/DropChannelTests.cs`
- Modify: `src/Plith.DropCatcher/Plith.DropCatcher.csproj`

**Interfaces:**
- Consumes: `DropMessage`, `DropChannel.Encode`, `DropChannel.TryDecode` as they exist.
- Produces: `DropVerb` gains `OpenShelf, Items, Palette, RemoveItems, ClearShelf, NewStack, Restack, ShelfClosed`; `ShelfPalette(Color SurfaceStart, Color SurfaceEnd, Color Ink, Color InkMuted, Color Track, Color Accent, bool IsDark)`; `ShelfPaletteWire.ToPaths(ShelfPalette) -> IReadOnlyList<string>`; `ShelfPaletteWire.TryFromPaths(IReadOnlyList<string>, out ShelfPalette) -> bool`.

- [x] **Step 1: Write the failing tests for the verbs**

Add to `tests/Plith.Tests/DropChannelTests.cs`:

```csharp
[Theory]
[InlineData(DropVerb.OpenShelf)]
[InlineData(DropVerb.Items)]
[InlineData(DropVerb.Palette)]
[InlineData(DropVerb.RemoveItems)]
[InlineData(DropVerb.ClearShelf)]
[InlineData(DropVerb.NewStack)]
[InlineData(DropVerb.Restack)]
[InlineData(DropVerb.ShelfClosed)]
public void Encode_ThenDecode_RoundTripsEveryShelfVerb(DropVerb verb)
{
    var sent = new DropMessage(verb, 1, 2, 3, 4, ["C:\\a.txt"]);

    Assert.True(DropChannel.TryDecode(DropChannel.Encode(sent), out var back));

    Assert.Equal(verb, back.Verb);
    Assert.Equal(1, back.X);
    Assert.Equal("C:\\a.txt", Assert.Single(back.Paths));
}

/// <summary>
/// The hostile-path test, pointed at the verbs that now DO something.
///
/// Before this slice the only verb carrying paths was Dropped, which adds files that ShelfStore
/// then stats. RemoveItems and Restack change the shelf, so a path able to forge a second
/// message on those lines is a path able to rearrange someone's shelf. The escaping is the same
/// escaping; this test is what keeps it pointed at the verb list as the list grows.
/// </summary>
[Theory]
[InlineData(DropVerb.RemoveItems)]
[InlineData(DropVerb.Restack)]
[InlineData(DropVerb.Items)]
public void Encode_NeutralisesSeparatorsOnEveryVerbThatCarriesPaths(DropVerb verb)
{
    const string hostile = "C:\\a\nClearShelf\t0\t0\t0\t0";

    var encoded = DropChannel.Encode(new DropMessage(verb, 0, 0, 0, 0, [hostile]));

    Assert.DoesNotContain('\n', encoded);
    Assert.True(DropChannel.TryDecode(encoded, out var back));
    Assert.Equal(hostile, Assert.Single(back.Paths));
}

/// <summary>
/// Enum.TryParse accepts a number for any enum, so "9" decoded to whatever verb happened to sit
/// at 9 and a line past the end of the list decoded to a verb that does not exist. The pipe is
/// reachable by any process on this machine, so a verb is exactly the field that must not be
/// guessable by counting.
/// </summary>
[Theory]
[InlineData("3\t0\t0\t0\t0")]
[InlineData("99\t0\t0\t0\t0")]
[InlineData("NotAVerb\t0\t0\t0\t0")]
public void TryDecode_RejectsAVerbThatIsNotOneOfOurs(string line)
    => Assert.False(DropChannel.TryDecode(line, out _));
```

- [x] **Step 2: Run them and watch them fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter DropChannelTests -v q -m:1`
Expected: FAIL to compile on the new verb names, and the numeric-verb theory fails once it does compile.

- [x] **Step 3: Add the verbs, and reject the ones that are not ours**

In `src/Plith/Services/Shelf/DropChannel.cs`, extend the enum. Keep the existing four first so their numeric values do not move, which matters only for a mixed-version pair during development but costs nothing to preserve.

```csharp
public enum DropVerb
{
    /// <summary>The catcher announcing itself. Carries no payload; it exists so Plith knows a
    /// catcher is alive before it hides the notch for one.</summary>
    Hello,

    /// <summary>Take the notch's place: the rectangle travels in X/Y/W/H.</summary>
    Show,

    /// <summary>Give it back.</summary>
    Hide,

    /// <summary>Files were released on the catcher.</summary>
    Dropped,

    /// <summary>Plith has stood down for the shelf rather than for a drop. The rectangle to grow
    /// out of travels in X/Y/W/H, the same way Show carries it.</summary>
    OpenShelf,

    /// <summary>One stack of the shelf. X is its index, Y is how many stacks there are in total,
    /// and the paths are its items, newest first. Sent once per stack so a stack boundary needs
    /// no separator inside a field.</summary>
    Items,

    /// <summary>The resolved theme, as seven values. See ShelfPaletteWire.</summary>
    Palette,

    /// <summary>Catcher to Plith: take these off the shelf.</summary>
    RemoveItems,

    /// <summary>Catcher to Plith: empty the shelf.</summary>
    ClearShelf,

    /// <summary>Catcher to Plith: put an empty stack at the front for the next drop.</summary>
    NewStack,

    /// <summary>Catcher to Plith: move these paths into the stack at index X. An index equal to
    /// the stack count appends a new one.</summary>
    Restack,

    /// <summary>Catcher to Plith: the shelf surface is gone, put the notch back.</summary>
    ShelfClosed,
}
```

In `TryDecode`, replace the verb parse:

```csharp
        // BOTH checks, and each one covers a hole the other leaves.
        //
        // TryParse accepts a NUMBER for any enum. "3" decodes to Dropped, which IsDefined would
        // happily confirm, so IsDefined alone lets a verb through that was reached by counting
        // rather than by name. And a name round trip alone is not enough either: for a value with
        // no name at all, such as 99, ToString falls back to printing the number, so "99" round
        // trips successfully. A pipe every process on this machine can write is precisely the
        // place a verb must not be reachable by counting, so the value must be one of ours AND
        // the sender must have written its name.
        if (!Enum.TryParse<DropVerb>(parts[0], out var verb)) return false;
        if (!Enum.IsDefined(verb)) return false;
        if (!string.Equals(verb.ToString(), parts[0], StringComparison.Ordinal)) return false;
```

- [x] **Step 4: Run the verb tests and watch them pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter DropChannelTests -v q -m:1`
Expected: PASS.

- [x] **Step 5: Write the failing palette test**

Create `tests/Plith.Tests/ShelfPaletteWireTests.cs`:

```csharp
using System.Windows.Media;
using Plith.Services.Shelf;

namespace Plith.Tests;

public sealed class ShelfPaletteWireTests
{
    private static ShelfPalette Sample() => new(
        Color.FromArgb(0xFF, 0x1A, 0x20, 0x28),
        Color.FromArgb(0xFF, 0x12, 0x16, 0x1C),
        Color.FromRgb(0xF2, 0xF5, 0xF8),
        Color.FromRgb(0x9A, 0xA6, 0xB2),
        Color.FromRgb(0x2A, 0x32, 0x3C),
        Color.FromRgb(0xA3, 0xE6, 0x35),
        IsDark: true);

    [Fact]
    public void ToPaths_ThenBack_RoundTripsEveryChannel()
    {
        var sent = Sample();

        Assert.True(ShelfPaletteWire.TryFromPaths(ShelfPaletteWire.ToPaths(sent), out var back));

        Assert.Equal(sent, back);
    }

    /// <summary>Seven values, and the order IS the contract. A short or long payload is a
    /// version mismatch between the two executables, and the catcher must fall back to its own
    /// colours rather than paint with whatever it managed to parse.</summary>
    [Theory]
    [InlineData(6)]
    [InlineData(8)]
    public void TryFromPaths_RefusesAPayloadOfTheWrongLength(int count)
    {
        var padded = Enumerable.Repeat("#FF000000", count).ToArray();

        Assert.False(ShelfPaletteWire.TryFromPaths(padded, out _));
    }

    [Fact]
    public void TryFromPaths_RefusesAValueThatIsNotAColour()
    {
        var paths = ShelfPaletteWire.ToPaths(Sample()).ToArray();
        paths[2] = "not a colour";

        Assert.False(ShelfPaletteWire.TryFromPaths(paths, out _));
    }
}
```

- [x] **Step 6: Run it and watch it fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter ShelfPaletteWireTests -v q -m:1`
Expected: FAIL, `ShelfPaletteWire` does not exist.

- [x] **Step 7: Write `ShelfPaletteWire`**

Create `src/Plith/Services/Shelf/ShelfPaletteWire.cs`:

```csharp
using System.Globalization;
using System.Windows.Media;

namespace Plith.Services.Shelf;

/// <param name="SurfaceStart">Top of the panel's gradient. Two colours rather than one because
/// OsdSurfaceBrush is a LinearGradientBrush: a single flat stand-in would be visibly not the
/// product's surface, which is the exact drift this type exists to prevent.</param>
/// <param name="IsDark">Carried rather than inferred from the ink. The catcher needs it for the
/// things a contrast ratio does not answer, such as which way a shadow falls.</param>
public readonly record struct ShelfPalette(
    Color SurfaceStart, Color SurfaceEnd, Color Ink, Color InkMuted, Color Track, Color Accent, bool IsDark);

/// <summary>
/// The theme, crossing a process boundary.
///
/// Plith derives its inks from the colour behind them through Services/ContrastInk.cs. A second
/// copy of that derivation in the catcher would drift, and the drift would show as the shelf not
/// looking like the product it belongs to. So the catcher is told the answer rather than given
/// the method.
///
/// It rides in the path list of a Palette message, which costs no new format: the list is
/// already escaped, already ordered, and already carried by every message.
/// </summary>
public static class ShelfPaletteWire
{
    /// <summary>Six colours and a flag. The ORDER is the contract; a named format would mean a
    /// parser on the far side and a second thing to keep in step.</summary>
    public const int FieldCount = 7;

    public static IReadOnlyList<string> ToPaths(ShelfPalette p) =>
    [
        Hex(p.SurfaceStart), Hex(p.SurfaceEnd), Hex(p.Ink), Hex(p.InkMuted), Hex(p.Track), Hex(p.Accent),
        p.IsDark ? "1" : "0",
    ];

    public static bool TryFromPaths(IReadOnlyList<string> paths, out ShelfPalette palette)
    {
        palette = default;
        if (paths.Count != FieldCount) return false;

        var colors = new Color[6];
        for (var i = 0; i < 6; i++)
        {
            if (!TryParseHex(paths[i], out colors[i])) return false;
        }

        if (paths[6] is not ("0" or "1")) return false;

        palette = new ShelfPalette(colors[0], colors[1], colors[2], colors[3], colors[4], colors[5],
                                   IsDark: paths[6] == "1");
        return true;
    }

    private static string Hex(Color c) =>
        string.Create(CultureInfo.InvariantCulture, $"#{c.A:X2}{c.R:X2}{c.G:X2}{c.B:X2}");

    /// <summary>
    /// Parsed by hand rather than through ColorConverter, because ColorConverter accepts named
    /// colours and malformed input by throwing, and this input arrives over a pipe any process
    /// can write. A parser that throws on hostile input is a parser that takes the catcher down.
    /// </summary>
    private static bool TryParseHex(string value, out Color color)
    {
        color = default;
        if (value.Length != 9 || value[0] != '#') return false;

        var channels = new byte[4];
        for (var i = 0; i < 4; i++)
        {
            if (!byte.TryParse(value.AsSpan(1 + i * 2, 2), NumberStyles.HexNumber,
                               CultureInfo.InvariantCulture, out channels[i]))
                return false;
        }

        color = Color.FromArgb(channels[0], channels[1], channels[2], channels[3]);
        return true;
    }
}
```

- [x] **Step 8: Link the new file into the catcher**

In `src/Plith.DropCatcher/Plith.DropCatcher.csproj`, beside the existing linked items:

```xml
    <!-- The theme crosses the wire, and both ends need the same seven fields in the same order.
         Linked for the same reason DropChannel is: two copies of a positional format drift, and
         the drift shows as the shelf painted in colours the product never uses. -->
    <Compile Include="..\Plith\Services\Shelf\ShelfPaletteWire.cs">
      <Link>Shared\ShelfPaletteWire.cs</Link>
    </Compile>
```

- [x] **Step 9: Run the palette tests and the build**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter ShelfPaletteWireTests -v q -m:1`
Expected: PASS.

Run: `dotnet build Plith.slnx -m:1`
Expected: build succeeds, including `Plith.DropCatcher` with the new linked file.

- [x] **Step 10: Commit**

```bash
git add src/Plith/Services/Shelf/DropChannel.cs src/Plith/Services/Shelf/ShelfPaletteWire.cs \
        src/Plith.DropCatcher/Plith.DropCatcher.csproj \
        tests/Plith.Tests/DropChannelTests.cs tests/Plith.Tests/ShelfPaletteWireTests.cs
git commit -m "feat(shelf): teach the wire about stacks, and stop it guessing verbs by number"
```

---

### Task 3: The surface, as something that can be photographed

The shelf's visuals go in a `UserControl`, not in the `Window`. That is the whole point of this task's shape: the render harness can instantiate a `UserControl` and photograph it, and it cannot do that with a layered `Window`. A surface nothing can look at is how this repo shipped a page that was unreadable in one theme and a row that overflowed on both axes.

**Files:**
- Create: `src/Plith.DropCatcher/Shelf/ShelfModel.cs`
- Create: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml`, `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`
- Create: `tests/Plith.Tests/ShelfModelTests.cs`
- Modify: `src/Plith.DropCatcher/Plith.DropCatcher.csproj` (link `ShelfModel.cs` into the test project's reachable set, see Step 1)
- Modify: `scripts/render-widgets.ps1`

**Interfaces:**
- Consumes: `ShelfPalette` from Task 2.
- Produces: `ShelfEntry(string Path, string Name, bool IsDirectory)`; `ShelfModel` with `IReadOnlyList<IReadOnlyList<ShelfEntry>> Stacks`, `void SetStack(int index, int total, IReadOnlyList<string> paths)`, `bool IsComplete`, `IReadOnlyCollection<string> Selection`, `void Select(string path, bool additive)`, `void ClearSelection()`, `IReadOnlyList<string> DragPaths(string pressedPath)`; `ShelfSurface` with `void Apply(ShelfPalette palette)`, `void Render(ShelfModel model)`, and the events `event Action<string, bool>? EntryPressed`, `event Action? ClearRequested`, `event Action? NewStackRequested`.

- [x] **Step 1: Make `ShelfModel` reachable from the test project**

`tests/Plith.Tests` references `Plith`, not `Plith.DropCatcher`, and it must not start referencing an executable that has to stay small. Add `ShelfModel.cs` to the test project as a linked compile item, in `tests/Plith.Tests/Plith.Tests.csproj`:

```xml
  <ItemGroup>
    <!-- The catcher's own model, linked rather than referenced. It holds no WPF types precisely
         so it can be tested here: the selection rule and the stack assembly are the two pieces
         of the surface that are logic rather than pixels, and they are the two that a headless
         suite can still reach. -->
    <Compile Include="..\..\src\Plith.DropCatcher\Shelf\ShelfModel.cs">
      <Link>Linked\ShelfModel.cs</Link>
    </Compile>
  </ItemGroup>
```

- [x] **Step 2: Write the failing model tests**

Create `tests/Plith.Tests/ShelfModelTests.cs`:

```csharp
using Plith.DropCatcher.Shelf;

namespace Plith.Tests;

public sealed class ShelfModelTests
{
    /// <summary>Stacks arrive one message at a time, so the model has to know when it has them
    /// all. Painting a half-delivered shelf would show a person a shelf that is missing rows.
    /// </summary>
    [Fact]
    public void SetStack_IsNotCompleteUntilEveryStackHasArrived()
    {
        var model = new ShelfModel();

        model.SetStack(0, 2, ["C:\\a.txt"]);
        Assert.False(model.IsComplete);

        model.SetStack(1, 2, ["C:\\b.txt"]);
        Assert.True(model.IsComplete);
        Assert.Equal(2, model.Stacks.Count);
    }

    [Fact]
    public void SetStack_WithZeroStacksIsAnEmptyShelfAndIsComplete()
    {
        var model = new ShelfModel();

        model.SetStack(0, 0, []);

        Assert.True(model.IsComplete);
        Assert.Empty(model.Stacks);
    }

    /// <summary>A second delivery replaces the first rather than adding to it. The shelf is
    /// re-sent whole on every change, so an appending model would double every row.</summary>
    [Fact]
    public void SetStack_StartingOverReplacesWhatWasThere()
    {
        var model = new ShelfModel();
        model.SetStack(0, 1, ["C:\\a.txt"]);

        model.SetStack(0, 1, ["C:\\b.txt"]);

        Assert.Equal("C:\\b.txt", Assert.Single(Assert.Single(model.Stacks)).Path);
    }

    [Fact]
    public void Select_WithoutAdditiveReplacesTheSelection()
    {
        var model = Loaded();

        model.Select("C:\\a.txt", additive: false);
        model.Select("C:\\b.txt", additive: false);

        Assert.Equal("C:\\b.txt", Assert.Single(model.Selection));
    }

    [Fact]
    public void Select_AdditiveTogglesAndAccumulates()
    {
        var model = Loaded();

        model.Select("C:\\a.txt", additive: true);
        model.Select("C:\\b.txt", additive: true);
        Assert.Equal(2, model.Selection.Count);

        model.Select("C:\\a.txt", additive: true);
        Assert.Equal("C:\\b.txt", Assert.Single(model.Selection));
    }

    /// <summary>
    /// The rule file managers use, and the reason it is here rather than in the window: pressing
    /// a tile inside the selection drags the whole selection, and pressing one outside it drags
    /// that tile alone. Getting this backwards means a person drags four files when they meant
    /// one, which with Copy semantics is recoverable but with any other would not be.
    /// </summary>
    [Fact]
    public void DragPaths_PressInsideTheSelectionCarriesAllOfIt()
    {
        var model = Loaded();
        model.Select("C:\\a.txt", additive: true);
        model.Select("C:\\b.txt", additive: true);

        var paths = model.DragPaths("C:\\a.txt");

        Assert.Equal(2, paths.Count);
    }

    [Fact]
    public void DragPaths_PressOutsideTheSelectionCarriesOnlyThatOne()
    {
        var model = Loaded();
        model.Select("C:\\a.txt", additive: false);

        var paths = model.DragPaths("C:\\b.txt");

        Assert.Equal("C:\\b.txt", Assert.Single(paths));
        Assert.Equal("C:\\b.txt", Assert.Single(model.Selection));
    }

    private static ShelfModel Loaded()
    {
        var model = new ShelfModel();
        model.SetStack(0, 1, ["C:\\a.txt", "C:\\b.txt"]);
        return model;
    }
}
```

- [x] **Step 3: Run them and watch them fail**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter ShelfModelTests -v q -m:1`
Expected: FAIL, `ShelfModel` does not exist.

- [x] **Step 4: Write `ShelfModel`**

Create `src/Plith.DropCatcher/Shelf/ShelfModel.cs`:

```csharp
using System.IO;

namespace Plith.DropCatcher.Shelf;

/// <param name="Path">As Plith sent it. Plith has already stat'd it; the catcher displays it.
/// </param>
public readonly record struct ShelfEntry(string Path, string Name, bool IsDirectory);

/// <summary>
/// What the catcher believes is on the shelf, and which of it is selected.
///
/// Deliberately free of every WPF type, for the same reason NotchGeometry is: it makes the two
/// parts of this surface that are logic rather than pixels reachable by a headless test suite.
/// Everything else about the shelf can only be judged by looking at it.
///
/// This is a VIEW of Plith's shelf, never the authority on it. Every change is a request sent
/// back over the wire, and the answer arrives as a fresh set of Items messages.
/// </summary>
public sealed class ShelfModel
{
    private readonly List<List<ShelfEntry>> _stacks = [];
    private readonly HashSet<string> _selection = new(StringComparer.OrdinalIgnoreCase);
    private int _expected = -1;

    public IReadOnlyList<IReadOnlyList<ShelfEntry>> Stacks => _stacks;

    /// <summary>True once every stack in the set has arrived. Stacks come one message each, and
    /// a surface painted halfway through the set would show a shelf missing rows.</summary>
    public bool IsComplete => _expected >= 0 && _stacks.Count >= _expected;

    public IReadOnlyCollection<string> Selection => _selection;

    public void SetStack(int index, int total, IReadOnlyList<string> paths)
    {
        // Index 0 starts the set over. The shelf is re-sent whole on every change, so anything
        // else would accumulate the old contents under the new ones.
        if (index <= 0)
        {
            _stacks.Clear();
            _expected = total;
        }

        if (total > 0) _stacks.Add([.. paths.Select(Describe)]);

        // A path that has gone away since Plith sent it stays selected otherwise, and a drag
        // would then carry a file that is not on the shelf any more.
        _selection.RemoveWhere(p => !_stacks.Any(s => s.Any(e => PathEquals(e.Path, p))));
    }

    public void Select(string path, bool additive)
    {
        if (!additive)
        {
            _selection.Clear();
            _selection.Add(path);
            return;
        }

        if (!_selection.Remove(path)) _selection.Add(path);
    }

    public void ClearSelection() => _selection.Clear();

    /// <summary>
    /// What a press on <paramref name="pressedPath"/> drags, and it has a side effect on purpose:
    /// a press outside the selection RESETS the selection to that one tile, which is what every
    /// file manager does and what a person pressing an unselected tile means.
    /// </summary>
    public IReadOnlyList<string> DragPaths(string pressedPath)
    {
        if (_selection.Contains(pressedPath)) return [.. _selection];

        Select(pressedPath, additive: false);
        return [pressedPath];
    }

    private static bool PathEquals(string a, string b) => string.Equals(a, b, StringComparison.OrdinalIgnoreCase);

    private static ShelfEntry Describe(string path)
    {
        var isDirectory = Directory.Exists(path);
        var name = isDirectory ? new DirectoryInfo(path).Name : Path.GetFileName(path);
        if (string.IsNullOrEmpty(name)) name = path;
        return new ShelfEntry(path, name, isDirectory);
    }
}
```

- [x] **Step 5: Run the model tests and watch them pass**

Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj --filter ShelfModelTests -v q -m:1`
Expected: PASS.

- [x] **Step 6: Build `ShelfSurface` against the render harness**

Create `ShelfSurface.xaml` and its code-behind. Constraints that are not negotiable while building it:

- Every brush comes from the `ShelfPalette` passed to `Apply`, resolved onto the control's own `Resources` under the same keys Plith uses: `OsdSurfaceBrush`, `NotchInk`, `NotchInkMuted`, `NotchTrack`, `AccentBrush`. Same keys, so `check-contrast.ps1` can name the same pairs in Task 9 and the harness can feed it Plith's real palette.
- Nothing resolves through `Application.Resources`. The harness puts the palette on the host element, and anything reaching through `Application` renders in colours the product never shows. This caught `ShelfWidget` once already.
- Drawn geometry for the chrome (the clear control, the new-stack control, the stack separators). Shell thumbnails arrive in Task 5 and are not an icon font.
- Provisional geometry, and it must be marked as provisional in the code that carries it: 384 x 264 DIP surface, five columns, 64 DIP tiles, 8 DIP gaps, two rows visible. These came from arithmetic, and arithmetic is what the first `ShelfWidget` was caught by.

Render it before calling it done:

Run: `pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark -Accent '#A3E635'`
Run: `pwsh -STA -File scripts/render-widgets.ps1 -Theme Light -Accent '#A3E635'`
Run: `pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark -Accent '#FFFFFF'`

Extend `scripts/render-widgets.ps1` to load `Plith.DropCatcher.dll` alongside `Plith.dll` and render `ShelfSurface` with a model of seven entries across two stacks, at the same three call sites the widgets use. The white accent is not optional: it is the accent that made `NotchInk` on a track tile measure 4.0:1, and it is the one that finds this class of defect.

Expected: the surface fits its own frame on both axes in all three renders, every label is readable, and no row is clipped.

- [x] **Step 7: Correct the provisional numbers from what the render shows**

Change the four constants to what the render needs, and replace the "provisional" comment with what was measured. If they were right, say that instead. A constant whose comment still says "provisional" after a render has been looked at is a lie that the next reader will act on.

- [x] **Step 8: Commit**

```bash
git add src/Plith.DropCatcher/Shelf/ShelfModel.cs src/Plith.DropCatcher/Shelf/ShelfSurface.xaml \
        src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs tests/Plith.Tests/ShelfModelTests.cs \
        tests/Plith.Tests/Plith.Tests.csproj scripts/render-widgets.ps1
git commit -m "feat(shelf): draw the shelf where a render harness can see it"
```

---

### Task 4: The window the person touches

**Files:**
- Create: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml`, `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs`
- Modify: `src/Plith.DropCatcher/App.xaml.cs`

**Interfaces:**
- Consumes: `ShelfSurface`, `ShelfModel`, `ShelfPalette`, `NotchGeometry`, `CatcherLog`.
- Produces: `ShelfWindow` with `void OpenAt(int x, int y, int width, int height)`, `void Apply(ShelfPalette palette)`, `void SetStack(int index, int total, IReadOnlyList<string> paths)`, `void CloseNow()`, and `event Action? Closed`.

- [x] **Step 1: Write the window**

It is a sibling of `CatcherWindow`, not an extension of it. Copy the parts that are the same and say why in the file, because two windows in one process with different activation rules is the kind of thing that gets "simplified" into one by a later reader.

What it shares with `CatcherWindow`:
- `WindowStyle="None"`, `AllowsTransparency="True"`, `Background="#01000000"`, `Topmost="True"`, `ShowInTaskbar="False"`, `ResizeMode="NoResize"`, `WS_EX_TOOLWINDOW`.
- Physical pixels applied with `SetWindowPos`, never through WPF's `Left`/`Top`. The wire carries physical pixels precisely so neither process repeats the other's DPI arithmetic.
- `Show()` before `SetWindowPos`. `EnsureHandle` plus `SWP_SHOWWINDOW` makes a window WPF does not consider shown: no visual tree, no drop target, nothing on screen. This cost a live run once already.
- The single-expansion-value growth, driven by `NotchGeometry.SurfaceSize` / `SurfaceRadius` / `ContentOpacity`, so the shelf grows the curve the notch grows.

What differs, each with its reason in the code:
- **No `WS_EX_NOACTIVATE`, and `ShowActivated` is true.** `Esc`, arrow keys and a visible selection have nothing to arrive at in a window that never takes focus.
- **No withdraw timer.** The drop stand-in withdraws because nothing distinguishes a carried file from a window being dragged to the top. The shelf was asked for.
- **The growth runs from the notch's open frame to the shelf size**, not from the resting strip: `NotchGeometry.SurfaceSize(openFrame.Width, openFrame.Height, shelfSize, t)`.

Dismissal, all of it in one place:

```csharp
    /// <summary>
    /// Every way the shelf goes away, and the two states that suspend all of them.
    ///
    /// A drag in flight must not dismiss the surface: the person is holding a file over another
    /// application, the shelf has lost activation by definition, and closing under them would
    /// cancel the gesture they are in the middle of. A context menu takes activation too, for
    /// the same reason and with the same answer.
    /// </summary>
    private void Dismiss(string why)
    {
        if (_dragInFlight || _menuOpen) return;

        _log.Info($"Shelf closing: {why}.");
        CloseNow();
    }
```

Declare both suspending fields here, defaulting false, even though nothing sets them yet:

```csharp
    /// <summary>Set by the drag out in Task 8. Declared here because Dismiss reads it, and a
    /// window that can only be dismissed correctly after a later task is a window that is wrong
    /// in between.</summary>
    private bool _dragInFlight;

    /// <summary>Set by the context menu in Task 7, same reason.</summary>
    private bool _menuOpen;
```

Wire `PreviewKeyDown` for `Key.Escape`, `Deactivated`, and a `MouseLeave` that starts a short `DispatcherTimer` rather than closing immediately, cancelled by `MouseEnter`. The grace period exists because the pointer crosses outside the surface on the way to a tile at its edge.

- [x] **Step 2: Route the new verbs in the catcher's `App`**

In `src/Plith.DropCatcher/App.xaml.cs`, extend `OnReceived`:

```csharp
            case DropVerb.OpenShelf:
                _shelf.OpenAt((int)message.X, (int)message.Y, (int)message.W, (int)message.H);
                break;
            case DropVerb.Items:
                _shelf.SetStack((int)message.X, (int)message.Y, message.Paths);
                break;
            case DropVerb.Palette:
                if (ShelfPaletteWire.TryFromPaths(message.Paths, out var palette)) _shelf.Apply(palette);
                else _log.Info("Palette payload did not decode; keeping the built-in colours.");
                break;
```

Construct `_shelf` beside `_window`, and send `ShelfClosed` from its `Closed` event:

```csharp
        _shelf = new ShelfWindow(_log);
        _shelf.Closed += () => _ = _client?.SendAsync(new DropMessage(DropVerb.ShelfClosed, 0, 0, 0, 0, []));
```

- [x] **Step 3: Build**

Run: `dotnet build Plith.slnx -m:1`
Expected: succeeds.

- [ ] **Step 4: Drive it by hand, before anything depends on it** PARTLY RUN. The probe was driven and its log read, including three runs with a real cursor, but nobody has LOOKED at the window: this session is over Remote Desktop and a layered window cannot be captured there. The visual half is docs/SHELF-VERIFICATION.md section 1.

The catcher already has hand-run probe modes ahead of the single-instance guard, and this is the same kind of thing. Add `--shelfprobe x y w h` beside them: it opens `ShelfWindow` at that rectangle with three invented entries in two stacks and a built-in palette, and logs what it did.

Run from a console session (not over Remote Desktop, where a layered window cannot be captured):

```
.\Plith.DropCatcher.exe --shelfprobe 708 0 384 264
```

Expected, and every one of these is a thing to look at rather than to infer from a log: the surface grows rather than appearing; it is readable; `Esc` closes it; clicking another application closes it; the pointer leaving and coming back does not.

- [x] **Step 5: Write down what the probe showed**

In `docs/SHELF-VERIFICATION.md` (created here, extended by later tasks), with the date and the session type from `qwinsta` rather than from `$env:SESSIONNAME`, which is stamped at process start and was wrong on this machine once already.

- [x] **Step 6: Commit**

```bash
git add src/Plith.DropCatcher/Shelf/ShelfWindow.xaml src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs \
        src/Plith.DropCatcher/App.xaml.cs docs/SHELF-VERIFICATION.md
git commit -m "feat(shelf): a surface that takes focus, and closes only when it should"
```

---

### Task 5: Real shell icons

This belongs at Medium integrity, and the reason is worth stating where it is done: icon extraction loads third-party shell handlers into the process doing it, and the process that must not be doing that is the one with UIAccess.

**Files:**
- Create: `src/Plith.DropCatcher/Shelf/ShellIcons.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ShellIcons.TryGet(string path, out ImageSource icon) -> bool`.

- [x] **Step 1: Write `ShellIcons`**

`SHGetFileInfo` with `SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES` when the path is gone, and without it when the file is there. Convert with `Imaging.CreateBitmapSourceFromHIcon`, then `DestroyIcon`, then `Freeze()`.

Three things that are defects if they are missed, each of which needs a comment saying so:

```csharp
    // DestroyIcon in a finally, always. SHGetFileInfo hands over an HICON that belongs to the
    // caller, and a shelf that redraws on every change would leak one per tile per repaint.

    // Freeze, so the ImageSource can be built off the UI thread and handed to it. Without this
    // the extraction has to happen on the dispatcher, and a slow network path stalls the surface
    // for as long as the shell takes to answer.

    // Cached by path, because a repaint costs an extraction otherwise and the shelf repaints on
    // every selection change. The cache is per-run and unbounded by design: it is bounded in
    // practice by the shelf's own cap of twenty.
```

Fall back to the drawn document and folder geometry already in the product when `TryGet` returns false, rather than showing a blank. A network path that does not answer is the case that makes this matter.

- [x] **Step 2: Keep the accessibility lint honest**

`scripts/check-a11y.ps1` fails the build on Segoe MDL2 glyphs, in code-behind as well as XAML. A shell thumbnail is not a glyph, but the lint should not have to be argued with either. Run it and see what it says:

Run: `pwsh -File scripts/check-a11y.ps1`
Expected: PASS. If it flags the new code, the fix is in the lint's pattern, and the change must be narrow enough that a real Segoe MDL2 glyph in the same file would still fail. Widening the rule to accommodate this one file would remove the check that caught the product's last four glyph regressions.

- [x] **Step 3: Render, in both themes**

Run: `pwsh -STA -File scripts/render-widgets.ps1 -Theme Dark -Accent '#A3E635'`
Run: `pwsh -STA -File scripts/render-widgets.ps1 -Theme Light -Accent '#A3E635'`

The harness renders with invented paths, so it exercises the fallback rather than the shell. Point at least one entry in the harness at a file that really exists (`$PSHOME\powershell.exe` or the built `Plith.exe`) so a real icon is in the picture too. A render that only ever shows the fallback proves nothing about the thing this task adds.

- [x] **Step 4: Commit**

```bash
git add src/Plith.DropCatcher/Shelf/ShellIcons.cs src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs \
        scripts/render-widgets.ps1
git commit -m "feat(shelf): show the shell's own icons, extracted where they belong"
```

---

### Task 6: The hand-over

**Files:**
- Create: `src/Plith/Services/Shelf/ShelfSession.cs`
- Modify: `src/Plith/Services/Shelf/DropCatcherLauncher.cs`
- Modify: `src/Plith/Views/OsdHost.cs`
- Modify: `src/Plith/Views/Widgets/ShelfWidget.cs`
- Modify: `src/Plith/App.xaml.cs`

**Interfaces:**
- Consumes: `DropChannelServer`, `ShelfStore`, `ShelfPaletteWire`, `DropCatcherLauncher`.
- Produces: `ShelfSession` with `void Open(Rect notchRectDip, double dpiScale)`, `event Action? Opened`, `event Action? Closed`, `event Action<string>? Unavailable`, and `void HandleMessage(DropMessage)`; `DropCatcherLauncher.EnsureRunning -> CatcherStart` where `enum CatcherStart { Started, AlreadyRunning, NotFound, Failed }`; `OsdHost.OpenShelf()`; `ShelfWidget.OpenRequested` event.

- [x] **Step 1: Split the launcher's result**

`EnsureRunning` returns `false` today both for "one is already running" and for "it could not be started". Nothing could distinguish them, and Task 6 has to tell a person which happened.

```csharp
/// <summary>Why a start attempt ended the way it did. A bool could not tell "one is already
/// running" from "it is not installed", and the shelf has to say something different for each.
/// </summary>
public enum CatcherStart { Started, AlreadyRunning, NotFound, Failed }
```

Change the signature to `public static CatcherStart EnsureRunning(DiagnosticLog? log = null)` and return the matching value at each existing exit. Update the two call sites the compiler finds.

- [x] **Step 2: Write `ShelfSession`**

It owns the whole conversation, so `OsdHost` gains a call rather than a protocol. `OsdHost` is already long, and threading another eight-verb exchange through it is how a file stops being readable.

```csharp
/// <summary>
/// The open-and-close conversation with the drop catcher.
///
/// Separate from OsdHost because OsdHost is a window and this is a protocol. Everything here is
/// a request to another process and an answer that may never come, and mixing that with a
/// window's layout is how both become hard to follow.
///
/// Every message arriving here came from a process at a lower integrity level over a pipe any
/// process on this machine can write. ShelfStore is the only thing that decides what is true;
/// this class routes, and routes nothing that ShelfStore would not check.
/// </summary>
public sealed class ShelfSession
```

`Open` does, in this order: `EnsureRunning`; if the channel is not connected, raise `Unavailable` with a sentence naming which `CatcherStart` happened and stop; otherwise send `Palette`, then one `Items` per stack, then `OpenShelf` with the physical rectangle, then raise `Opened`.

Palette before items, and items before `OpenShelf`, so the surface has everything it needs before it is told to appear. A surface that grows and then repaints is a surface the person watches assemble itself.

`HandleMessage` routes `RemoveItems` to `ShelfStore.RemoveMany`, `ClearShelf` to `Clear`, `NewStack` to `NewStack`, `Restack` to `Restack`, and `ShelfClosed` to `PruneEmptyStacks` followed by raising `Closed`. On any of the four that change the shelf, re-send the `Items` set, because the catcher's model is a view and this is what refreshes it.

- [x] **Step 3: Give standing aside a reason**

In `OsdHost`, `_standingAside` becomes `private StandAsideReason _standAside;` with `enum StandAsideReason { None, Drag, Shelf }`. `EndStandAside` already exists and is called from two places; it must not pull the notch back while the reason is `Shelf`, because the shelf is not a 450 ms stand-in and the drag detector will happily raise a transition under it.

Add:

```csharp
    /// <summary>
    /// Stand aside for the shelf rather than for a drop.
    ///
    /// The same mechanism as BeginStandAside and deliberately not the same method: that one
    /// arms a withdrawal after 450 ms, because nothing distinguishes a file being carried to the
    /// top of the screen from a window being dragged there. The shelf was asked for, so it
    /// stays until it is dismissed.
    /// </summary>
    public void OpenShelf()
```

- [x] **Step 4: Make the glance page a door**

In `ShelfWidget`, add `public event Action? OpenRequested;` and raise it from `MouseLeftButtonUp` on the root.

Button **up**, not down, and it matters: the hand-over is what Task 8 makes dangerous, and on release there is no press in flight to be split across two processes.

`OsdHost.OnNotchClicked` is wired at `PreviewMouseLeftButtonDown` but returns early once the frame is open (`src/Plith/Views/OsdHost.cs:877`), and the rail marks its own clicks handled (`src/Plith/Views/Widgets/WidgetFrame.cs:218`), so the page gets this event without any change to either. Verified by reading both paths while planning; verify again by clicking, because reading is not running.

Add the hint the empty state already has, for the case where the shelf is not empty: a line under the row saying the shelf opens on a click. This is the only place the product will ever say so.

- [x] **Step 5: Build and run the suite**

Run: `dotnet build Plith.slnx -m:1`
Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`
Expected: both succeed.

- [ ] **Step 6: Drive the hand-over on a console session**. NOT RUN. This session is `rdp-tcp#0` (`qwinsta`), where the shelf's layered window cannot be captured. Written up as numbered items in `docs/SHELF-VERIFICATION.md` §2 instead, phrased for someone who was not here.

Start Plith and let the catcher start through Explorer. Confirm `dropcatcher.log` says `MEDIUM`; if it says `HIGH` the launch route is wrong and nothing after this will work, which is the failure mode that looks exactly like success.

Then: open the notch, page to the shelf, click it. Expected: the notch goes down, the shelf grows from the same rectangle, and `Esc` brings the notch back. Then kill the catcher and click the shelf page: expected, a sentence rather than a dead click.

Write both into `docs/SHELF-VERIFICATION.md`.

- [x] **Step 7: Commit**

```bash
git add src/Plith/Services/Shelf/ShelfSession.cs src/Plith/Services/Shelf/DropCatcherLauncher.cs \
        src/Plith/Views/OsdHost.cs src/Plith/Views/Widgets/ShelfWidget.cs src/Plith/App.xaml.cs \
        docs/SHELF-VERIFICATION.md
git commit -m "feat(shelf): open the shelf from the notch, and say so when it cannot"
```

---

### Task 7: The actions

**Files:**
- Create: `src/Plith.DropCatcher/Shelf/ShelfActions.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml`, `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs`

**Interfaces:**
- Consumes: `ShelfModel`, `ShelfSurface` events from Task 3, `DropVerb` from Task 2.
- Produces: `ShelfActions.Open(string path)`, `ShelfActions.ShowInFileManager(string path)`; `ShelfSurface` gains `event Action<IReadOnlyList<string>>? RemoveRequested`, `event Action<int, IReadOnlyList<string>>? RestackRequested`, `event Action<string>? OpenRequested`, `event Action<string>? RevealRequested`.

- [x] **Step 1: Write `ShelfActions`**

```csharp
/// <summary>
/// Opening a file, from the right process.
///
/// This runs at Medium integrity, which is the point rather than an accident. The same call from
/// Plith would start the person's document at High, because a child inherits its parent's token
/// and Plith has UIAccess. A text file opened at High is a text file whose editor cannot be
/// dragged onto by anything, and the person has no way of knowing why.
/// </summary>
public static class ShelfActions
{
    public static void Open(string path)
    {
        try
        {
            using var _ = Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
        }
        catch (Win32Exception) { /* no handler for this type; nothing useful to say */ }
        catch (InvalidOperationException) { }
    }

    /// <summary>
    /// The file manager, whichever one this is. Explorer's /select verb is honoured by the
    /// default handler for a folder, and on this machine that is Files rather than Explorer:
    /// `explorer.exe &lt;path&gt;` opens a WinUIDesktopWin32WindowClass window here, not a
    /// CabinetWClass one. That is measured, and it is why nothing downstream may look for
    /// Explorer's window class.
    /// </summary>
    public static void ShowInFileManager(string path)
    {
        try
        {
            using var _ = Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{path}\"")
            {
                UseShellExecute = true,
            });
        }
        catch (Win32Exception) { }
    }
}
```

- [x] **Step 2: Wire the surface's controls**

- A clear control in the header, which raises `ClearRequested`. It asks nothing first: the shelf holds references, clearing deletes no file, and a confirmation for a reversible action on a surface this small is friction rather than safety. Say that in a comment, because the next reader will want to add a dialog.
- A new-stack control, which raises `NewStackRequested`.
- A per-tile remove affordance, shown on hover and always reachable from the context menu, raising `RemoveRequested` with the selection when the tile is in it and with the one path when it is not. That is `ShelfModel.DragPaths`, which already has exactly this rule and a test for it; use it rather than writing the rule twice.
- A context menu per tile: open, show in file manager, remove. Set `_menuOpen` on the window while it is up, or the menu taking activation dismisses the surface underneath it.
- Dragging a tile onto another stack raises `RestackRequested` with that stack's index; onto the empty area past the last stack, with `Stacks.Count`.

- [x] **Step 3: Send them**

In `ShelfWindow`, forward each event to the client as `RemoveItems`, `ClearShelf`, `NewStack` or `Restack`. Nothing is applied locally: the model is a view, Plith answers with a fresh `Items` set, and a surface that also updated itself would show a shelf that disagreed with the file the moment anything failed.

- [ ] **Step 4: Run it** NOT RUN. Needs a physical console session; see docs/SHELF-VERIFICATION.md section 3.

Console session. Drop three files on the notch, open the shelf, and: remove one, clear all, make a new stack and drop into it, move a tile between stacks, open a file, show one in the file manager.

Expected: every one of them is reflected in `shelf.txt` as well as on screen. Open the file in Notepad and look, because that file being readable is a feature this slice deliberately preserved and this is the one moment it gets checked.

- [x] **Step 5: Write down what happened, including anything that did not work**

`docs/SHELF-VERIFICATION.md`.

- [x] **Step 6: Commit**

```bash
git add src/Plith.DropCatcher/Shelf/ShelfActions.cs src/Plith.DropCatcher/Shelf/ShelfSurface.xaml \
        src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs \
        docs/SHELF-VERIFICATION.md
git commit -m "feat(shelf): remove, clear, stack and open, from the process that may"
```

---

### Task 8: Dragging out

Last, because it is the only step whose failure mode is a hung UI thread, and it should meet a surface that already works.

**Files:**
- Modify: `src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs`
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`

**Interfaces:**
- Consumes: `ShelfModel.DragPaths`, `ShelfSurface.EntryPressed`.
- Produces: nothing new. The drag is started inside the window.

- [x] **Step 1: Start the drag, with the guard**

```csharp
    /// <summary>
    /// Start a drag for a press that landed on THIS window, and only ever for one.
    ///
    /// Measured on 18.09.2026, three runs at Medium integrity with the press verified by
    /// WindowFromPoint to belong to another process: DoDragDrop never delivers a drag for a press
    /// it did not receive. No drop target saw a DragEnter, and the call returned None. The
    /// control is the run before it: same binary, same integrity, same call, press on its own
    /// window, Copy, Move, and the file landed.
    ///
    /// The second finding is why this is a guard rather than a comment. One of those three runs
    /// did not return AT ALL, and was still blocked seventeen seconds after the release. The
    /// thread that would block here is the catcher's UI thread, which is the thread the whole
    /// shelf depends on. So: only from a mouse event on our own element tree, never from a timer,
    /// never from a pipe message, never from anywhere the press origin is not known.
    ///
    /// Full ledger: docs/superpowers/plans/2026-09-17-shelf-drop-catcher.md, Task 8.
    /// </summary>
    private void StartDrag(DependencyObject source, IReadOnlyList<string> paths)
    {
        if (paths.Count == 0) return;

        // The origin check, not a formality. If this element is not ours, the press was not ours.
        if (!ReferenceEquals(PresentationSource.FromDependencyObject(source), PresentationSource.FromVisual(this)))
        {
            _log.Info("Refused a drag whose press did not land on this window.");
            return;
        }

        var data = new DataObject(DataFormats.FileDrop, paths.ToArray());

        _dragInFlight = true;
        try
        {
            // Copy and Link, never Move. The shelf stages references; Move invites the
            // destination to delete the original, and a shelf that loses someone's work the
            // first time they mistake it for a pocket is worse than no shelf.
            var effect = DragDrop.DoDragDrop(this, data, DragDropEffects.Copy | DragDropEffects.Link);
            _log.Info($"Drag out returned {effect} for {paths.Count} path(s).");
        }
        finally
        {
            _dragInFlight = false;
        }
    }
```

- [x] **Step 2: Only past the system threshold**

Track the press point on `PreviewMouseLeftButtonDown` over a tile, and call `StartDrag` from `MouseMove` once the pointer has moved further than `SystemParameters.MinimumHorizontalDragDistance` or `MinimumVerticalDragDistance`. Below the threshold it is a click, and a click selects.

The system values rather than a number of our own: a person who has tuned their threshold has done so for every application, and this one has no reason to be the exception.

- [ ] **Step 3: Drive it, on a console session** NOT RUN. This is the step that would find a hang. See docs/SHELF-VERIFICATION.md section 4, and 4.4 in particular.

Not over Remote Desktop, where this is meaningless.

1. Open the shelf with three items in one stack.
2. Press one tile and drag it into an Explorer or Files window. Expected: the file lands, `dropcatcher.log` says `Copy` or `Link`, and the row is still on the shelf.
3. Select two tiles with `Ctrl` and drag from one of them. Expected: both land.
4. Press a tile, move two pixels, release. Expected: it selects and no drag starts.
5. Drag out and release over empty desktop. Expected: whatever the shell does, and the catcher is still alive and answering afterwards. This is the step that would find a hang.

- [x] **Step 4: Write the result into `docs/SHELF-VERIFICATION.md`**

Including the `DoDragDrop` return value for each run, and the integrity level from the log. Both go in the document, because this is the measurement the whole slice was built on top of and the next person should not have to take it on trust.

- [x] **Step 5: Commit**

```bash
git add src/Plith.DropCatcher/Shelf/ShelfWindow.xaml.cs src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs \
        docs/SHELF-VERIFICATION.md
git commit -m "feat(shelf): drag items out, from the only window that can"
```

---

### Task 9: Accessibility, the lints, and the record

**Files:**
- Modify: `src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs`
- Modify: `scripts/check-contrast.ps1`
- Modify: `docs/SHELF-VERIFICATION.md`
- Modify: `docs/ROADMAP.md`, `CLAUDE.md`, `docs/superpowers/plans/2026-09-18-shelf-slice-2.md`

- [x] **Step 1: Names on everything a screen reader reaches**

`AutomationProperties.SetName` on every tile, on each stack, and on the clear and new-stack controls. Announce the count before the names, as `ShelfWidget` already does: a screen reader user needs to know how much is there before hearing a list of file names.

A `StackPanel` carries no automation peer of its own, so a name set on one reaches nothing. Set them on the elements that have peers. This is the defect class that shipped green through Phase 5's accessibility pass four times.

Keyboard: arrow keys move the selection, `Space` toggles it, `Enter` opens, `Delete` removes. A surface that takes focus and then answers no key is worse than one that never took focus.

- [x] **Step 2: Extend `check-contrast.ps1` to the catcher**

Two changes:

```powershell
# The catcher paints the shelf, in the same palette keys, and until this slice nothing scanned
# it. Its window carried hard-coded #F2141414 and #FFFFFFFF that no check ever measured.
$xamlFiles = @(
    Get-ChildItem -Path (Join-Path $root 'src\Plith') -Filter '*.xaml' -Recurse
    Get-ChildItem -Path (Join-Path $root 'src\Plith.DropCatcher') -Filter '*.xaml' -Recurse
) | Where-Object { $_.FullName -notmatch '[\\/](obj|bin)[\\/]' }
```

and, in `$codeBehindPairs`:

```powershell
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchInk';      Where = 'ShelfSurface.xaml.cs:tile label and count' }
    @{ Bg = 'OsdSurfaceBrush'; Fg = 'NotchInkMuted'; Where = 'ShelfSurface.xaml.cs:stack caption' }
```

The pairs work only because Task 3 resolved the catcher's brushes under Plith's own key names. If a colour in the catcher is not reachable by one of those keys, it is not measured, and the honest fix is to make it reachable rather than to add an exception.

- [x] **Step 3: Run all three lints and the suite**

Run: `pwsh -File scripts/check-a11y.ps1`
Run: `pwsh -File scripts/check-shared-xaml.ps1`
Run: `pwsh -File scripts/check-contrast.ps1`
Run: `dotnet test tests/Plith.Tests/Plith.Tests.csproj -v q -m:1`
Expected: all pass. Fix what they find in the code, not in the scripts.

- [x] **Step 4: Finish `docs/SHELF-VERIFICATION.md`**

It must say, in its own words rather than by implication: what was driven on hardware and on what date; what was not; and that a green build, green tests and a green lint prove nothing about this surface, because the suite is not STA and a layered window cannot be captured over Remote Desktop.

Carry the harness lessons from the slice 1 plan's Task 8 into it, so they are somewhere a person will open: ask `qwinsta` and never `$env:SESSIONNAME`; stage and aim in one process and re-verify the aim in the same breath as the press; use windows the harness created; minimise the competing maximised window and restore it in a `finally`; `WindowFromPoint` returns the child, so compare owning process ids.

- [x] **Step 5: Tick this plan's boxes honestly, and band it**

Put a band at the top of this file saying what a tick means:

> **A tick means the step was taken, NOT that nothing was deviated from.** Deviations are in `docs/SHELF-VERIFICATION.md` and in the code that carries them.

Update `docs/ROADMAP.md` Phase 7 and `CLAUDE.md`'s shelf paragraph with what this slice actually did, including anything it could not do.

`CLAUDE.md`'s Status section is split across this branch and `feature/brightness`, and this branch's copy still describes Phases 5 and 6 as unmerged, which is wrong. Do not fix that here. The existing note says whichever branch merges second merges the section by hand, and quietly half-fixing it on one side is what makes a hand merge impossible to do correctly.

- [x] **Step 6: Commit**

```bash
git add src/Plith.DropCatcher/Shelf/ShelfSurface.xaml.cs scripts/check-contrast.ps1 \
        docs/SHELF-VERIFICATION.md docs/ROADMAP.md CLAUDE.md \
        docs/superpowers/plans/2026-09-18-shelf-slice-2.md
git commit -m "docs(shelf): record what slice 2 does, and what nothing verified"
```

---

## Self-Review

**Spec coverage.** Section 1 (two windows) is Tasks 3 and 4. Section 2 (ownership) is Tasks 1 and 6. Section 3 (wire) is Task 2. Section 4 (stacks) is Task 1, with the surface half in Task 7. Section 5 (dragging out) is Task 8. Section 6 (palette) is Tasks 2, 3 and 9. Section 7 (entry, exit, no catcher) is Tasks 4 and 6. Section 8 (testing) is spread across every task's last steps and gathered in Task 9. Section 9 (non-goals) adds no task, correctly. Section 10's risks map to: risk 1 to Task 9 Step 4, risk 2 to Task 6 Step 2's `Unavailable`, risk 3 resolved while planning and recorded above, risk 4 to Task 4 Step 1, risk 5 to Task 3 Step 7.

**Gaps found and closed.** The spec's "open and show in the file manager" were listed in its build order but had no owner in the first draft of this plan; they are Task 7. The spec's `NewStack` gap and the palette's seventh field are recorded at the top rather than left as silent divergence.

**Type consistency.** `ShelfStore.RemoveMany` is used under that name in Task 6; `ShelfModel.DragPaths` is used under that name in Tasks 7 and 8; `ShelfPaletteWire.TryFromPaths` is used under that name in Task 4; `CatcherStart` is produced in Task 6 Step 1 and consumed in Step 2 of the same task. `ShelfSurface.Apply`, `.Render` and `ShelfWindow.SetStack` are declared in Task 3's Interfaces block and called from Task 4.

**Known weakness of this plan, stated rather than hidden.** Tasks 3, 4, 5 and 7 carry constraints and acceptance criteria rather than complete code, because they are windows and their real content is what a render and a run show. That is the same limit the slice 1 plan had on its Tasks 5 and 6, and it is honest about the same thing: nobody can write a surface's final geometry into a plan without having looked at it. Every one of those tasks names the command that produces the picture and what to look for in it.
