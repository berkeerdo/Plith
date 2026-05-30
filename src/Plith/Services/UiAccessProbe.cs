using Plith.Interop;

namespace Plith.Services;

/// <summary>
/// Reports whether the current process can use the UIAccess + CreateWindowInBand path
/// to draw above exclusive fullscreen games. True only when the underlying Win32 API
/// is available AND the process token carries the UIAccess privilege (granted by
/// Windows when a signed binary in a trusted location requests it via app.manifest).
/// </summary>
public static class UiAccessProbe
{
    public static bool IsGameModeActive()
    {
        if (!NativeMethods.IsBandWindowSupported()) return false;
        using var proc = System.Diagnostics.Process.GetCurrentProcess();
        return NativeMethods.HasUiAccessProcess(proc.Handle);
    }
}
