// Portions adapted from VoicemeeterFancyOSD (MIT, A-tG and contributors).
// https://github.com/A-tG/VoicemeeterFancyOSD — see NOTICE.md.
using System.Runtime.InteropServices;
using System.Security;
using System.Text;
using System.Windows;

namespace Plith.Interop;

#region Enums

public enum GetWindowLongFields
{
    GWL_STYLE = -16,
    GWL_EXSTYLE = -20,
}

[Flags]
public enum WindowStyles : uint
{
    WS_OVERLAPPED = 0x00000000,
    WS_POPUP = 0x80000000,
    WS_CHILD = 0x40000000,
    WS_MINIMIZE = 0x20000000,
    WS_VISIBLE = 0x10000000,
    WS_DISABLED = 0x08000000,
    WS_CLIPSIBLINGS = 0x04000000,
    WS_CLIPCHILDREN = 0x02000000,
    WS_MAXIMIZE = 0x01000000,
    WS_BORDER = 0x00800000,
    WS_DLGFRAME = 0x00400000,
    WS_VSCROLL = 0x00200000,
    WS_HSCROLL = 0x00100000,
    WS_SYSMENU = 0x00080000,
    WS_THICKFRAME = 0x00040000,
    WS_GROUP = 0x00020000,
    WS_TABSTOP = 0x00010000,
}

[Flags]
public enum ExtendedWindowStyles : uint
{
    WS_EX_TOPMOST = 0x00000008,
    WS_EX_TRANSPARENT = 0x00000020,
    WS_EX_TOOLWINDOW = 0x00000080,
    WS_EX_LAYERED = 0x00080000,
    WS_EX_NOREDIRECTIONBITMAP = 0x00200000,
    WS_EX_NOACTIVATE = 0x08000000,
}

public static class SWP
{
    public const int
        NOSIZE = 0x0001,
        NOMOVE = 0x0002,
        NOZORDER = 0x0004,
        NOACTIVATE = 0x0010,
        SHOWWINDOW = 0x0040,
        NOOWNERZORDER = 0x0200;
}

public enum ShowWindowCommands
{
    Hide = 0,
    Show = 5,
    ShowNoActivate = 4,
}

public enum WindowMessage : uint
{
    WM_DESTROY = 0x0002,
    WM_MOVE = 0x0003,
    WM_ACTIVATE = 0x0006,
    WM_LBUTTONUP = 0x0202,
    WM_EXITSIZEMOVE = 0x0232,
    WM_DPICHANGED = 0x02E0,
    WM_SYSCOMMAND = 0x0112,
    WM_DWMCOMPOSITIONCHANGED = 0x031E,
}

#endregion

