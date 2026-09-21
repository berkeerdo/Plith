using System.Globalization;
using System.Runtime.InteropServices;
using System.Security.Principal;

namespace Plith.DropCatcher;

/// <summary>
/// Reads this process's own integrity level.
///
/// Logged at every start, because it is the single fact that decides whether the catcher can do
/// its job at all, and it is not visible from anywhere else. Launched the obvious way — as a
/// child of Plith — this process inherits Plith's HIGH token and becomes exactly as unable to
/// receive a drop as the window it exists to stand in for, while looking completely healthy:
/// it starts, it connects, it shows itself, and no drop ever arrives. That failure is
/// indistinguishable from the bug it was built to fix, so it is named here rather than guessed
/// at later.
/// </summary>
internal static class IntegrityLevel
{
    private const int TokenIntegrityLevel = 25;

    [DllImport("advapi32.dll", SetLastError = true)]
    private static extern bool GetTokenInformation(
        nint tokenHandle, int tokenInformationClass, nint tokenInformation, int length, out int returnLength);

    /// <summary>The well-known RIDs, from WinNT.h. Medium is what this process must be.</summary>
    public static string Describe()
    {
        var rid = Rid();
        var name = rid switch
        {
            null => "unknown",
            0x0000 => "UNTRUSTED",
            0x1000 => "LOW",
            0x2000 => "MEDIUM",
            0x2100 => "MEDIUM PLUS",
            0x3000 => "HIGH",
            0x4000 => "SYSTEM",
            _ => "RID " + rid.Value.ToString(CultureInfo.InvariantCulture),
        };

        return rid == 0x2000
            ? name
            : $"{name} — this process must be MEDIUM or it cannot receive a drop";
    }

    public static bool IsMedium() => Rid() == 0x2000;

    private static int? Rid()
    {
        using var identity = WindowsIdentity.GetCurrent();
        var token = identity.AccessToken.DangerousGetHandle();

        _ = GetTokenInformation(token, TokenIntegrityLevel, 0, 0, out var size);
        if (size <= 0) return null;

        var buffer = Marshal.AllocHGlobal(size);
        try
        {
            if (!GetTokenInformation(token, TokenIntegrityLevel, buffer, size, out _)) return null;

            // TOKEN_MANDATORY_LABEL is a single SID_AND_ATTRIBUTES: a pointer, then attributes.
            var sidPointer = Marshal.ReadIntPtr(buffer);
            var sid = new SecurityIdentifier(sidPointer).Value;

            // S-1-16-<rid>. The RID is the last sub-authority and the only part that matters.
            var lastDash = sid.LastIndexOf('-');
            return lastDash >= 0
                   && int.TryParse(sid.AsSpan(lastDash + 1), NumberStyles.Integer, CultureInfo.InvariantCulture, out var rid)
                ? rid
                : null;
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }
}
