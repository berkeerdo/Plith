using System.Runtime.InteropServices;
using System.Windows.Input;
using System.Windows.Interop;

namespace Plith.Services;

/// <summary>
/// Registers a system-wide hotkey via RegisterHotKey and re-raises it as the
/// <see cref="Pressed"/> event. Uses a hidden message-only window so the hotkey
/// receiver is independent of any UI window's lifecycle. The combo is
/// reconfigurable at runtime — <see cref="Apply"/> unregisters the old binding
/// and applies the new one in one step.
/// </summary>
public sealed class HotkeyService : IDisposable
{
    private const int WM_HOTKEY = 0x0312;
    private const int HotkeyId = 1;

    [Flags]
    private enum HotkeyModifiers : uint
    {
        Alt = 0x0001,
        Control = 0x0002,
        Shift = 0x0004,
        Win = 0x0008,
        NoRepeat = 0x4000,
    }

    private HwndSource? _source;
    private bool _isRegistered;
    private HotkeyCombo _activeCombo;
    private bool _disposed;

    public event Action? Pressed;

    /// <summary>Currently-bound combo, or <see cref="HotkeyCombo.None"/> if nothing is registered.</summary>
    public HotkeyCombo ActiveCombo => _isRegistered ? _activeCombo : HotkeyCombo.None;

    /// <summary>
    /// Swaps the active hotkey to <paramref name="combo"/>. Returns false when Windows refuses
    /// the registration (another process owns it) — the previous binding is also cleared in
    /// that case, so the caller gets a clean "nothing is bound" state.
    /// </summary>
    public bool Apply(HotkeyCombo combo)
    {
        if (_disposed) return false;

        try { EnsureMessageWindow(); }
        catch { return false; }   // HwndSource ctor can throw if WPF dispatcher is in a bad state.
        if (_source is null) return false;

        if (_isRegistered)
        {
            UnregisterHotKey(_source.Handle, HotkeyId);
            _isRegistered = false;
        }

        if (combo == HotkeyCombo.None) { _activeCombo = HotkeyCombo.None; return true; }

        var (mods, vk) = MapCombo(combo);
        if (RegisterHotKey(_source.Handle, HotkeyId, (uint)(mods | HotkeyModifiers.NoRepeat), (uint)vk))
        {
            _isRegistered = true;
            _activeCombo = combo;
            return true;
        }

        _activeCombo = HotkeyCombo.None;
        return false;
    }

    private static (HotkeyModifiers mods, int vk) MapCombo(HotkeyCombo combo) => combo switch
    {
        HotkeyCombo.CtrlAltV   => (HotkeyModifiers.Control | HotkeyModifiers.Alt,   KeyInterop.VirtualKeyFromKey(Key.V)),
        HotkeyCombo.CtrlShiftV => (HotkeyModifiers.Control | HotkeyModifiers.Shift, KeyInterop.VirtualKeyFromKey(Key.V)),
        HotkeyCombo.AltShiftV  => (HotkeyModifiers.Alt     | HotkeyModifiers.Shift, KeyInterop.VirtualKeyFromKey(Key.V)),
        HotkeyCombo.CtrlAltM   => (HotkeyModifiers.Control | HotkeyModifiers.Alt,   KeyInterop.VirtualKeyFromKey(Key.M)),
        _                      => (0, 0),
    };

    private void EnsureMessageWindow()
    {
        if (_source is not null) return;
        var parameters = new HwndSourceParameters("PlithHotkey")
        {
            ParentWindow = (nint)(-3),   // HWND_MESSAGE
            UsesPerPixelOpacity = false,
            HwndSourceHook = HwndHook,
        };
        _source = new HwndSource(parameters);
    }

    private nint HwndHook(nint hwnd, int msg, nint wParam, nint lParam, ref bool handled)
    {
        if (msg == WM_HOTKEY && wParam.ToInt32() == HotkeyId)
        {
            Pressed?.Invoke();
            handled = true;
        }
        return 0;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_source is not null)
        {
            if (_isRegistered)
            {
                try { UnregisterHotKey(_source.Handle, HotkeyId); } catch { }
                _isRegistered = false;
            }
            _source.Dispose();
            _source = null;
        }
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool RegisterHotKey(nint hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnregisterHotKey(nint hWnd, int id);
}
