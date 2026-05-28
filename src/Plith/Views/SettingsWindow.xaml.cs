using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Threading;
using Plith.Services;

namespace Plith.Views;

public partial class SettingsWindow : Window
{
    private readonly SettingsService _settings;
    private bool _loadingFromModel;
    private DispatcherTimer? _savedPulseTimer;

    public SettingsWindow(SettingsService settings)
    {
        _settings = settings;
        InitializeComponent();

        BusCombo.ItemsSource = new[]
        {
            "A1 (0)", "A2 (1)", "A3 (2)", "A4 (3)", "A5 (4)",
            "B1 (5)", "B2 (6)", "B3 (7)",
        };

        WirePreview();
        WireAutoSave();
        LoadIntoUi(_settings.Current);

        MinimizeButton.Click += (_, _) => WindowState = WindowState.Minimized;
        MaximizeButton.Click += (_, _) => WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
        CloseButton.Click += (_, _) => Close();

        StateChanged += (_, _) =>
        {
            MaximizeButton.Content = WindowState == WindowState.Maximized ? "" : "";
            MaximizeButton.ToolTip = WindowState == WindowState.Maximized ? "Restore" : "Maximize";
        };

        // Esc closes (auto-save model — there's nothing to discard).
        PreviewKeyDown += (_, e) =>
        {
            if (e.Key == System.Windows.Input.Key.Escape)
            {
                Close();
                e.Handled = true;
            }
        };

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
        _loadingFromModel = true;
        try
        {
            DurationSlider.Value = m.ShowDurationMs;
            PositionCombo.SelectedItem = m.Position;
            HoverToggle.IsChecked = m.HoverKeepAlive;
            OpacitySlider.Value = m.OsdOpacityPercent;
            ColorThresholdsToggle.IsChecked = m.UseColorThresholds;
            CompactToggle.IsChecked = m.CompactMode;
            HotkeyComboBox.SelectedItem = m.SummonHotkey;
            SourceCombo.SelectedItem = m.AudioSource;
            BusCombo.SelectedIndex = Math.Clamp(m.MonitoredBusIndex, 0, BusCombo.Items.Count - 1);
            AutoShowMediaToggle.IsChecked = m.AutoShowOnMedia;
            AutoStartToggle.IsChecked = m.AutoStart;
        }
        finally
        {
            _loadingFromModel = false;
        }
        SyncPreview();
    }

    private void WirePreview()
    {
        OpacitySlider.ValueChanged += (_, _) => SyncPreview();
        ColorThresholdsToggle.Checked += (_, _) => SyncPreview();
        ColorThresholdsToggle.Unchecked += (_, _) => SyncPreview();
        CompactToggle.Checked += (_, _) => SyncPreview();
        CompactToggle.Unchecked += (_, _) => SyncPreview();
        PositionCombo.SelectionChanged += (_, _) => SyncPreview();
    }

    private void SyncPreview()
    {
        if (Preview is null) return;
        Preview.UpdateOpacity(OpacitySlider.Value / 100.0);
        Preview.UpdateColorThresholds(ColorThresholdsToggle.IsChecked == true);
        Preview.UpdateCompact(CompactToggle.IsChecked == true);
        if (PositionCombo.SelectedItem is OsdPosition pos) Preview.UpdatePosition(pos);
    }

    private void WireAutoSave()
    {
        DurationSlider.ValueChanged += (_, _) => AutoSave();
        PositionCombo.SelectionChanged += (_, _) => AutoSave();
        HoverToggle.Checked += (_, _) => AutoSave();
        HoverToggle.Unchecked += (_, _) => AutoSave();
        OpacitySlider.ValueChanged += (_, _) => AutoSave();
        ColorThresholdsToggle.Checked += (_, _) => AutoSave();
        ColorThresholdsToggle.Unchecked += (_, _) => AutoSave();
        CompactToggle.Checked += (_, _) => AutoSave();
        CompactToggle.Unchecked += (_, _) => AutoSave();
        HotkeyComboBox.SelectionChanged += (_, _) => AutoSave();
        SourceCombo.SelectionChanged += (_, _) => AutoSave();
        BusCombo.SelectionChanged += (_, _) => AutoSave();
        AutoShowMediaToggle.Checked += (_, _) => AutoSave();
        AutoShowMediaToggle.Unchecked += (_, _) => AutoSave();
        AutoStartToggle.Checked += (_, _) => AutoSave();
        AutoStartToggle.Unchecked += (_, _) => AutoSave();
    }

    private void AutoSave()
    {
        // Skip during LoadIntoUi — otherwise every slider/toggle the loader touches would
        // trigger a save and the orchestrator would see N spurious Changed events on open.
        if (_loadingFromModel) return;
        ApplyFromUi();
        PulseSavedIndicator();
    }

    private void PulseSavedIndicator()
    {
        if (SavedIndicator is null) return;
        SavedIndicator.Opacity = 1.0;
        _savedPulseTimer?.Stop();
        _savedPulseTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(1500) };
        _savedPulseTimer.Tick += (_, _) =>
        {
            _savedPulseTimer!.Stop();
            var fade = new System.Windows.Media.Animation.DoubleAnimation(1.0, 0.55, TimeSpan.FromMilliseconds(250));
            SavedIndicator.BeginAnimation(OpacityProperty, fade);
        };
        _savedPulseTimer.Start();
    }

    private void ApplyFromUi()
    {
        var m = _settings.Current.Clone();
        m.ShowDurationMs = (int)Math.Round(DurationSlider.Value);
        if (PositionCombo.SelectedItem is OsdPosition pos) m.Position = pos;
        m.HoverKeepAlive = HoverToggle.IsChecked == true;
        m.OsdOpacityPercent = (int)Math.Round(OpacitySlider.Value);
        m.UseColorThresholds = ColorThresholdsToggle.IsChecked == true;
        m.CompactMode = CompactToggle.IsChecked == true;
        if (HotkeyComboBox.SelectedItem is HotkeyCombo hk) m.SummonHotkey = hk;
        if (SourceCombo.SelectedItem is AudioSourceMode src) m.AudioSource = src;
        m.MonitoredBusIndex = Math.Max(0, BusCombo.SelectedIndex);
        m.AutoShowOnMedia = AutoShowMediaToggle.IsChecked == true;
        m.AutoStart = AutoStartToggle.IsChecked == true;

        _settings.Save(m);
        AutoStartService.Apply(m.AutoStart);
    }
}
