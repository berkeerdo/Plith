using System.IO;
using Plith.Services.Shelf;

namespace Plith.Tests;

public sealed class ShelfStoreTests : IDisposable
{
    private readonly string _directory;
    private readonly string _storePath;

    public ShelfStoreTests()
    {
        _directory = Path.Combine(Path.GetTempPath(), "plith-shelf-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_directory);
        _storePath = Path.Combine(_directory, "shelf.txt");
    }

    public void Dispose()
    {
        try { Directory.Delete(_directory, recursive: true); }
        catch (IOException) { }
    }

    private string MakeFile(string name)
    {
        var path = Path.Combine(_directory, name);
        File.WriteAllText(path, "x");
        return path;
    }

    /// <summary>
    /// Paths arrive from a process at a lower integrity level, over a pipe whose ACL is open to
    /// everyone on the machine. They are claims, not facts, and this is the only place that is
    /// enforced: a path that does not resolve to something on disk is dropped rather than stored.
    /// </summary>
    [Fact]
    public void Add_IgnoresAPathThatIsNotThere()
    {
        var store = new ShelfStore(_storePath);

        store.Add([Path.Combine(_directory, "does-not-exist-" + Guid.NewGuid() + ".txt")]);

        Assert.Empty(store.Items);
    }

    [Fact]
    public void Add_IgnoresAPathThatIsNotRooted()
    {
        var store = new ShelfStore(_storePath);

        store.Add(["relative.txt", "", "   "]);

        Assert.Empty(store.Items);
    }

    [Fact]
    public void Add_KeepsADirectory()
    {
        var store = new ShelfStore(_storePath);
        var sub = Path.Combine(_directory, "folder");
        Directory.CreateDirectory(sub);

        store.Add([sub]);

        Assert.True(Assert.Single(store.Items).IsDirectory);
    }

    [Fact]
    public void Add_KeepsTheMostRecentFirst()
    {
        var store = new ShelfStore(_storePath);

        store.Add([MakeFile("first.txt")]);
        store.Add([MakeFile("second.txt")]);

        Assert.Equal(["second.txt", "first.txt"], store.Items.Select(i => i.Name));
    }

    /// <summary>Dropping the same file twice is a person repeating themselves, not a request for
    /// two copies of one row.</summary>
    [Fact]
    public void Add_IsIdempotentForTheSamePath_AndMovesItToTheFront()
    {
        var store = new ShelfStore(_storePath);
        var first = MakeFile("first.txt");
        store.Add([first]);
        store.Add([MakeFile("second.txt")]);

        store.Add([first]);

        Assert.Equal(2, store.Items.Count);
        Assert.Equal("first.txt", store.Items[0].Name);
    }

    /// <summary>Windows paths are case-insensitive, so the same file reached two ways is one
    /// file. Comparing ordinally would let it in twice.</summary>
    [Fact]
    public void Add_TreatsCaseDifferencesAsTheSameFile()
    {
        var store = new ShelfStore(_storePath);
        var path = MakeFile("Mixed.TXT");

        store.Add([path]);
        store.Add([path.ToUpperInvariant()]);

        Assert.Single(store.Items);
    }

    [Fact]
    public void Add_StopsGrowingAtTheCap()
    {
        var store = new ShelfStore(_storePath);

        for (var i = 0; i < ShelfStore.MaxItems + 5; i++) store.Add([MakeFile($"f{i}.txt")]);

        Assert.Equal(ShelfStore.MaxItems, store.Items.Count);
        // The cap drops the oldest, so the newest must have survived it.
        Assert.Equal($"f{ShelfStore.MaxItems + 4}.txt", store.Items[0].Name);
    }

    [Fact]
    public void Remove_TakesOneOut()
    {
        var store = new ShelfStore(_storePath);
        var path = MakeFile("gone.txt");
        store.Add([path]);

        store.Remove(path);

        Assert.Empty(store.Items);
    }

    [Fact]
    public void Add_RaisesChangedOnceForABatch()
    {
        var store = new ShelfStore(_storePath);
        var raised = 0;
        store.Changed += () => raised++;

        store.Add([MakeFile("a.txt"), MakeFile("b.txt")]);

        Assert.Equal(1, raised);
    }

    [Fact]
    public void Add_DoesNotRaiseChangedWhenNothingWasKept()
    {
        var store = new ShelfStore(_storePath);
        var raised = 0;
        store.Changed += () => raised++;

        store.Add([Path.Combine(_directory, "nope.txt")]);

        Assert.Equal(0, raised);
    }

    [Fact]
    public void WhatWasStagedSurvivesARestart()
    {
        var kept = MakeFile("kept.txt");
        new ShelfStore(_storePath).Add([kept]);

        var reloaded = new ShelfStore(_storePath);

        Assert.Equal(kept, Assert.Single(reloaded.Items).Path);
    }

    /// <summary>
    /// A staged file can be deleted or moved between sessions, and a shelf that lists it anyway
    /// hands out a path that opens nothing. The check on load is the same one used on the way in.
    /// </summary>
    [Fact]
    public void LoadingDropsAnythingThatIsNoLongerOnDisk()
    {
        var doomed = MakeFile("doomed.txt");
        var survivor = MakeFile("survivor.txt");
        new ShelfStore(_storePath).Add([doomed, survivor]);

        File.Delete(doomed);

        Assert.Equal("survivor.txt", Assert.Single(new ShelfStore(_storePath).Items).Name);
    }

    [Fact]
    public void AMissingStoreFileIsAnEmptyShelfRatherThanAFailure()
    {
        Assert.Empty(new ShelfStore(Path.Combine(_directory, "never-written.txt")).Items);
    }

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
}
