using System.Runtime.InteropServices;
using System.Text;

namespace Plith.Services;

/// <summary>
/// Hides Windows' native volume / brightness flyout so Plith's OSD is the only thing the user
/// sees on a volume change. Approach is the one ModernFlyouts uses: hook EVENT_OBJECT_SHOW,
/// classify the window by class name + owning process, and SW_HIDE it on the spot.
///
/// SetWinEventHook delivery is async (WINEVENT_OUTOFCONTEXT), so we don't block the
/// shell process even when checking class names and process IDs per event.
/// </summary>
public sealed class NativeFlyoutSuppressor : IDisposable
{
    private const uint EVENT_OBJECT_SHOW = 0x8002;
    private const uint EVENT_OBJECT_LOCATIONCHANGE = 0x800B;
    private const uint WINEVENT_OUTOFCONTEXT = 0x0000;
    private const uint WINEVENT_SKIPOWNPROCESS = 0x0002;
    private const int SW_HIDE = 0;

    // Window classes the shell uses to host the modern volume / brightness flyout across
    // Win10 / Win11 builds. Newer builds added Xaml_WindowedPopupClass and the WinAppSDK-style
    // DesktopChildSiteBridge; we keep all of them so the suppressor stays effective if the
    // shell shifts between hosts.
    private static readonly string[] HostClasses =
    {
        "NativeHWNDHost",
        "Xaml_WindowedPopupClass",
        "Microsoft.UI.Content.DesktopChildSiteBridge",
        "Windows.UI.Composition.DesktopWindowContentBridge",
    };

    // Owning process name (no extension). On Win10/11 the volume flyout is hosted by
    // ShellExperienceHost; on some builds it can also surface under Explorer or the
    // newer Windows Input Experience host.
    private static readonly string[] OwningProcessNames =
    {
        "ShellExperienceHost",
        "Explorer",
        "WindowsInputExperience",
    };

    private nint _showHook;
    private nint _locationHook;
    private WinEventDelegate? _delegate;   // keep a managed reference so the GC doesn't collect it
    private bool _disposed;

    // PID → owning-process-name cache. EVENT_OBJECT_LOCATIONCHANGE fires hundreds-to-thousands
    // of times per second on a busy desktop (every window resize, every DWM frame, every tooltip
    // move). Resolving the process name via Process.GetProcessById on each event opens a kernel
    // handle and allocates a finalizable managed Process per call — that's a GC + handle-table
    // pressure cliff. Cache the lookup; a stale entry only causes a brief miss until the process
    // exits and a different PID arrives, which the catch path below picks up.
    private readonly Dictionary<uint, string> _pidNameCache = new();

    public void Start()
    {
        if (_showHook != 0) return;
        _delegate = OnWinEvent;

        // EVENT_OBJECT_SHOW catches freshly-created flyout windows. EVENT_OBJECT_LOCATIONCHANGE
        // catches the case where the flyout is a long-lived pre-created window that simply moves
        // on-screen when the user presses a volume key — show wouldn't fire there.
        _showHook = SetWinEventHook(
            EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW,
            0, _delegate, 0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

        _locationHook = SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE,
            0, _delegate, 0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
    }

    public void Stop()
    {
        if (_showHook != 0) { UnhookWinEvent(_showHook); _showHook = 0; }
        if (_locationHook != 0) { UnhookWinEvent(_locationHook); _locationHook = 0; }
        _delegate = null;
        _pidNameCache.Clear();
    }

    private void OnWinEvent(nint hWinEventHook, uint eventType, nint hwnd,
                            int idObject, int idChild,
                            uint dwEventThread, uint dwmsEventTime)
    {
        if (hwnd == 0) return;
        // Only top-level window shows are relevant; child object shows (OBJID values) are noise.
        if (idObject != 0 /* OBJID_WINDOW */ || idChild != 0) return;

        if (!IsHostClass(hwnd)) return;
        if (!IsOwnedByShellProcess(hwnd)) return;

        ShowWindow(hwnd, SW_HIDE);
    }

    private static bool IsHostClass(nint hwnd)
    {
        var sb = new StringBuilder(64);
        if (GetClassName(hwnd, sb, sb.Capacity) == 0) return false;
        var name = sb.ToString();
        for (int i = 0; i < HostClasses.Length; i++)
            if (string.Equals(name, HostClasses[i], StringComparison.Ordinal)) return true;
        return false;
    }

    private bool IsOwnedByShellProcess(nint hwnd)
    {
        _ = GetWindowThreadProcessId(hwnd, out uint pid);
        if (pid == 0) return false;

        if (!_pidNameCache.TryGetValue(pid, out var name))
        {
            try
            {
                using var proc = System.Diagnostics.Process.GetProcessById((int)pid);
                name = proc.ProcessName;
            }
            catch
            {
                // Process exited between GetWindowThreadProcessId and GetProcessById. Cache
                // the miss too — same PID may still be valid for a bit before reuse, and an
                // empty entry costs us a string compare instead of another OpenProcess call.
                name = string.Empty;
            }
            _pidNameCache[pid] = name;
        }

        for (int i = 0; i < OwningProcessNames.Length; i++)
            if (string.Equals(name, OwningProcessNames[i], StringComparison.OrdinalIgnoreCase))
                return true;
        return false;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Stop();
    }

    #region P/Invoke

    private delegate void WinEventDelegate(nint hWinEventHook, uint eventType, nint hwnd,
                                           int idObject, int idChild,
                                           uint dwEventThread, uint dwmsEventTime);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern nint SetWinEventHook(uint eventMin, uint eventMax,
        nint hmodWinEventProc, WinEventDelegate lpfnWinEventProc,
        uint idProcess, uint idThread, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnhookWinEvent(nint hWinEventHook);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int GetClassName(nint hWnd, StringBuilder lpClassName, int nMaxCount);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(nint hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(nint hWnd, int nCmdShow);

    #endregion
}
