// Portions adapted from VoicemeeterFancyOSD (MIT, A-tG and contributors). See NOTICE.md.
namespace Plith.Interop;

internal interface IWndProcObject { }

public interface IWndProcHookHandler
{
    uint OnHwndCreated(nint hWnd, out bool register);
    nint OnWndProc(nint hWnd, uint msg, nint wParam, nint lParam);
}

public class WndProcHookManager
{
    private readonly Dictionary<uint, WndProc> _hooks = new();
    private readonly List<IWndProcHookHandler> _hookHandlers = new();

    private static readonly Dictionary<IWndProcObject, WndProcHookManager> _hookManagers = new();

    internal static WndProcHookManager RegisterForIWndProcObject(IWndProcObject wndProcObject)
    {
        ArgumentNullException.ThrowIfNull(wndProcObject);
        var manager = new WndProcHookManager();
        _hookManagers[wndProcObject] = manager;
        return manager;
    }

    public void RegisterCallbackForMessage(uint msg, WndProc callback)
    {
        ArgumentNullException.ThrowIfNull(callback);
        _hooks[msg] = callback;
    }

    internal void OnHwndCreated(nint hWnd)
    {
        foreach (var h in _hookHandlers)
        {
            var msg = h.OnHwndCreated(hWnd, out var register);
            if (register) _hooks[msg] = h.OnWndProc;
        }
    }

    internal nint TryHandleWindowMessage(nint hWnd, uint msg, nint wParam, nint lParam, out bool handled)
    {
        if (_hooks.TryGetValue(msg, out var hook))
        {
            handled = true;
            return hook(hWnd, msg, wParam, lParam);
        }
        handled = false;
        return 0;
    }
}
