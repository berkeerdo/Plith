using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Plith.Interop;

/// <summary>
/// Reads whether this process actually holds the UIAccess privilege.
///
/// It is worth knowing at runtime rather than inferring, because it silently decides whether
/// the OSD can cover a true exclusive-fullscreen game. Windows grants UIAccess only to a
/// binary that requests it in its manifest AND is signed AND runs from a trusted location, so
/// a Debug build out of bin\ never has it — `app.manifest` sets uiAccess="false" there on
/// purpose, and only `app.release.manifest` asks for it. Without the privilege
/// <see cref="BandWindow.BandWindow"/> falls back from CreateWindowInBand to a plain topmost
/// window, which still composites over borderless and Fullscreen-Optimizations games and
/// cannot cover an exclusive-fullscreen swapchain.
///
/// That distinction cost real time to rediscover from behaviour alone: the OSD appeared over
/// most games and not over one, which reads like a bug in the app rather than a property of
/// the build being run. One line in the log settles it.
/// </summary>
internal static class UiAccess
{
    private const int TokenUIAccess = 26;
    private const uint TOKEN_QUERY = 0x0008;

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool OpenProcessToken(nint processHandle, uint desiredAccess, out nint tokenHandle);

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool GetTokenInformation(nint tokenHandle, int tokenInformationClass,
        out uint tokenInformation, uint tokenInformationLength, out uint returnLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(nint handle);

    [DllImport("kernel32.dll")]
    private static extern nint GetCurrentProcess();

    /// <summary>
    /// True when this process holds UIAccess. Returns null if the token could not be read,
    /// which is reported as unknown rather than as false — claiming "no UIAccess" because a
    /// query failed would send the next investigation down the same wrong path this exists
    /// to prevent.
    /// </summary>
    public static bool? HasUiAccess()
    {
        nint token = 0;
        try
        {
            if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, out token)) return null;
            if (!GetTokenInformation(token, TokenUIAccess, out uint value, sizeof(uint), out _)) return null;
            return value != 0;
        }
        catch (Exception ex) when (ex is DllNotFoundException or EntryPointNotFoundException or Win32Exception)
        {
            return null;
        }
        finally
        {
            if (token != 0) CloseHandle(token);
        }
    }

    /// <summary>One-line summary for the diagnostic log, including what it implies.</summary>
    public static string Describe() => HasUiAccess() switch
    {
        true => "UIAccess granted — the OSD can be created in the topmost band and can cover exclusive-fullscreen games",
        false => "UIAccess NOT granted — the band window falls back to a plain topmost window, "
               + "which draws over borderless and Fullscreen-Optimizations games but not over a true "
               + "exclusive-fullscreen game. Expected for a Debug build; a signed install in Program Files should have it",
        _ => "UIAccess could not be determined",
    };
}
