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
    // A null slot means "no message for this index has arrived yet", which is a different thing
    // from an arrived-and-empty stack. Sizing this list up front to the total the FIRST message
    // of a set declares, rather than growing it by appending as messages happen to arrive, is
    // what makes SetStack place a stack at the index it was given instead of wherever it landed
    // in arrival order. See SetStack for why arrival order cannot be trusted at all.
    private readonly List<List<ShelfEntry>?> _slots = [];
    private readonly HashSet<string> _selection = new(StringComparer.OrdinalIgnoreCase);
    private int _expected = -1;

    public IReadOnlyList<IReadOnlyList<ShelfEntry>> Stacks =>
        [.. _slots.Where(s => s is not null).Select(s => (IReadOnlyList<ShelfEntry>)s!)];

    /// <summary>True once every slot in the set has a message in it. Checking every slot,
    /// not just counting how many messages have arrived, matters exactly because SetStack no
    /// longer trusts arrival order: a duplicate or misplaced message must not be able to count
    /// twice toward completion the way a simple counter would let it.</summary>
    public bool IsComplete => _expected >= 0 && _slots.TrueForAll(s => s is not null);

    public IReadOnlyCollection<string> Selection => _selection;

    /// <summary>
    /// Places <paramref name="paths"/> at stack <paramref name="index"/> of a set of
    /// <paramref name="total"/> stacks.
    ///
    /// This does NOT simply trust that messages arrive in the order they were sent, even though
    /// the sender now guarantees it. DropChannelServer.SendAsync opens a fresh StreamWriter per
    /// call and every call site fires it without awaiting the result, so two overlapping sends
    /// (two shelf changes close enough together to both be in flight) could interleave on the one
    /// pipe the catcher reads; Task 6 chained the sends so they cannot, which is the send-side
    /// fix the last paragraph below asks for. The checks here stay all the same: this process
    /// reads a pipe ANY process on the machine may write, so "the sender is well behaved" is a
    /// statement about one sender rather than about the input. A model that appended each message
    /// to wherever the list currently ends, as this one used to,
    /// would then assemble a shelf out of two different deliveries and call it complete: the
    /// result LOOKS like an ordinary shelf and is quietly wrong, which is worse than looking
    /// incomplete, because nothing about it invites a second look.
    ///
    /// So position always comes from `index`, never from arrival order, and a message is only
    /// accepted into the delivery currently being assembled. Index 0 always starts a NEW
    /// delivery (the shelf is re-sent whole on every change, so a delivery that never finished is
    /// superseded rather than merged with whatever follows). A message for index > 0 is folded in
    /// only if the `total` it carries matches `_expected`: an index-0 message not yet seen for
    /// this delivery leaves `_expected` describing the PREVIOUS delivery or nothing at all, and a
    /// newer delivery already under way leaves it describing THAT one instead. Comparing the
    /// declared total catches both an EARLY message (its own index-0 sibling has not arrived) and
    /// a LATE, stale one (a new delivery has already started) without needing a session id the
    /// wire format does not carry. What it cannot catch is two deliveries of the exact same size
    /// racing each other byte-for-byte; closing that gap means the sends must stop interleaving
    /// in the first place, which is the send-side fix this model's own limits push toward rather
    /// than paper over.
    /// </summary>
    public void SetStack(int index, int total, IReadOnlyList<string> paths)
    {
        if (index <= 0)
        {
            _expected = total;
            _slots.Clear();
            for (var i = 0; i < total; i++) _slots.Add(null);
            if (total > 0) _slots[0] = [.. paths.Select(Describe)];
        }
        else
        {
            if (total != _expected || index >= _slots.Count) return;
            _slots[index] = [.. paths.Select(Describe)];
        }

        // A path that has gone away since Plith sent it stays selected otherwise, and a drag
        // would then carry a file that is not on the shelf any more.
        _selection.RemoveWhere(p => !_slots.Any(s => s is not null && s.Any(e => PathEquals(e.Path, p))));
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
