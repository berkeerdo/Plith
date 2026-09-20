using System.IO;
using Plith.Services.Shelf;
using Plith.Views.Presentation;

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

    /// <summary>
    /// The cap is not a taste, it is the surface's capacity. A shelf that holds more than the
    /// grid draws puts a file out of reach: the stack build capped at 20 against a surface that
    /// drew 10, and the other ten were in no UIA tree at all.
    /// </summary>
    [Fact]
    public void TheCapIsWhatTheSurfaceCanDraw()
    {
        Assert.Equal(NotchGeometry.ShelfCapacity, ShelfStore.MaxItems);
    }

    /// <summary>
    /// One flat list, newest first. The stack model put a new item at the front of the front
    /// stack; with no stacks there is one front.
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
    /// The cap is the surface's capacity, so a file that is on the shelf is on the screen. Past
    /// it the oldest falls off, which is the end of the list.
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
    /// A file written by the stack build separated its groups with blank lines. Loading skips
    /// them, so an existing shelf survives the change as one flat list in the order it already
    /// had, and no migration code is needed.
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

    /// <summary>And it writes the new form: one path per line, no blank lines at all.</summary>
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
}
