using System.Globalization;
using System.Text;

namespace Plith.Services.Shelf;

public enum DropVerb
{
    /// <summary>The catcher announcing itself. Carries no payload; it exists so Plith knows a
    /// catcher is alive before it hides the notch for one.</summary>
    Hello,

    /// <summary>Take the notch's place: the rectangle travels in X/Y/W/H.</summary>
    Show,

    /// <summary>Give it back.</summary>
    Hide,

    /// <summary>Files were released on the catcher.</summary>
    Dropped,

    /// <summary>Plith has stood down for the shelf rather than for a drop. The rectangle to grow
    /// out of travels in X/Y/W/H, the same way Show carries it.</summary>
    OpenShelf,

    /// <summary>The whole shelf, in one message: the paths, newest first. X and Y carry nothing.
    ///
    /// It used to be one message per stack, with an index and a total in X and Y. Stacks are gone
    /// and so is the reassembly that needed them.</summary>
    Items,

    /// <summary>The resolved theme, as seven values. See ShelfPaletteWire.</summary>
    Palette,

    /// <summary>Catcher to Plith: take these off the shelf.</summary>
    RemoveItems,

    /// <summary>Catcher to Plith: empty the shelf.</summary>
    ClearShelf,

    /// <summary>Catcher to Plith: the shelf surface is gone, put the notch back.</summary>
    ShelfClosed,

    /// <summary>
    /// Catcher to Plith: the shelf's window is ON SCREEN. Plith may take its own window down.
    ///
    /// THE ONLY ACKNOWLEDGEMENT ON THIS WIRE, and it exists because its absence was visible.
    /// Plith used to hide the instant it had sent OpenShelf, and the catcher's window arrives
    /// about 25 ms later and then faded in over 150: between the two there was nothing on screen
    /// at all, which a person reported as the shelf closing and reopening, "like a double shelf".
    ///
    /// Every other verb here is fire-and-forget on purpose, and the comments on Open and Opened
    /// say why: waiting for an answer that may never come would leave Plith's window hidden
    /// behind a shelf that never appeared. This one is answered with a TIMEOUT rather than
    /// trusted, so the worst case is the old behaviour rather than a missing notch.
    /// </summary>
    ShelfShown,

    /// <summary>
    /// Plith to catcher: take the shelf down.
    ///
    /// New with the shelf becoming a page in the notch's frame. Until then the shelf only ever
    /// closed because the pointer left it, so Plith had nothing to say; now PAGING AWAY from the
    /// shelf page has to close it, and only Plith knows a page turned. The catcher's CloseNow
    /// already expected this message to exist: it defers one that arrives mid-drag and honours it
    /// when the drag ends.
    /// </summary>
    CloseShelf,

    /// <summary>
    /// Plith to catcher: how the notch's page rail should look while the catcher holds the frame.
    /// X is how many pages there are, Y is the index of the shelf's own page.
    ///
    /// It carried a slide direction in W for one commit. That is gone with the slide: two
    /// processes cannot animate one page turn across a window handover, and the attempt read as
    /// stuttering.
    ///
    /// The catcher cannot know either. Only Plith knows how many widget pages are installed,
    /// which depends on settings (the weather page comes and goes), and without this the rail
    /// would simply vanish on one page out of five and come back on the others, which reads as
    /// the shelf being a different thing.
    /// </summary>
    Rail,

    /// <summary>
    /// Catcher to Plith: a paging gesture happened on the shelf page. X is a raw wheel delta and
    /// Y is a page index; exactly one of the two is meaningful and the other is zero.
    ///
    /// THE DELTA IS RAW, deliberately. WheelDecoder and NotchPager stay in Plith, so the commit
    /// threshold and the idle rearm have one definition rather than one per process. The catcher
    /// reports what the hardware did; Plith decides what it means.
    /// </summary>
    Page,
}

/// <param name="X">Physical screen pixels, not DIP, and the same for Y/W/H.
/// Plith reads them straight off its own window handle with GetWindowRect and the catcher applies
/// them with SetWindowPos, so neither end converts and neither end has to agree with the other
/// about what a DIP is. Two processes that each do their own DIP arithmetic would disagree on any
/// monitor that is not at 100%, and put the catcher somewhere other than the notch.</param>
/// <param name="Paths">Only ever populated on <see cref="DropVerb.Dropped"/>, and always from
/// the catcher — which runs at a lower integrity level than this process. Untrusted.</param>
public readonly record struct DropMessage(
    DropVerb Verb, double X, double Y, double W, double H, IReadOnlyList<string> Paths);

