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

    /// <summary>Files were released on the catcher. The only verb that carries paths.</summary>
    Dropped,
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
        if (!Enum.TryParse<DropVerb>(parts[0], out var verb)) return false;

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
