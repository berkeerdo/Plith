namespace Plith.Services.Shelf;

/// <summary>
/// The short caption a shelf tile carries under its picture.
///
/// A tile is 56 DIP wide and a caption gets one line of about ten characters, so this is not
/// "the file name, trimmed": it is a decision about WHICH ten characters, and the decision is
/// what makes the caption worth its space.
///
/// Reported from a real session, after the captions had been deleted entirely: with several files
/// of the same type the icons are identical, so you hover each one in turn trying to remember
/// which is which. The picture answers "what kind of thing is this" and cannot answer "which one".
///
/// Shared, linked into the catcher project rather than copied into it, for the reason DropChannel
/// and NotchGeometry are: two copies of a rule about names drift, and this one has tests.
/// </summary>
public static class ShelfLabel
{
    /// <summary>
    /// The caption for <paramref name="name"/>, at most <paramref name="maxChars"/> characters.
    ///
    /// TWO DECISIONS, and both come from what the caption is for.
    ///
    /// The EXTENSION GOES. The tile's picture is the shell's own icon for that extension, or the
    /// file's own preview, so ".xlsx" under a green X spends a quarter of the caption saying what
    /// the picture already said. A name that is nothing but an extension keeps it, because
    /// ".gitignore" with the extension removed is nothing at all.
    ///
    /// The MIDDLE GOES, not the end. Trimming from the right is what an ellipsis does by default
    /// and it is the wrong half here: files of the same type on one shelf are usually one export
    /// series, and what separates them is the tail. Measured on this repo's own fixture,
    /// "NM_Mukellef_Veri_Dosyasi_2026-09-21" trimmed from the right is "NM_Mukel...", which is
    /// the same string for every file in the series; trimmed in the middle it is "NM_Mu…09-21",
    /// which is the one part that differs.
    /// </summary>
    public static string Short(string name, int maxChars)
    {
        if (string.IsNullOrEmpty(name)) return string.Empty;
        if (maxChars <= 0) return string.Empty;

        var stem = StripExtension(name);
        if (stem.Length <= maxChars) return stem;

        // The ellipsis costs one character of the budget, so the two halves share what is left.
        // The HEAD gets the odd character when the budget is odd: a name is read from the left,
        // and the first thing a person matches against is how it starts.
        var budget = maxChars - 1;
        var head = (budget + 1) / 2;
        var tail = budget - head;

        return tail == 0
            ? string.Concat(stem.AsSpan(0, head), "…")
            : string.Concat(stem.AsSpan(0, head), "…", stem.AsSpan(stem.Length - tail));
    }

    /// <summary>
    /// The name without its extension, or the name itself when removing it would leave nothing.
    ///
    /// Not Path.GetFileNameWithoutExtension: that returns an empty string for ".gitignore", and a
    /// caption of nothing is worse than a caption of the extension. A trailing dot is left alone
    /// for the same reason.
    /// </summary>
    private static string StripExtension(string name)
    {
        var dot = name.LastIndexOf('.');
        if (dot <= 0) return name;                    // no dot, or a leading dot: keep it whole
        if (dot == name.Length - 1) return name;      // "report." keeps its dot rather than losing it

        return name[..dot];
    }
}
