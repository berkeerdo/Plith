using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;

namespace Plith.Interop;

/// <summary>
/// Applies Windows 11 Mica / Acrylic backdrop to a WPF window via DwmSetWindowAttribute.
/// Requires Windows 11 build 22000 or newer; silently no-ops on older systems.
/// </summary>
public static class Mica
{
    public enum BackdropType
    {
        Auto = 0,
        None = 1,
        Mica = 2,
        Acrylic = 3,
        MicaAlt = 4,
    }

    private const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;
    private const int DWMWA_SYSTEMBACKDROP_TYPE = 38;

    [DllImport("dwmapi.dll", PreserveSig = true)]
    private static extern int DwmSetWindowAttribute(nint hwnd, int attr, ref int attrValue, int attrSize);

    public static bool TryApplyToHandle(nint hWnd, BackdropType type, bool useDarkMode)
    {
        if (hWnd == 0) return false;
        if (!OperatingSystem.IsWindowsVersionAtLeast(10, 0, 22000)) return false;

        int dark = useDarkMode ? 1 : 0;
        _ = DwmSetWindowAttribute(hWnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref dark, sizeof(int));

        int backdrop = (int)type;
        int hr = DwmSetWindowAttribute(hWnd, DWMWA_SYSTEMBACKDROP_TYPE, ref backdrop, sizeof(int));
        return hr == 0;
    }

    /// <summary>For WPF Mica: window background must be transparent so DWM can paint behind it.</summary>
    public static void PrepareWindowBackground(Window window)
    {
        window.Background = Brushes.Transparent;
        window.AllowsTransparency = false;
        window.WindowStyle = WindowStyle.None;
    }
}
