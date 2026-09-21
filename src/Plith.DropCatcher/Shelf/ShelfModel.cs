using System.IO;
using Plith.Views.Presentation;

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
    private readonly List<ShelfEntry> _items = [];
    private readonly HashSet<string> _selection = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>What the catcher believes is on the shelf, newest first.</summary>
    public IReadOnlyList<ShelfEntry> Items => _items;

    public IReadOnlyCollection<string> Selection => _selection;

    /// <summary>
    /// Replace the shelf with what Plith just sent.
    ///
    /// One message carries the whole shelf, which is why this is a replace and not an assembly.
    /// The stack build sent one message PER STACK and had to defend against two deliveries
    /// interleaving on a pipe any local process may write: position came from a declared index
    /// rather than from arrival order, a message was accepted only into the delivery currently
    /// being assembled, and the declared stack total was clamped so a stranger could not make
    /// this process allocate slots on its say-so. That was the most intricate code in the shelf,
    /// and none of it has anything left to defend: there is one message, so there is no
    /// assembly to corrupt and nothing for a second delivery to interleave with.
    ///
    /// The hostile input does not go away, it gets SMALLER. The list is truncated to what the
    /// surface can draw, which bounds a list that has already arrived rather than an allocation
    /// made ahead of it on a number a stranger chose.
    /// </summary>
    public void SetItems(IReadOnlyList<string> paths)
    {
        _items.Clear();
        foreach (var path in paths)
        {
            if (_items.Count >= NotchGeometry.ShelfCapacity) break;
            _items.Add(Describe(path));
        }

        // A path that has gone away since Plith sent it stays selected otherwise, and a drag
        // would then carry a file that is not on the shelf any more.
        _selection.RemoveWhere(p => !_items.Any(e => PathEquals(e.Path, p)));
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
