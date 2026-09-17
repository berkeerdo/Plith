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
    private readonly List<ShelfItem> _items = [];

    public ShelfStore() : this(DefaultStorePath()) { }

    public ShelfStore(string storePath)
    {
        _storePath = storePath;
        Load();
    }

    public event Action? Changed;

    /// <summary>Most recent first.</summary>
    public IReadOnlyList<ShelfItem> Items => _items;

    public static string DefaultStorePath() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith", "shelf.txt");

    /// <summary>
    /// Stage everything in <paramref name="paths"/> that resolves to something on disk, newest
    /// first. Raises <see cref="Changed"/> once for the batch, and not at all when nothing was
    /// kept — a drop of three paths is one event, and a drop of nothing is none.
    /// </summary>
    public void Add(IEnumerable<string> paths)
    {
        var kept = false;

        foreach (var path in paths)
        {
            if (!TryResolve(path, out var item)) continue;

            // Removed before inserting, so a repeat drop moves the row to the front instead of
            // creating a second one. Ordinal-ignore-case because Windows paths are.
            _items.RemoveAll(i => string.Equals(i.Path, item.Path, StringComparison.OrdinalIgnoreCase));
            _items.Insert(0, item);
            kept = true;
        }

        if (!kept) return;

        if (_items.Count > MaxItems) _items.RemoveRange(MaxItems, _items.Count - MaxItems);

        Save();
        Changed?.Invoke();
    }

    public void Remove(string path)
    {
        if (_items.RemoveAll(i => string.Equals(i.Path, path, StringComparison.OrdinalIgnoreCase)) == 0) return;

        Save();
        Changed?.Invoke();
    }

    public void Clear()
    {
        if (_items.Count == 0) return;

        _items.Clear();
        Save();
        Changed?.Invoke();
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
    /// One path per line, newest first. A text file rather than a serialized format because that
    /// is the whole content — and because a shelf file a person can read and edit in Notepad is
    /// a feature for something that holds references to their own files.
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
            // The same check as on the way in, because a staged file can be deleted or moved
            // between sessions and a row that opens nothing is worse than no row.
            if (_items.Count >= MaxItems) break;
            if (TryResolve(line, out var item)) _items.Add(item);
        }
    }
}
