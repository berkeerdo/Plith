namespace Plith.Services;

/// <summary>One cell of the output picker: a device, or the door to Windows' sound settings.</summary>
public sealed record OutputChoice(string Id, string Label, bool IsCurrent, bool IsOverflow = false);

/// <summary>
/// What the output picker's grid holds.
///
/// Pure, and a class of its own for the reason <c>NotchOpeningPolicy</c> is: the page that draws
/// this lives in a BandWindow the test project cannot construct, so a rule written inside it
/// would have no test at all.
/// </summary>
public static class OutputPickerModel
{
    /// <summary>What the last cell says when there are more devices than cells.</summary>
    public const string OverflowLabel = "More in Windows settings";

    /// <summary>
    /// The cells to draw, in order.
    ///
    /// The current output comes first because it is the anchor: it says the list is about the
    /// thing you are already hearing, and being first also means it can never be the one that
    /// falls off the end. The rest keep enumeration order, which is the order Windows itself
    /// lists them in.
    ///
    /// With more endpoints than cells the LAST cell is the overflow door and only
    /// <paramref name="capacity"/> minus one devices are drawn. Nothing is folded behind a count:
    /// a folded cell is in no UIA tree, invisible to a screen reader and reachable by no key,
    /// which is the shelf's own most expensive lesson.
    /// </summary>
    /// <param name="endpoints">Active render endpoints, in enumeration order.</param>
    /// <param name="currentId">The default endpoint's id, which may no longer be in the list.</param>
    /// <param name="capacity">How many cells the grid draws. See NotchGeometry.OutputPickerCapacity.</param>
    public static IReadOnlyList<OutputChoice> Cells(
        IReadOnlyList<WindowsAudioEndpointInfo> endpoints, string currentId, int capacity)
    {
        ArgumentNullException.ThrowIfNull(endpoints);
        // Two, not one: with a capacity of one and an overflow, there would be room for the door
        // and no device at all, which is a grid that shows nothing it is for.
        ArgumentOutOfRangeException.ThrowIfLessThan(capacity, 2);

        if (endpoints.Count == 0) return [];

        var ordered = endpoints
            .Where(e => IsCurrent(e, currentId))
            .Concat(endpoints.Where(e => !IsCurrent(e, currentId)))
            .ToList();

        var overflowing = ordered.Count > capacity;
        var room = overflowing ? capacity - 1 : capacity;

        var cells = ordered
            .Take(room)
            .Select(e => new OutputChoice(e.Id, e.FriendlyName, IsCurrent(e, currentId)))
            .ToList();

        if (overflowing)
            cells.Add(new OutputChoice(string.Empty, OverflowLabel, IsCurrent: false, IsOverflow: true));

        return cells;
    }

    // Ordinal: an endpoint id is an opaque token from Core Audio, not text anyone reads, so a
    // culture-aware comparison would only be a way to get it wrong.
    private static bool IsCurrent(WindowsAudioEndpointInfo endpoint, string currentId)
        => string.Equals(endpoint.Id, currentId, StringComparison.Ordinal);
}
