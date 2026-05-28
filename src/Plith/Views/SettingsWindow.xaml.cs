using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using Plith.Services;

namespace Plith.Views;

public partial class SettingsWindow : Window
{
    private readonly SettingsService _settings;

    public SettingsWindow(SettingsService settings)
    {
        _settings = settings;
        InitializeComponent();

        // Populate the bus dropdown with friendly Voicemeeter bus names.
        BusCombo.ItemsSource = new[]
        {
            "A1 (0)", "A2 (1)", "A3 (2)", "A4 (3)", "A5 (4)",
            "B1 (5)", "B2 (6)", "B3 (7)",
        };

        LoadIntoUi(_settings.Current);

        SaveButton.Click += (_, _) => { ApplyFromUi(); Close(); };
        CancelButton.Click += (_, _) => Close();

        MinimizeButton.Click += (_, _) => WindowState = WindowState.Minimized;
        CloseButton.Click += (_, _) => Close();

        // Esc dismisses without saving.
        PreviewKeyDown += (_, e) =>
        {
            if (e.Key == System.Windows.Input.Key.Escape)
            {
                Close();
                e.Handled = true;
            }
        };

        // Ask DWM for Win11 rounded window corners on the borderless window. Silently no-ops
        // on Windows 10 — DWMWA_WINDOW_CORNER_PREFERENCE is ignored there.
        SourceInitialized += (_, _) => ApplyRoundedCorners();
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint hwnd, int attr, ref int value, int size);

    private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    private const int DWMWCP_ROUND = 2;

    private void ApplyRoundedCorners()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int pref = DWMWCP_ROUND;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, ref pref, sizeof(int));
    }

    private void LoadIntoUi(SettingsModel m)
    {
        DurationSlider.Value = m.ShowDurationMs;
        PositionCombo.SelectedItem = m.Position;
        HoverToggle.IsChecked = m.HoverKeepAlive;
        BusCombo.SelectedIndex = Math.Clamp(m.MonitoredBusIndex, 0, BusCombo.Items.Count - 1);
        AutoShowMediaToggle.IsChecked = m.AutoShowOnMedia;
        AutoStartToggle.IsChecked = m.AutoStart;
    }

    private void ApplyFromUi()
    {
        var m = _settings.Current.Clone();
        m.ShowDurationMs = (int)Math.Round(DurationSlider.Value);
        if (PositionCombo.SelectedItem is OsdPosition pos) m.Position = pos;
        m.HoverKeepAlive = HoverToggle.IsChecked == true;
        m.MonitoredBusIndex = Math.Max(0, BusCombo.SelectedIndex);
        m.AutoShowOnMedia = AutoShowMediaToggle.IsChecked == true;
        m.AutoStart = AutoStartToggle.IsChecked == true;

        // Apply registry change in a try/finally with the Save so a throwing Changed subscriber
        // or a failing INI write doesn't leave INI and registry out of sync. Either both succeed
        // visibly or the user sees an error.
        try
        {
            _settings.Save(m);
        }
        finally
        {
            AutoStartService.Apply(m.AutoStart);
        }
    }
}
