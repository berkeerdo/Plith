using System.IO;

namespace Plith.Services.Shelf;

/// <param name="Path">The full path as it exists on disk.</param>
/// <param name="IsDirectory">Folders are staged as readily as files; the shelf holds whatever
/// the shell handed over.</param>
public readonly record struct ShelfItem(string Path, string Name, bool IsDirectory, DateTime AddedUtc);

/// <summary>
/// What is on the shelf, and where it survives a restart.
///
/// The shelf stages REFERENCES. Nothing is copied, nothing is moved, and removing a row does not
/// touch the file — which is why the catcher reports Copy rather than Move as its drag effect. A
/// shelf that took files away from where they were would lose someone's work the first time they
/// mistook it for a pocket.
///
/// Every path here arrived from the drop catcher: a process at a lower integrity level, reached
/// over a pipe whose ACL is deliberately open to everyone on the machine. So the paths are claims
/// rather than facts, and this class is the one place that is enforced — it stats what it is told
/// about and keeps nothing it cannot see. That check is not security theatre against a hostile
/// process (anything that can write the pipe can also read the disk); it is what keeps the shelf
/// from listing rows that open nothing.
/// </summary>
public sealed class ShelfStore
{
    /// <summary>
    /// The shelf is a staging area, not an archive. Past this many rows the oldest falls off,
    /// because the alternative is a list nobody prunes and a notch page that cannot show it.
    /// </summary>
    public const int MaxItems = 20;

    private readonly string _storePath;
    private readonly List<List<ShelfItem>> _stacks = [];

    public ShelfStore() : this(DefaultStorePath()) { }

    public ShelfStore(string storePath)
    {
        _storePath = storePath;
        Load();
    }

    public event Action? Changed;

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

    public static string DefaultStorePath() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith", "shelf.txt");

    /// <summary>
    /// Stage everything in <paramref name="paths"/> that resolves to something on disk, newest
    /// first, joining the front stack. Raises <see cref="Changed"/> once for the batch, and not
    /// at all when nothing was kept — a drop of three paths is one event, and a drop of nothing
    /// is none.
    /// </summary>
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

    private static bool TryResolve(string path, out ShelfItem item)
    {
        item = default;
        if (string.IsNullOrWhiteSpace(path)) return false;

        string full;
        try
        {
            // Rooted only. A relative path from the catcher would resolve against THIS process's
            // working directory, which has nothing to do with where the file the person dragged
            // actually is.
            if (!Path.IsPathRooted(path)) return false;
            full = Path.GetFullPath(path);
        }
        catch (ArgumentException) { return false; }
        catch (PathTooLongException) { return false; }
        catch (NotSupportedException) { return false; }

        var isDirectory = Directory.Exists(full);
        if (!isDirectory && !File.Exists(full)) return false;

        var name = isDirectory
            ? new DirectoryInfo(full).Name
            : Path.GetFileName(full);

        // A drive root has no file name and no directory name; the path itself is the only label
        // there is for it.
        if (string.IsNullOrEmpty(name)) name = full;

        item = new ShelfItem(full, name, isDirectory, DateTime.UtcNow);
        return true;
    }

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
}