[SuppressUnmanagedCodeSecurity]
public static partial class NativeMethods
{
    public static readonly nint SC_MOUSEMOVE = 0xF012;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    internal struct WNDCLASSEX
    {
        public int cbSize;
        public int style;
        public nint lpfnWndProc;
        public int cbClsExtra;
        public int cbWndExtra;
        public nint hInstance;
        public nint hIcon;
        public nint hCursor;
        public nint hbrBackground;
        [MarshalAs(UnmanagedType.LPTStr)] public string lpszMenuName;
        [MarshalAs(UnmanagedType.LPTStr)] public string lpszClassName;
        public nint hIconSm;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT
    {
        public int Left, Top, Right, Bottom;
        public int Width => Right - Left;
        public int Height => Bottom - Top;
    }

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "CreateWindowExW", CharSet = CharSet.Unicode)]
    internal static extern nint CreateWindowEx(
        int dwExStyle, ushort regResult, [MarshalAs(UnmanagedType.LPWStr)] string lpWindowName,
        uint dwStyle, int x, int y, int nWidth, int nHeight,
        nint hWndParent, nint hMenu, nint hInstance, nint lpParam);

    // Undocumented Windows API exported from user32.dll on Windows 10+.
    // Creates a window with a specific Z-order band, enabling true topmost-over-fullscreen behavior.
    [DllImport("user32.dll", SetLastError = true, EntryPoint = "CreateWindowInBand", CharSet = CharSet.Unicode)]
    internal static extern nint CreateWindowInBand(
        int dwExStyle, ushort atomBomb, [MarshalAs(UnmanagedType.LPWStr)] string lpWindowName,
        uint dwStyle, int x, int y, int nWidth, int nHeight,
        nint hWndParent, nint hMenu, nint hInstance, nint lpParam, int dwBand);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "RegisterClassExW", CharSet = CharSet.Unicode)]
    internal static extern ushort RegisterClassEx([In] ref WNDCLASSEX lpWndClass);

    [DllImport("user32.dll")]
    internal static extern nint DefWindowProc(nint hWnd, uint uMsg, nint wParam, nint lParam);

    [DllImport("user32.dll")]
    internal static extern bool UpdateWindow(nint hWnd);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    private static extern nint GetWindowLongPtr64(nint hWnd, int nIndex);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongW")]
    private static extern nint GetWindowLong32(nint hWnd, int nIndex);

    public static nint GetWindowLongPtr(nint hWnd, int nIndex) =>
        nint.Size == 8 ? GetWindowLongPtr64(hWnd, nIndex) : GetWindowLong32(hWnd, nIndex);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
    private static extern nint SetWindowLongPtr64(nint hWnd, int nIndex, nint dwNewLong);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongW")]
    private static extern int SetWindowLong32(nint hWnd, int nIndex, int dwNewLong);

    internal static nint SetWindowLongPtr(nint hWnd, int nIndex, nint dwNewLong) =>
        nint.Size == 8 ? SetWindowLongPtr64(hWnd, nIndex, dwNewLong)
                       : new nint(SetWindowLong32(hWnd, nIndex, (int)dwNewLong));

    [DllImport("user32.dll")]
    internal static extern int SetForegroundWindow(nint hWnd);

    [DllImport("user32.dll")]
    internal static extern bool ShowWindow(nint hWnd, int nCmdShow);

    [DllImport("user32.dll", SetLastError = true)]
    internal static extern bool DestroyWindow(nint hWnd);

    [DllImport("user32.dll", SetLastError = true)]
    internal static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, int uFlags);

    [DllImport("user32.dll")]
    internal static extern bool GetWindowRect(nint hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    internal static extern int GetDpiForWindow(nint hWnd);

    [DllImport("user32.dll")]
    internal static extern int SendMessage(nint hWnd, WindowMessage msg, nint wParam, nint lParam);

    [DllImport("user32.dll")]
    internal static extern bool SetLayeredWindowAttributes(nint hwnd, uint crKey, byte bAlpha, uint dwFlags);

    public const uint LWA_ALPHA = 0x2;

    public static void ApplyWindowStyles(
        nint hWnd,
        WindowStyles wsToAdd = 0, WindowStyles wsToRemove = 0,
        ExtendedWindowStyles wsEXToAdd = 0, ExtendedWindowStyles wsEXToRemove = 0)
    {
        var style = (long)GetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_STYLE);
        var exstyle = (long)GetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_EXSTYLE);
        style |= (uint)wsToAdd; style &= ~(uint)wsToRemove;
        exstyle |= (uint)wsEXToAdd; exstyle &= ~(uint)wsEXToRemove;
        SetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_STYLE, new(style));
        SetWindowLongPtr(hWnd, (int)GetWindowLongFields.GWL_EXSTYLE, new(exstyle));
    }

    public static bool IsBandWindowSupported()
    {
        if (!NativeLibrary.TryLoad("user32.dll", out var libHandle))
            return false;
        try
        {
            return NativeLibrary.TryGetExport(libHandle, "CreateWindowInBand", out _);
        }
        finally
        {
            NativeLibrary.Free(libHandle);
        }
    }

    /// <summary>
    /// Picks the highest Z-band the current process is allowed to enter.
    /// Without UIAccess (Phase 4), <see cref="ZBandID.Desktop"/> is the realistic ceiling.
    /// </summary>
    public static ZBandID GetTopMostZBandID()
    {
        try
        {
            using var proc = System.Diagnostics.Process.GetCurrentProcess();
            var isImmersive = IsImmersiveProcess(proc.Handle);
            var hasUiAccess = HasUiAccessProcess(proc.Handle);
            var top = Environment.OSVersion.Version >= new Version(10, 0)
                ? ZBandID.AboveLockUX
                : ZBandID.SystemTools;
            return isImmersive ? top
                 : hasUiAccess ? ZBandID.UIAccess
                 : ZBandID.Desktop;
        }
        catch
        {
            return ZBandID.Desktop;
        }
    }

    [DllImport("user32.dll")]
    internal static extern bool IsImmersiveProcess(nint hProcess);

    [DllImport("advapi32.dll")]
    private static extern bool OpenProcessToken(nint hProcess, uint DesiredAccess, out nint hToken);

    [DllImport("advapi32.dll")]
    private static extern bool GetTokenInformation(nint TokenHandle, int TokenInformationClass,
        nint TokenInformation, uint TokenInformationLength, out uint ReturnLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(nint hObject);

    private const uint TOKEN_QUERY = 0x0008;
    private const int TokenUIAccess = 26;

    public static bool HasUiAccessProcess(nint hProcess)
    {
        if (!OpenProcessToken(hProcess, TOKEN_QUERY, out var token)) return false;
        try
        {
            var info = Marshal.AllocHGlobal(sizeof(uint));
            try
            {
                return GetTokenInformation(token, TokenUIAccess, info, sizeof(uint), out _)
                    && Marshal.ReadInt32(info) != 0;
            }
            finally
            {
                Marshal.FreeHGlobal(info);
            }
        }
        finally
        {
            CloseHandle(token);
        }
    }
}
