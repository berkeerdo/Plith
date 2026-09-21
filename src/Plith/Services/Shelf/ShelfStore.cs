using System.IO;
using Plith.Views.Presentation;

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
    /// The shelf is a staging area, not an archive. Past this many rows the oldest falls off.
    ///
    /// The number is the SURFACE'S CAPACITY, not a taste: see NotchGeometry.ShelfCapacity. A cap
    /// larger than what the grid draws puts a file on the shelf and off the screen, which is what
    /// the stack model did with 20 against a surface that could draw 10.
    /// </summary>
    public const int MaxItems = NotchGeometry.ShelfCapacity;

    private readonly string _storePath;
    private readonly List<ShelfItem> _items = [];

    public ShelfStore() : this(DefaultStorePath()) { }

    public ShelfStore(string storePath)
    {
        _storePath = storePath;
        Load();
    }

    public event Action? Changed;

    /// <summary>Everything on the shelf, newest first.</summary>
    public IReadOnlyList<ShelfItem> Items => _items;

    public static string DefaultStorePath() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Plith", "shelf.txt");

    /// <summary>
    /// Stage everything in <paramref name="paths"/> that resolves to something on disk, newest
    /// first. Raises <see cref="Changed"/> once for the batch, and not at all when nothing was
    /// kept. A drop of three paths is one event, and a drop of nothing is none.
    /// </summary>
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

    public void Remove(string path) => RemoveMany([path]);

    public void RemoveMany(IEnumerable<string> paths)
    {
        var removed = 0;
        foreach (var path in paths)
            removed += _items.RemoveAll(i => string.Equals(i.Path, path, StringComparison.OrdinalIgnoreCase));

        if (removed == 0) return;

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

    /// <summary>Oldest first off, which is the end of a newest-first list.</summary>
    private void TrimToCap()
    {
        if (_items.Count <= MaxItems) return;

        _items.RemoveRange(MaxItems, _items.Count - MaxItems);
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
    /// One path per line, newest first. Still a text file a person can read and edit in Notepad,
    /// which was a deliberate property before stacks existed and survives their removal.
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
            // A blank line was the stack separator in the previous format. Skipping it is the
            // whole of the migration: a shelf written by the stack build loads as one flat list
            // in the order it already had, with no version marker anywhere.
            if (string.IsNullOrWhiteSpace(line)) continue;
            if (_items.Count >= MaxItems) break;

            // The same check as on the way in, because a staged file can be deleted or moved
            // between sessions and a row that opens nothing is worse than no row.
            if (!TryResolve(line, out var item)) continue;

            _items.Add(item);
        }
    }
}