/// <summary>
/// The wire between Plith and the drop catcher.
///
/// One file, compiled into both, so the two ends cannot drift into disagreeing about the format.
/// Text rather than a serializer: the payload is four numbers and a list of paths, and a
/// serializer would be a dependency plus an attack surface for the sake of nothing.
/// </summary>
public static class DropChannel
{
    private const char Separator = '\t';
    private const char Escape = '\\';

    /// <summary>Per-user, because two signed-in users each get their own Plith and their own
    /// catcher, and a shared name would let one session's catcher answer another's.</summary>
    public static string PipeName(string userSid) => $"Plith.DropCatcher.{userSid}";

    public static string Encode(DropMessage message)
    {
        var line = new StringBuilder();
        line.Append(message.Verb.ToString());

        // Invariant, because the two ends are separate processes and need not share a locale.
        // Under a comma-decimal culture the encoder would write "708,5" and the decoder — which
        // parses invariant — would read 0, putting the catcher somewhere other than the notch.
        foreach (var n in new[] { message.X, message.Y, message.W, message.H })
            line.Append(Separator).Append(n.ToString("R", CultureInfo.InvariantCulture));

        foreach (var path in message.Paths)
        {
            line.Append(Separator);
            AppendEscaped(line, path);
        }

        return line.ToString();
    }

    public static bool TryDecode(string line, out DropMessage message)
    {
        message = default;
        if (string.IsNullOrEmpty(line)) return false;

        var parts = line.Split(Separator);
        if (parts.Length < 5) return false;
        // A NAME round trip, not Enum.IsDefined alone, and the difference is the whole check.
        //
        // TryParse accepts a NUMBER for any enum. "3" decodes to Dropped, which IsDefined would
        // happily confirm, so IsDefined by itself lets a verb through that was reached by
        // counting rather than by name. But IsDefined is still needed alongside the round trip:
        // for a value with no name at all, such as 99, ToString falls back to printing the
        // number itself, so a name round trip on its own would let "99" through too. A pipe every
        // process on this machine can write is precisely the place a verb must not be reachable
        // by counting, so both checks are required: the value must be one of ours, and the sender
        // must have written its name rather than its number.
        if (!Enum.TryParse<DropVerb>(parts[0], out var verb)) return false;
        if (!Enum.IsDefined(verb)) return false;
        if (!string.Equals(verb.ToString(), parts[0], StringComparison.Ordinal)) return false;

        var numbers = new double[4];
        for (var i = 0; i < 4; i++)
        {
            if (!double.TryParse(parts[i + 1], NumberStyles.Float, CultureInfo.InvariantCulture, out numbers[i]))
                return false;
        }

        var paths = new string[parts.Length - 5];
        for (var i = 0; i < paths.Length; i++) paths[i] = Unescape(parts[i + 5]);

        message = new DropMessage(verb, numbers[0], numbers[1], numbers[2], numbers[3], paths);
        return true;
    }

    private static void AppendEscaped(StringBuilder target, string value)
    {
        foreach (var c in value)
        {
            switch (c)
            {
                case Escape: target.Append(Escape).Append(Escape); break;
                case Separator: target.Append(Escape).Append('t'); break;
                case '\n': target.Append(Escape).Append('n'); break;
                case '\r': target.Append(Escape).Append('r'); break;
                default: target.Append(c); break;
            }
        }
    }

    /// <summary>
    /// A single left-to-right pass, and that is the whole point of it: a backslash consumes the
    /// character after it, whatever that character is. The obvious implementation — three chained
    /// Replace calls — corrupts ordinary Windows paths instead, because escaping doubles every
    /// separator and a later pass then reads the second backslash of a pair as the start of an
    /// escape. A path under a directory whose name begins with t comes back with a tab in it.
    /// </summary>
    private static string Unescape(string value)
    {
        if (!value.Contains(Escape, StringComparison.Ordinal)) return value;

        var result = new StringBuilder(value.Length);
        for (var i = 0; i < value.Length; i++)
        {
            if (value[i] != Escape || i + 1 == value.Length)
            {
                result.Append(value[i]);
                continue;
            }

            i++;
            result.Append(value[i] switch
            {
                't' => Separator,
                'n' => '\n',
                'r' => '\r',
                // Includes the backslash pair, and anything the other end escaped that this one
                // does not know about: the escape is dropped and the character kept, which is the
                // lossless reading of a format neither side is free to extend alone.
                var other => other,
            });
        }

        return result.ToString();
    }
}
