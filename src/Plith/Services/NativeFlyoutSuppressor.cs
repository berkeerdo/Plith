using System.Runtime.InteropServices;
using System.Text;

namespace Plith.Services;

/// <summary>
/// Hides Windows' native volume flyout so Plith's OSD is the only thing the user sees on
/// a volume change. Combines four filters to avoid accidentally hiding Start menu /
/// taskbar / notification toasts:
///
///   1. Host class matches modern shell flyout class names.
///   2. Owning process is Explorer or ShellExperienceHost.
///   3. Z-band is ZBID_IMMERSIVE_NOTIFICATIONS (volume + brightness flyouts live here;
///      Start menu and taskbar are in different bands).
///   4. A volume event arrived within the last 400 ms (suppression window). This is the
///      key filter — without it we'd also hide brightness OSD and immersive toasts.
///
/// SetWinEventHook delivery is async (WINEVENT_OUTOFCONTEXT), so we don't block the
/// shell process even when checking class names, process IDs, and bands per event.
/// </summary>
public sealed class NativeFlyoutSuppressor : IDisposable
{
    private const uint EVENT_OBJECT_CREATE = 0x8000;
    private const uint EVENT_OBJECT_SHOW = 0x8002;
    private const uint EVENT_OBJECT_LOCATIONCHANGE = 0x800B;
    private const uint WINEVENT_OUTOFCONTEXT = 0x0000;
    private const uint WINEVENT_SKIPOWNPROCESS = 0x0002;
    private const int SW_HIDE = 0;

    // Historically the volume / brightness / network flyouts lived in
    // ZBID_IMMERSIVE_NOTIFICATIONS (0x4). Recent Win11 builds moved them to newer bands
    // (0x11 = system-tools-adjacent, 0x12 = shell-owned island widget). We accept the
    // whole set the shell has used, keyed off class+process still doing most of the work.
    private static readonly uint[] FlyoutBands = new uint[] { 0x4, 0x10, 0x11, 0x12 };

    // Suppression window after a Windows volume event. Long enough to catch the flyout
    // even on slow systems where show is delayed; short enough that a brightness key
    // press won't fall inside the window.
    private static readonly TimeSpan SuppressionWindowDuration = TimeSpan.FromMilliseconds(400);

