using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Input;
using System.Windows.Interop;

namespace Plith.Services;

/// <summary>
/// Registers a system-wide hotkey via RegisterHotKey and re-raises it as the
/// <see cref="Pressed"/> event. Uses a hidden message-only window so the hotkey
/// receiver is independent of any UI window's lifecycle. The combo is fully
/// configurable at runtime — <see cref="Apply"/> unregisters the previous
/// binding and applies the new (mods, vk) in one atomic step.
/// </summary>
public sealed class HotkeyService : IDisposable
{
    private const int WM_HOTKEY = 0x0312;
    private const int HotkeyId = 1;

    [Flags]
    public enum HotkeyMods : uint
    {
        None = 0,
        Alt = 0x0001,
        Control = 0x0002,
        Shift = 0x0004,
        Win = 0x0008,
        NoRepeat = 0x4000,
    }

    private HwndSource? _source;
    private bool _isRegistered;
    private uint _activeMods;
    private int _activeKey;
    private bool _disposed;

    public event Action? Pressed;

    public uint ActiveMods => _isRegistered ? _activeMods : 0;
    public int ActiveKey => _isRegistered ? _activeKey : 0;

    /// <summary>
    /// Swap the active hotkey to (<paramref name="mods"/>, <paramref name="vk"/>).
    /// Pass both as 0 to unbind. Returns false when Windows refuses the new binding
    /// (another process already owns it); the previous binding is also cleared in that
    /// case so the caller gets a clean 'nothing bound' state.
    /// </summary>
    public bool Apply(uint mods, int vk)
    {
        if (_disposed) return false;

        try { EnsureMessageWindow(); }
        catch { return false; }
        if (_source is null) return false;

        if (_isRegistered)
        {
            _ = UnregisterHotKey(_source.Handle, HotkeyId);
            _isRegistered = false;
        }

        if (mods == 0 || vk == 0)
        {
            _activeMods = 0;
            _activeKey = 0;
            return true;
        }

        if (RegisterHotKey(_source.Handle, HotkeyId, mods | (uint)HotkeyMods.NoRepeat, (uint)vk))
        {
            _isRegistered = true;
            _activeMods = mods;
            _activeKey = vk;
            return true;
        }

        _activeMods = 0;
        _activeKey = 0;
        return false;
    }

    /// <summary>Format a (mods, vk) pair as a user-facing string, e.g. 'Ctrl+Alt+V'.
    /// Returns an empty string when nothing is bound.</summary>
    public static string FormatCombo(uint mods, int vk)
    {
        if (mods == 0 || vk == 0) return "";
        var sb = new StringBuilder();
        if ((mods & (uint)HotkeyMods.Control) != 0) sb.Append("Ctrl+");
        if ((mods & (uint)HotkeyMods.Alt) != 0) sb.Append("Alt+");
        if ((mods & (uint)HotkeyMods.Shift) != 0) sb.Append("Shift+");
        if ((mods & (uint)HotkeyMods.Win) != 0) sb.Append("Win+");
        try { sb.Append(KeyInterop.KeyFromVirtualKey(vk).ToString()); }
        catch { sb.Append("Key(" + vk.ToString(CultureInfo.InvariantCulture) + ")"); }
        return sb.ToString();
    }

    /// <summary>Migrate a legacy [Osd]SummonHotkey enum string from older config.ini
    /// files into (mods, vk). Returns (0, 0) when the value is missing or unknown.</summary>
    public static (uint mods, int vk) MigrateLegacy(string? legacy) => legacy switch
    {
        "CtrlAltV"   => ((uint)(HotkeyMods.Control | HotkeyMods.Alt),   KeyInterop.VirtualKeyFromKey(Key.V)),
        "CtrlShiftV" => ((uint)(HotkeyMods.Control | HotkeyMods.Shift), KeyInterop.VirtualKeyFromKey(Key.V)),
        "AltShiftV"  => ((uint)(HotkeyMods.Alt     | HotkeyMods.Shift), KeyInterop.VirtualKeyFromKey(Key.V)),
        "CtrlAltM"   => ((uint)(HotkeyMods.Control | HotkeyMods.Alt),   KeyInterop.VirtualKeyFromKey(Key.M)),
        _            => (0, 0),
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
                try { _ = UnregisterHotKey(_source.Handle, HotkeyId); } catch { }
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
