using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows.Threading;
using Plith.Views;

namespace Plith.Services;

/// <summary>
/// Listens for the system foreground window changing and re-asserts the OSD's
/// HWND_TOPMOST so a game / video player popping a new topmost window mid-OSD-fade
/// can't sit above us. Per-show ReassertTopmost in <see cref="OsdWindow.ShowOsd"/>
/// covers the initial promotion; this hook covers the in-flight foreground swap.
///
/// The hook uses WINEVENT_OUTOFCONTEXT so callbacks fire on the thread that set the
/// hook (we set from the UI dispatcher) — no marshalling needed, but we still check
/// dispatcher access defensively in case a future caller subscribes from elsewhere.
/// </summary>
public sealed class ForegroundWatcher : IDisposable
{
    private const uint EVENT_SYSTEM_FOREGROUND = 0x0003;
    private const uint WINEVENT_OUTOFCONTEXT = 0x0000;

    // Coalesce bursts of foreground events (e.g. rapid alt-tab) to one re-assert per
    // throttle window so we don't hammer SetWindowPos on switcher animations.
    private static readonly TimeSpan ThrottleWindow = TimeSpan.FromMilliseconds(150);

    private readonly OsdHost _osd;
    private readonly Dispatcher _dispatcher;
    private nint _hook;
    // Held in a field so the GC doesn't collect the delegate while the native hook is live.
    private WinEventDelegate? _callback;
    private DateTime _lastReassertUtc = DateTime.MinValue;
    private bool _disposed;

    public ForegroundWatcher(OsdHost osd)
    {
        _osd = osd;
        _dispatcher = osd.Dispatcher;
    }

    public void Start()
    {
        if (_hook != 0) return;
        _callback = OnForegroundChanged;
        _hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            hmodWinEventProc: 0,
            _callback,
            idProcess: 0,
            idThread: 0,
            WINEVENT_OUTOFCONTEXT);
        if (_hook == 0)
        {
            // Hook registration can fail under elevated UAC contexts or sandboxes. Log once
            // and continue without foreground tracking — the per-show ReassertTopmost still
            // covers the common case of a game raising itself before our first ShowOsd.
            Trace.WriteLine("Plith: SetWinEventHook failed; foreground topmost tracking is off.");
        }
    }

    private void OnForegroundChanged(
        nint hWinEventHook, uint eventType, nint hwnd,
        int idObject, int idChild, uint idEventThread, uint dwmsEventTime)
    {
        if (_disposed) return;
        if (!_dispatcher.CheckAccess())
        {
            _dispatcher.BeginInvoke(new Action(MaybeReassert));
            return;
        }
        MaybeReassert();
    }

    private void MaybeReassert()
    {
        if (_disposed) return;
        // Skip when the OSD isn't on screen — no point promoting an invisible window. We
        // also avoid re-asserting during the fade-out tail so a foreground event arriving
        // mid-fade doesn't pull a half-faded card back up.
        if (_osd.Opacity < 0.01) return;
        var now = DateTime.UtcNow;
        if (now - _lastReassertUtc < ThrottleWindow) return;
        _lastReassertUtc = now;
        _osd.ReassertTopmost();
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
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

    [DllImport("user32.dll", SetLastError = true)]
    private static extern nint SetWinEventHook(
        uint eventMin, uint eventMax,
        nint hmodWinEventProc, WinEventDelegate lpfnWinEventProc,
        uint idProcess, uint idThread, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnhookWinEvent(nint hWinEventHook);
}
