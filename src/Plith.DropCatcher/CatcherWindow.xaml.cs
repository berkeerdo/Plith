using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace Plith.DropCatcher;

/// <summary>
/// The window that stands in for the notch while a drag is in flight.
///
/// It is an ordinary Medium-integrity window, and that is its entire qualification: Plith's own
/// window has a registered drop target and still receives neither a DragEnter nor a WM_DROPFILES,
/// because UIAccess puts Plith at High integrity and UIPI refuses the cross-integrity COM call
/// Explorer makes to start the drag.
/// </summary>
public partial class CatcherWindow : Window
{
    private const int GWL_EXSTYLE = -20;
    private const int WS_EX_TOOLWINDOW = 0x00000080;
    private const int WS_EX_NOACTIVATE = 0x08000000;

    private static readonly nint HWND_TOPMOST = -1;
    private const uint SWP_NOACTIVATE = 0x0010;
    private const uint SWP_SHOWWINDOW = 0x0040;
    private const uint SWP_HIDEWINDOW = 0x0080;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "GetWindowLongPtrW")]
    private static extern nint GetWindowLongPtr(nint hWnd, int nIndex);

    [DllImport("user32.dll", SetLastError = true, EntryPoint = "SetWindowLongPtrW")]
    private static extern nint SetWindowLongPtr(nint hWnd, int nIndex, nint dwNewLong);

    private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    private const int DWMWCP_ROUND = 2;

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint hWnd, int attribute, ref int value, int size);

    private readonly CatcherLog _log;

    internal CatcherWindow(CatcherLog log)
    {
        _log = log;
        InitializeComponent();
        SourceInitialized += OnSourceInitialized;
    }

    /// <summary>Raised on the UI thread with whatever the shell handed over. The paths are not
    /// checked here — Plith stats them, because Plith is the side that has to care.</summary>
    public event Action<IReadOnlyList<string>>? FilesDropped;

    private void OnSourceInitialized(object? sender, EventArgs e)
    {
        var handle = new WindowInteropHelper(this).Handle;

        // TOOLWINDOW keeps it out of Alt+Tab; NOACTIVATE keeps a click on it from pulling focus
        // away from whatever the person was doing. Neither affects the drop.
        var style = GetWindowLongPtr(handle, GWL_EXSTYLE);
        _ = SetWindowLongPtr(handle, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE);

        // Rounded corners without AllowsTransparency, which would make this a layered window and
        // take it out of reach of every screen-capture route — the blind spot that let four
        // accessibility defects ship green in the OSD itself.
        var round = DWMWCP_ROUND;
        _ = DwmSetWindowAttribute(handle, DWMWA_WINDOW_CORNER_PREFERENCE, ref round, sizeof(int));
    }

    /// <summary>
    /// Physical screen pixels, applied without conversion. WPF's Left/Top would route the same
    /// numbers through this process's own idea of the DPI scale, which is exactly the arithmetic
    /// the wire format exists to avoid duplicating.
    /// </summary>
    public void ShowAt(int x, int y, int width, int height)
    {
        // Show() first, and it is not optional: EnsureHandle() alone creates the HWND and
        // SWP_SHOWWINDOW alone makes it visible to the window manager, but WPF does not consider
        // the window shown — it builds no visual tree and registers no OLE drop target. The
        // result is a window that exists, sits in the right rectangle, draws nothing, and
        // silently refuses every drop. Measured on the first probe run, where the only symptom
        // was MainWindowHandle staying 0.
        if (!IsVisible) Show();

        var handle = new WindowInteropHelper(this).Handle;
        _ = SetWindowPos(handle, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
        _log.Info($"Shown at {x},{y} {width}x{height}. AllowDrop={AllowDrop}, handle=0x{handle:X}");
    }

    public void HideNow()
    {
        if (!IsVisible) return;

        // Hide() rather than SWP_HIDEWINDOW, so WPF's own idea of visibility stays in step with
        // the window manager's. Out of step, the next ShowAt would skip Show() and land back in
        // the bug above.
        Hide();
        _log.Info("Hidden.");
    }

    protected override void OnDragEnter(DragEventArgs e)
    {
        base.OnDragEnter(e);
        _log.Info($"DragEnter. FileDrop present: {e.Data.GetDataPresent(DataFormats.FileDrop)}");
        e.Effects = Effects(e);
        e.Handled = true;
    }

    protected override void OnDragOver(DragEventArgs e)
    {
        base.OnDragOver(e);
        e.Effects = Effects(e);
        e.Handled = true;
    }

    protected override void OnDrop(DragEventArgs e)
    {
        base.OnDrop(e);
        e.Handled = true;

        var paths = e.Data.GetData(DataFormats.FileDrop) as string[] ?? [];
        _log.Info($"DROP: {paths.Length} path(s): {string.Join(" | ", paths)}");

        HideNow();
        if (paths.Length > 0) FilesDropped?.Invoke(paths);
    }

    /// <summary>
    /// Copy, not Move — the shelf stages a reference to a file, it does not take the file away
    /// from where it was. Move would let a drag onto the notch delete the original.
    /// </summary>
    private static DragDropEffects Effects(DragEventArgs e)
        => e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None;
}
