namespace Plith.Services;

/// <summary>One reading of current conditions. Immutable and timestamped: the ambient row
/// shows it only while it is fresh, and freshness is decided against the fetch time rather
/// than against a "last updated" flag that a failed refresh would leave stale.</summary>
public readonly record struct WeatherSnapshot(double TemperatureC, int WeatherCode, DateTimeOffset FetchedAt);

/// <summary>
/// Turns Open-Meteo's WMO 4677 weather codes into something renderable, and decides whether
/// a snapshot is still worth showing.
///
/// Ranges rather than a case per code: WMO defines about thirty, most of which differ only in
/// intensity ("slight" vs "moderate" drizzle), and the row has space for one glyph and one
/// short word. Collapsing them is a display decision, which is why it lives here and not in
/// the HTTP client.
/// </summary>
public static class WeatherCodeMap
{
    /// <summary>The icon font every glyph below is a code point in. A single constant so the
    /// view (Task 7) and the test that verifies these code points actually exist cannot drift
    /// apart into two hand-typed literals.</summary>
    public const string GlyphFontFamilyName = "Segoe MDL2 Assets";

    /// <summary>File backing <see cref="GlyphFontFamilyName"/>, for tools (like
    /// <see cref="System.Windows.Media.GlyphTypeface"/>) that need the font file rather than
    /// the family name. Ships with Windows 10 and 11 — no font is downloaded and none has to
    /// be embedded.</summary>
    public const string GlyphFontFilePath = @"C:\Windows\Fonts\segmdl2.ttf";

    /// <summary>
    /// Glyphs are Segoe MDL2 Assets code points, chosen only from glyphs confirmed present in
    /// that font via <see cref="System.Windows.Media.GlyphTypeface.CharacterToGlyphMap"/> (see
    /// <c>WeatherCodeMapTests.Describe_EveryGlyphExistsInTheDeclaredFont</c>) — MDL2 has no
    /// dedicated icon for most weather conditions, and several plausible-looking code points in
    /// this range (the E9C0-E9CD span) are simply undefined and render as a hollow "tofu" box.
    /// Where no dedicated glyph exists, an adjacent documented glyph is reused deliberately:
    /// Cloud for every degree of overcast including fog (fog is a cloud at ground level), Drop
    /// for every liquid precipitation state, Frigid (a cold-temperature glyph) for the two snow
    /// states, and LightningBolt for storms — the same kind of defensible reuse Clear already
    /// made with Brightness.
    /// </summary>
    public static (string Glyph, string Label) Describe(int code) => code switch
    {
        0        => ("\uE706", "Clear"),          // Brightness
        1 or 2   => ("\uE753", "Partly cloudy"),  // Cloud
        3        => ("\uE753", "Overcast"),       // Cloud
        45 or 48 => ("\uE753", "Fog"),            // Cloud
        >= 51 and <= 57 => ("\uEB42", "Drizzle"),      // Drop
        >= 61 and <= 67 => ("\uEB42", "Rain"),         // Drop
        >= 71 and <= 77 => ("\uE9CA", "Snow"),         // Frigid
        >= 80 and <= 82 => ("\uEB42", "Showers"),      // Drop
        >= 85 and <= 86 => ("\uE9CA", "Snow showers"), // Frigid
        >= 95 => ("\uE945", "Thunderstorm"),           // LightningBolt

        // Deliberately not an exception. Open-Meteo can add codes, and this runs on a
        // background refresh timer where a throw would silently stop every future refresh
        // for the sake of a decoration.
        _ => ("\uE9CE", "Unknown"), // Unknown
    };

    /// <summary>
    /// Whether a snapshot is still worth rendering. A stale reading is treated as absent
    /// rather than shown: a temperature from three hours ago is a wrong answer presented as
    /// a right one, and the column collapsing is honest.
    ///
    /// A future timestamp fails too. It is reachable through a clock change or a DST jump
    /// between the fetch and the render, and "negative age" would otherwise pass the
    /// comparison and pin a bad reading on screen until the next successful refresh.
    /// </summary>
    public static bool IsFresh(WeatherSnapshot snapshot, DateTimeOffset now, int maxAgeMinutes)
    {
        var age = now - snapshot.FetchedAt;
        return age >= TimeSpan.Zero && age <= TimeSpan.FromMinutes(maxAgeMinutes);
    }
}
