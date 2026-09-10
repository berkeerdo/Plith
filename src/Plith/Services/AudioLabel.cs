namespace Plith.Services;

/// <summary>Which rail a level belongs to. Only used to name it.</summary>
public enum AudioRail
{
    VoicemeeterBus,
    VoicemeeterStrip,
    WindowsEndpoint,
}

/// <summary>
/// The two strings on the audio card: what the device is called, and which rail it is.
///
/// Pure and here rather than inline in a view, for a reason with a scar on it. The shortener
/// below already existed — as a private method on <see cref="WindowsAudioClient"/> — and was
/// called from the settings dropdown but not from the card, so the same endpoint appeared
/// shortened in one place and raw in the other. It has been moved rather than copied: a second
/// copy is the thing that drifts.
/// </summary>
public static class AudioLabel
{
    /// <summary>
    /// Strips the parenthesised adapter suffix Windows appends to endpoint names when it says
    /// nothing the head has not already said, e.g.
    /// <c>"SteelSeries Sonar - Chat (SteelSeries Sonar Virtual Audio Device)"</c> becomes
    /// <c>"SteelSeries Sonar - Chat"</c>.
    ///
    /// Otherwise the name is left alone, so two endpoints sharing a name on different drivers
    /// stay tellable apart. Display only: the raw endpoint id is what is persisted and matched.
    /// </summary>
    /// <remarks>
    /// <b>Corrected while moving this out of WindowsAudioClient, 2026-09-10.</b> Both examples
    /// in the original doc comment described behaviour the code did not have, and tests written
    /// from that comment failed against it:
    ///
    /// - The Sonar case did NOT collapse. The redundancy check was <c>head.Contains(tail)</c>,
    ///   and the head contains only the adapter's first two words, never the whole descriptor —
    ///   so it produced <c>"SteelSeries Sonar - Chat (SteelSeries Sonar)"</c>, saying the same
    ///   thing twice, which is exactly what it was written to prevent.
    /// - <c>"Hoparlör (Realtek(R) Audio)"</c> did NOT become <c>"Hoparlör (Realtek)"</c>. The
    ///   adapter is two words, so the two-word hint reproduced it whole and the name came back
    ///   unchanged.
    ///
    /// The redundancy check now compares the head against the adapter's first two words, which
    /// is the signal it was reaching for. This shipped in the settings dropdown for two phases;
    /// nothing depended on the shortened form, so the correction is display-only.
    /// </remarks>
    public static string Shorten(string? full)
    {
        // An empty name is reachable: a device can report one, and a failed read falls back to
        // whatever the caller had. Returning "" would leave the card's device line blank with
        // no clue that anything is attached at all.
        if (string.IsNullOrWhiteSpace(full)) return "Unknown device";

        full = full.Trim();

        int paren = full.LastIndexOf(" (", StringComparison.Ordinal);
        if (paren <= 0 || !full.EndsWith(')')) return full;

        var head = full[..paren];
        var tail = full.Substring(paren + 2, full.Length - paren - 3);   // strip " (" and ")"

        var words = tail.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        var shortAdapter = words.Length switch
        {
            0 => tail,
            1 => words[0],
            _ => words[0] + " " + words[1],
        };

        // The head already naming the adapter is the Sonar case: Chat, Gaming and Media all
        // bundle the driver name into the head, so repeating it in the parenthesis says the
        // same thing twice. Compared against the adapter's first two words rather than the
        // whole descriptor — see the remarks above for why the other way round never fired.
        if (head.Contains(shortAdapter, StringComparison.OrdinalIgnoreCase)) return head;

        // Nothing was actually shortened. Returning the reconstructed string would be the same
        // characters through a longer path, and a "shortened" name identical to the original is
        // worth leaving visibly untouched.
        if (string.Equals(shortAdapter, tail, StringComparison.Ordinal)) return full;

        return $"{head} ({shortAdapter})";
    }

    /// <summary>
    /// Which rail this level is, in words.
    ///
    /// On a machine running Voicemeeter alongside Sonar alongside a headset, a number with no
    /// rail attached to it is ambiguous — and that is the machine this app was built for.
    ///
    /// Muted wins over the rail. When the sound is off, which bus it is off on is the second
    /// question, not the first.
    /// </summary>
    public static string BusLine(AudioRail rail, int index, bool muted)
    {
        if (muted) return "Muted";

        return rail switch
        {
            AudioRail.VoicemeeterBus => $"Voicemeeter · {BusName(index)}",
            AudioRail.VoicemeeterStrip => $"Voicemeeter · Strip {index + 1}",
            _ => "Windows · Output",
        };
    }

    /// <summary>Voicemeeter's own names for its buses. Out-of-range indices fall back to the
    /// number rather than throwing: the index comes from a settings file a person can edit.</summary>
    private static string BusName(int index) => index switch
    {
        0 => "A1", 1 => "A2", 2 => "A3", 3 => "A4",
        4 => "A5", 5 => "B1", 6 => "B2", 7 => "B3",
        _ => $"Bus {index}",
    };
}
