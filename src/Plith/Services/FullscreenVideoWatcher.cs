using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows.Threading;
using Plith.Cards;

namespace Plith.Services;

/// <summary>
/// Watches for a fullscreen window that is playing video and reports it as a suppression
/// state to <see cref="CardHost"/>.
///
/// Two triggers, because neither alone is sufficient: the foreground WinEvent hook catches
/// alt-tabbing into a player, and the 1 s poll catches entering fullscreen with F11 — which
/// changes no foreground window. Each evaluation is one GetForegroundWindow, one
/// GetWindowRect, one SHQueryUserNotificationState and a string compare, so 1 Hz is cheap.
/// </summary>
public sealed class FullscreenVideoWatcher : IShowSuppressor, IDisposable
{
    private const uint EVENT_SYSTEM_FOREGROUND = 0x0003;
    private const uint WINEVENT_OUTOFCONTEXT = 0x0000;
    private static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(1);

    private readonly SettingsService _settings;
    private readonly MediaSessionClient _media;
    private readonly DiagnosticLog? _log;
    private readonly DispatcherTimer _pollTimer;
    private nint _hook;
    private WinEventDelegate? _callback;
    private bool _suppressed;
    private bool _disposed;

    public FullscreenVideoWatcher(SettingsService settings, MediaSessionClient media, Dispatcher dispatcher, DiagnosticLog? log = null)
    {
        _settings = settings;
        _media = media;
        _log = log;
        _pollTimer = new DispatcherTimer(DispatcherPriority.Background, dispatcher) { Interval = PollInterval };
        _pollTimer.Tick += (_, _) => Evaluate();
    }

    public bool IsSuppressed => _suppressed;

    public event Action<bool>? SuppressionChanged;

    public void Start()
    {
        _callback = OnForegroundChanged;
        _hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
            hmodWinEventProc: 0, _callback,
            idProcess: 0, idThread: 0, WINEVENT_OUTOFCONTEXT);
        if (_hook == 0)
            _log?.Warn("FullscreenVideo", "SetWinEventHook failed; falling back to poll only.");

        _pollTimer.Start();
        Evaluate();
    }

    private void OnForegroundChanged(
        nint hWinEventHook, uint eventType, nint hwnd,
        int idObject, int idChild, uint idEventThread, uint dwmsEventTime) => Evaluate();

    private void Evaluate()
    {
        if (_disposed) return;

        bool next;
        try
        {
            next = FullscreenVideoDetector.ShouldSuppress(
                enabled: _settings.Current.HideDuringFullscreenVideo,
                foregroundCoversMonitor: ForegroundCoversMonitor(out var processName),
                notificationState: QueryNotificationState(),
                foregroundOwnsPlayingSmtc: ForegroundOwnsPlayingSmtc(processName),
                foregroundProcessName: processName,
                hideList: FullscreenVideoDetector.ParseHideList(_settings.Current.FullscreenVideoHideList));
        }
        catch (Exception ex)
        {
            // A window can die between GetForegroundWindow and Process lookup. Never let a
            // transient interop failure suppress the OSD — fail toward showing it.
            _log?.Warn("FullscreenVideo", $"Evaluate threw: {ex.GetType().Name}: {ex.Message}");
            next = false;
        }

        if (next == _suppressed) return;
        _suppressed = next;
        _log?.Info("FullscreenVideo", $"Suppression -> {next}");
        SuppressionChanged?.Invoke(next);
    }

    private static bool ForegroundCoversMonitor(out string processName)
    {
        processName = string.Empty;

        var hwnd = GetForegroundWindow();
        if (hwnd == 0) return false;

        // The desktop itself is technically fullscreen; never treat it as a video window.
        var cls = new char[64];
        int len = GetClassName(hwnd, cls, cls.Length);
        var className = len > 0 ? new string(cls, 0, len) : string.Empty;
        if (className is "Progman" or "WorkerW") return false;

        _ = GetWindowThreadProcessId(hwnd, out uint pid);
        if (pid != 0)
        {
            try { using var p = Process.GetProcessById((int)pid); processName = p.ProcessName; }
            catch { /* process exited between the two calls */ }
        }

        if (!GetWindowRect(hwnd, out var rect)) return false;

        // Monitor bounds come from GetMonitorInfo, NOT from WpfScreenHelper's Screen.Bounds.
        // GetWindowRect reports physical pixels while Screen.Bounds reports device-independent
        // units, so mixing them silently breaks every comparison on a non-100% DPI display —
        // a 3840-wide fullscreen window would measure 3072 against a 3840 monitor at 125% and
        // never register as fullscreen. Staying inside Win32 keeps both sides in one space.
        var monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if (monitor == 0) return false;

        var info = new MONITORINFO { cbSize = Marshal.SizeOf<MONITORINFO>() };
        if (!GetMonitorInfo(monitor, ref info)) return false;

        var b = info.rcMonitor;
        // A few pixels of tolerance: some players sit one pixel proud of the monitor edge.
        const int Tolerance = 2;
        return rect.Left <= b.Left + Tolerance
            && rect.Top <= b.Top + Tolerance
            && rect.Right >= b.Right - Tolerance
            && rect.Bottom >= b.Bottom - Tolerance;
    }

    // AUMID -> process matching is a heuristic: for Win32 apps the AUMID is in practice the
    // executable name, but for packaged apps it is a package family name that will not match
    // a process name at all. Every miss fails toward "do not hide", and the user's hide list
    // is the override. See the spec's "AUMID -> process matching is a heuristic" section.
    private bool ForegroundOwnsPlayingSmtc(string processName)
    {
        if (!_media.IsCurrentSessionPlaying) return false;
        if (string.IsNullOrWhiteSpace(processName)) return false;

        var aumid = _media.CurrentSourceAppUserModelId;
        if (string.IsNullOrWhiteSpace(aumid)) return false;

        return aumid.Contains(processName, StringComparison.OrdinalIgnoreCase)
            || processName.Contains(aumid, StringComparison.OrdinalIgnoreCase);
    }

    private static uint QueryNotificationState()
        => SHQueryUserNotificationState(out var state) == 0 ? state : 0;

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _pollTimer.Stop();
        if (_hook != 0)
        {
            try { _ = UnhookWinEvent(_hook); } catch { }
            _hook = 0;
        }
        _callback = null;
    }

    private delegate void WinEventDelegate(
        nint hWinEventHook, uint eventType, nint hwnd,
        int idObject, int idChild, uint idEventThread, uint dwmsEventTime);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    private struct MONITORINFO
    {
        public int cbSize;
        public RECT rcMonitor;
        public RECT rcWork;
        public uint dwFlags;
    }

    private const uint MONITOR_DEFAULTTONEAREST = 2;

    [DllImport("user32.dll")]
    private static extern nint MonitorFromWindow(nint hwnd, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetMonitorInfo(nint hMonitor, ref MONITORINFO lpmi);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern nint SetWinEventHook(
        uint eventMin, uint eventMax, nint hmodWinEventProc,
        WinEventDelegate lpfnWinEventProc, uint idProcess, uint idThread, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnhookWinEvent(nint hWinEventHook);

    [DllImport("user32.dll")]
    private static extern nint GetForegroundWindow();

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool GetWindowRect(nint hWnd, out RECT lpRect);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern int GetClassName(nint hWnd, char[] lpClassName, int nMaxCount);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint GetWindowThreadProcessId(nint hWnd, out uint lpdwProcessId);

    [DllImport("shell32.dll")]
    private static extern int SHQueryUserNotificationState(out uint pquns);
}
