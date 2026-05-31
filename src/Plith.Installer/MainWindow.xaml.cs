using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace Plith.Installer;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        MinimizeButton.Click += (_, _) => WindowState = WindowState.Minimized;
        CloseButton.Click += (_, _) => Close();

        SourceInitialized += (_, _) =>
        {
            ApplyRoundedCorners();
            ApplyImmersiveDarkMode();
            ApplyMica();
        };

        // Page navigation (NavigateTo) will be wired up by the orchestrator in a later task.
        // For now MainWindow just hosts an empty ContentControl named PageHost.
    }

    /// <summary>Replace the current page in the host. Used by App.xaml.cs and orchestrator.</summary>
    public void NavigateTo(System.Windows.Controls.UserControl page)
    {
        PageHost.Content = page;
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint hwnd, int attr, ref int value, int size);

    private const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;
    private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    private const int DWMWA_SYSTEMBACKDROP_TYPE = 38;
    private const int DWMWCP_ROUND = 2;
    private const int DWMSBT_MAINWINDOW = 2; // Mica

    private void ApplyRoundedCorners()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int pref = DWMWCP_ROUND;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, ref pref, sizeof(int));
    }

    private void ApplyImmersiveDarkMode()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int dark = 1;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref dark, sizeof(int));
    }

    private void ApplyMica()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int backdrop = DWMSBT_MAINWINDOW;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, ref backdrop, sizeof(int));
    }
}
