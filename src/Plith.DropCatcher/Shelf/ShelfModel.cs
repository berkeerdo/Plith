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