    // Window classes the shell uses to host the modern volume / brightness flyout across
    // Win10 / Win11 builds. Newer builds added XamlExplorerHostIslandWindow and the
    // WinAppSDK-style DesktopChildSiteBridge; we keep all of them so the suppressor stays
    // effective if the shell shifts between hosts.
    private static readonly string[] HostClasses =
    {
        "NativeHWNDHost",
        "Xaml_WindowedPopupClass",
        "Microsoft.UI.Content.DesktopChildSiteBridge",
        "Windows.UI.Composition.DesktopWindowContentBridge",
        "XamlExplorerHostIslandWindow",
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

    private readonly DiagnosticLog? _log;

    private nint _showHook;
    private nint _createHook;
    private nint _locationHook;
    private WinEventDelegate? _delegate;   // keep a managed reference so the GC doesn't collect it
    private DateTime _suppressionWindowEndsUtc;
    private bool _getWindowBandResolved;
    private GetWindowBandDelegate? _getWindowBand;
    private bool _disposed;

    // PID → owning-process-name cache. EVENT_OBJECT_LOCATIONCHANGE fires hundreds-to-thousands
    // of times per second on a busy desktop. Resolving the process name via
    // Process.GetProcessById on each event opens a kernel handle and allocates a finalizable
    // managed Process per call — that's a GC + handle-table pressure cliff. Cache the lookup.
    private readonly Dictionary<uint, string> _pidNameCache = new();

    public NativeFlyoutSuppressor(DiagnosticLog? log = null)
    {
        _log = log;
    }

    /// <summary>Open the suppression window starting now. Called from App when a Windows
    /// audio volume event fires. Any matching shell flyout within
    /// <see cref="SuppressionWindowDuration"/> after this call gets hidden.</summary>
    public void OpenSuppressionWindow()
    {
        _suppressionWindowEndsUtc = DateTime.UtcNow + SuppressionWindowDuration;
        _log?.Info("FlyoutSuppressor", $"Suppression window opened +{SuppressionWindowDuration.TotalMilliseconds:F0}ms");
    }

    public void Start()
    {
        if (_showHook != 0) return;
        _delegate = OnWinEvent;
        ResolveGetWindowBand();

        // Three hooks because the volume flyout has used different patterns across Windows
        // builds: SHOW for newly-created flyout windows (old Win10), LOCATIONCHANGE for
        // long-lived pre-created windows that simply move on-screen on a key press, and
        // CREATE for Win11 builds that re-create the bridge window per show.
        _showHook = SetWinEventHook(
            EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW,
            0, _delegate, 0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

        _createHook = SetWinEventHook(
            EVENT_OBJECT_CREATE, EVENT_OBJECT_CREATE,
            0, _delegate, 0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

        _locationHook = SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE,
            0, _delegate, 0, 0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);

        _log?.Info("FlyoutSuppressor", $"Started. GetWindowBand available: {_getWindowBand is not null}");
    }

    public void Stop()
    {
        if (_showHook != 0) { UnhookWinEvent(_showHook); _showHook = 0; }
        if (_createHook != 0) { UnhookWinEvent(_createHook); _createHook = 0; }
        if (_locationHook != 0) { UnhookWinEvent(_locationHook); _locationHook = 0; }
        _delegate = null;
        _pidNameCache.Clear();
    }

    private void OnWinEvent(nint hWinEventHook, uint eventType, nint hwnd,
                            int idObject, int idChild,
                            uint dwEventThread, uint dwmsEventTime)
    {
        if (hwnd == 0) return;
        if (idObject != 0 /* OBJID_WINDOW */ || idChild != 0) return;

        bool inWindow = DateTime.UtcNow <= _suppressionWindowEndsUtc;

        // Cheapest checks first, skip disk I/O and P/Invoke on the vast majority of shell
        // events. LOCATIONCHANGE fires thousands of times per second on a busy desktop
        // (Task Manager row updates, Explorer popups, teams, VS Code); logging or resolving
        // process names for all of them starves the UI dispatcher and appears as a hang.
        if (!inWindow) return;

        var className = GetClassNameSafe(hwnd);
        if (!IsHostClassName(className)) return;

        var procName = GetProcessNameSafe(hwnd);
        if (!IsShellProcessName(procName)) return;

        var band = GetBandSafe(hwnd);
        if (!IsImmersiveNotificationsBand(band)) return;

        // Log only the events that actually pass ALL filters — real suppression targets.
        _log?.Info("FlyoutSuppressor",
            $"Hiding flyout hwnd=0x{hwnd:X} ev=0x{eventType:X} class='{className}' proc='{procName}' band=0x{band:X}");
        ShowWindow(hwnd, SW_HIDE);
    }

    private static string GetClassNameSafe(nint hwnd)
    {
        var sb = new StringBuilder(64);
        return GetClassName(hwnd, sb, sb.Capacity) > 0 ? sb.ToString() : string.Empty;
    }

    private string GetProcessNameSafe(nint hwnd)
    {
        _ = GetWindowThreadProcessId(hwnd, out uint pid);
        if (pid == 0) return string.Empty;
        if (_pidNameCache.TryGetValue(pid, out var name)) return name;
        try
        {
            using var proc = System.Diagnostics.Process.GetProcessById((int)pid);
            name = proc.ProcessName;
        }
        catch
        {
            name = string.Empty;
        }
        _pidNameCache[pid] = name;
        return name;
    }

    private uint GetBandSafe(nint hwnd)
    {
        if (_getWindowBand is null) return 0xFFFFFFFF;   // unknown
        try { return _getWindowBand(hwnd, out uint band) ? band : 0xFFFFFFFF; }
        catch { return 0xFFFFFFFF; }
    }

    private static bool IsHostClassName(string name)
    {
        for (int i = 0; i < HostClasses.Length; i++)
            if (string.Equals(name, HostClasses[i], StringComparison.Ordinal)) return true;
        return false;
    }

    // When GetWindowBand isn't available, GetBandSafe returns 0xFFFFFFFF (unknown) — we
    // treat unknown as "allow" so older Windows where the export doesn't exist still benefit
    // from class+process filtering. Treating unknown as "match" is safer than "reject" here.
    private bool IsImmersiveNotificationsBand(uint band)
    {
        if (_getWindowBand is null || band == 0xFFFFFFFF) return true;
        for (int i = 0; i < FlyoutBands.Length; i++) if (FlyoutBands[i] == band) return true;
        return false;
    }

    private static bool IsShellProcessName(string name)
    {
        for (int i = 0; i < OwningProcessNames.Length; i++)
            if (string.Equals(name, OwningProcessNames[i], StringComparison.OrdinalIgnoreCase))
                return true;
        return false;
    }

    private void ResolveGetWindowBand()
    {
        if (_getWindowBandResolved) return;
        _getWindowBandResolved = true;
        try
        {
            if (NativeLibrary.TryLoad("user32.dll", out var handle)
                && NativeLibrary.TryGetExport(handle, "GetWindowBand", out var addr))
            {
                _getWindowBand = Marshal.GetDelegateForFunctionPointer<GetWindowBandDelegate>(addr);
            }
        }
        catch
        {
            _getWindowBand = null;
        }
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

    // GetWindowBand is an undocumented user32 export. Dynamic GetProcAddress lookup so we
    // gracefully degrade on older Windows where it isn't present.
    [UnmanagedFunctionPointer(CallingConvention.StdCall)]
    private delegate bool GetWindowBandDelegate(nint hwnd, out uint pdwBand);

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
