using System.Runtime.InteropServices;

namespace Plith.Services;

/// <summary>
/// Low-level keyboard hook (WH_KEYBOARD_LL) that fires the instant a volume key
/// (VK_VOLUME_UP / VK_VOLUME_DOWN / VK_VOLUME_MUTE) is pressed, before Windows can
/// render its native volume flyout. NativeFlyoutSuppressor's suppression window is
/// opened from this signal so it's already open by the time the flyout appears —
/// closing the race the audio-notification-driven trigger loses on Win11.
///
/// The hook runs in the message loop of whichever thread called Start; App's UI
/// dispatcher is the intended caller. Delegate is pinned to a field so the GC does
/// not free it while Windows holds a callback pointer to it.
/// </summary>
public sealed class VolumeKeyHook : IDisposable
{
    private const int WH_KEYBOARD_LL = 13;
    private const int WM_KEYDOWN = 0x0100;
    private const int WM_SYSKEYDOWN = 0x0104;

    private const int VK_VOLUME_MUTE = 0xAD;
    private const int VK_VOLUME_DOWN = 0xAE;
    private const int VK_VOLUME_UP   = 0xAF;

    private readonly DiagnosticLog? _log;
    private LowLevelKeyboardProc? _proc;
    private nint _hookId;
    private bool _disposed;

    public event Action? VolumeKeyPressed;

    public VolumeKeyHook(DiagnosticLog? log = null) { _log = log; }

    public void Start()
    {
        if (_hookId != 0) return;
        _proc = HookCallback;
        // hMod = handle to any loaded module (the current EXE works). dwThreadId = 0
        // for a system-wide hook. SetWindowsHookEx returns 0 on failure.
        using var proc = System.Diagnostics.Process.GetCurrentProcess();
        _hookId = SetWindowsHookEx(WH_KEYBOARD_LL, _proc, GetModuleHandle(proc.MainModule!.ModuleName), 0);
        _log?.Info("VolumeKeyHook", $"Started (hookId=0x{_hookId:X})");
    }

    public void Stop()
    {
        if (_hookId != 0) { UnhookWindowsHookEx(_hookId); _hookId = 0; }
        _proc = null;
    }

    private nint HookCallback(int nCode, nint wParam, nint lParam)
    {
        // nCode < 0 means "pass through untouched, do not process". Only inspect KEYDOWN /
        // SYSKEYDOWN — key-up events aren't what triggers the flyout.
        if (nCode >= 0 && (wParam == WM_KEYDOWN || wParam == WM_SYSKEYDOWN))
        {
            int vk = Marshal.ReadInt32(lParam); // KBDLLHOOKSTRUCT.vkCode is the first field
            if (vk == VK_VOLUME_UP || vk == VK_VOLUME_DOWN || vk == VK_VOLUME_MUTE)
            {
                try { VolumeKeyPressed?.Invoke(); }
                catch (Exception ex) { _log?.Warn("VolumeKeyHook", $"handler threw: {ex.Message}"); }
            }
        }
        // Always pass the key through so the OS still processes the volume change.
        return CallNextHookEx(_hookId, nCode, wParam, lParam);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Stop();
    }

    #region P/Invoke

    private delegate nint LowLevelKeyboardProc(int nCode, nint wParam, nint lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern nint SetWindowsHookEx(int idHook, LowLevelKeyboardProc lpfn, nint hMod, uint dwThreadId);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnhookWindowsHookEx(nint hhk);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern nint CallNextHookEx(nint hhk, int nCode, nint wParam, nint lParam);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern nint GetModuleHandle(string lpModuleName);

    #endregion
}
