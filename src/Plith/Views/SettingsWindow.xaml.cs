using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Threading;
using Plith.Services;

namespace Plith.Views;

public partial class SettingsWindow : Window
{
    private readonly SettingsService _settings;
    private readonly HotkeyService _hotkey;
    private readonly ThemeService _theme;
    private bool _loadingFromModel;
    private DispatcherTimer? _savedPulseTimer;

    // Captured combo for the in-progress hotkey recording. Apply on first valid KeyDown.
    private uint _capturedMods;
    private int _capturedKey;
    private bool _isCapturingHotkey;

    public SettingsWindow(SettingsService settings, HotkeyService hotkey, ThemeService theme)
    {
        _settings = settings;
        _hotkey = hotkey;
        _theme = theme;
        InitializeComponent();

        BusCombo.ItemsSource = new[]
        {
            "A1 (0)", "A2 (1)", "A3 (2)", "A4 (3)", "A5 (4)",
            "B1 (5)", "B2 (6)", "B3 (7)",
        };

        PopulateEndpointCombo();

        // Plith stays a general-purpose OSD — every backend Plith knows how to read stays
        // visible in the picker even when that backend isn't on the machine. Instead of
        // hiding controls, we surface a small hint that names which backend is live right
        // now, and disable the Voicemeeter bus picker when Voicemeeter can't be reached
        // (still visible so the user sees the option exists).
        UpdateAudioSourceHint();
        BusCombo.IsEnabled = VoicemeeterClient.IsInstalled;

        WirePreview();
        WireAutoSave();
        LoadIntoUi(_settings.Current);

        HotkeyCaptureButton.Click += (_, _) => StartHotkeyCapture();
        HotkeyClearButton.Click += (_, _) => ClearHotkey();

        MinimizeButton.Click += (_, _) => WindowState = WindowState.Minimized;
        MaximizeButton.Click += (_, _) => WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
        CloseButton.Click += (_, _) => Close();

        StateChanged += (_, _) =>
        {
            MaximizeButton.Content = WindowState == WindowState.Maximized ? "" : "";
            MaximizeButton.ToolTip = WindowState == WindowState.Maximized ? "Restore" : "Maximize";
        };

        PreviewKeyDown += OnPreviewKeyDown;
        SourceInitialized += (_, _) =>
        {
            ApplyRoundedCorners();
            ApplyImmersiveDarkMode();
        };

        // BindingChanged fires after App.ApplyHotkeyFromSettings has talked to Windows,
        // so this is the only place that knows whether the user's combo was accepted.
        _hotkey.BindingChanged += OnHotkeyBindingChanged;
        _theme.ThemeApplied += OnThemeApplied;
        Closed += (_, _) =>
        {
            _hotkey.BindingChanged -= OnHotkeyBindingChanged;
            _theme.ThemeApplied -= OnThemeApplied;
        };
        UpdateHotkeyConflictWarning();
        ApplyGameModeStatus();
    }

    private void SelectEndpointById(string? id)
    {
        // Match against the id we persisted; if it's gone (device removed since last run),
        // fall back to the Default sentinel so the OSD still shows something meaningful.
        var items = EndpointCombo.ItemsSource as IEnumerable<WindowsAudioEndpointInfo>;
        if (items is null) return;
        WindowsAudioEndpointInfo? match = null;
        foreach (var e in items)
        {
            if (string.Equals(e.Id, id ?? string.Empty, StringComparison.OrdinalIgnoreCase)) { match = e; break; }
        }
        EndpointCombo.SelectedItem = match ?? items.FirstOrDefault();
    }

    private void PopulateEndpointCombo()
    {
        // "Default" sentinel + every active Windows render endpoint. Enumerated on window
        // open so Sonar (which registers new endpoints when its Streamer mode toggles) is
        // always current. If the persisted endpoint id no longer exists, selection falls
        // back to Default silently — the user sees the picker at Default and can re-pick.
        var items = new List<WindowsAudioEndpointInfo>
        {
            new(string.Empty, "Default (follow Windows)"),
        };
        items.AddRange(WindowsAudioClient.EnumerateRenderEndpoints());
        EndpointCombo.ItemsSource = items;
    }

    private void UpdateAudioSourceHint()
    {
        // Reports which backend Plith is currently reading, so users who juggle multiple
        // mixers (Voicemeeter, SteelSeries Sonar exposed as Windows endpoints, plain Windows)
        // can see at a glance whether their preferred source is live.
        string vmState = VoicemeeterClient.IsInstalled ? "installed" : "not installed";
        AudioSourceHint.Text =
            $"Auto uses Voicemeeter when it's running, otherwise the Windows default endpoint " +
            $"(this is where SteelSeries Sonar / VoiceMeeter alternatives surface). " +
            $"Voicemeeter: {vmState}.";
    }

    private void ApplyGameModeStatus()
    {
        bool active = UiAccessProbe.IsGameModeActive();
        GameModeDot.Background = (System.Windows.Media.Brush)FindResource(active ? "Accent" : "WarningAmber");
        GameModeStatusLabel.Text = active
            ? "Game mode: Active"
            : "Game mode: Limited";
        GameModeHint.Text = active
            ? "Plith is signed and running from a trusted location — OSD draws over exclusive fullscreen games."
            : @"Run scripts\install-local.ps1 (admin) to install Plith with UIAccess and draw over exclusive fullscreen games.";
    }

    private void OnThemeApplied()
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(OnThemeApplied));
            return;
        }
        ApplyImmersiveDarkMode();
        // The mini OSD card in the preview pane caches threshold brushes per the OSD viewmodel
        // contract; refresh it in tandem with the main OSD so the bar colour matches the swap.
        Preview?.PreviewViewModel.RefreshThresholdBrushes();
    }

    private void OnHotkeyBindingChanged()
    {
        // BindingChanged can be raised from a non-UI thread in principle (Apply runs on
        // whichever dispatcher loaded the message window). Marshal to ours before touching XAML.
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action(UpdateHotkeyConflictWarning));
            return;
        }
        UpdateHotkeyConflictWarning();
    }

    private void UpdateHotkeyConflictWarning()
    {
        var m = _settings.Current;
        bool wantsHotkey = m.HasSummonHotkey;
        bool isActive = _hotkey.IsBound
                        && _hotkey.ActiveMods == m.SummonHotkeyMods
                        && _hotkey.ActiveKey == m.SummonHotkeyKey;
        HotkeyConflictWarning.Visibility = wantsHotkey && !isActive
            ? Visibility.Visible
            : Visibility.Collapsed;
    }

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        // While the user is recording a hotkey, swallow the key event and process it as a
        // capture instead of letting Esc close the window or letters reach the OSD.
        if (_isCapturingHotkey)
        {
            CaptureHotkeyKeyDown(e);
            return;
        }
        if (e.Key == Key.Escape)
        {
            Close();
            e.Handled = true;
        }
    }

    private void StartHotkeyCapture()
    {
        _isCapturingHotkey = true;
        _capturedMods = 0;
        _capturedKey = 0;
        HotkeyCaptureButton.Content = "Press a combo…";
        HotkeyCaptureButton.FontStyle = FontStyles.Italic;
        HotkeyClearButton.Visibility = Visibility.Collapsed;
        Keyboard.Focus(HotkeyCaptureButton);
    }

    private void CaptureHotkeyKeyDown(KeyEventArgs e)
    {
        // Esc cancels capture without changing the existing binding.
        if (e.Key == Key.Escape)
        {
            EndHotkeyCapture(cancelled: true);
            e.Handled = true;
            return;
        }

        // The actual key arrives in SystemKey when the Alt modifier is held.
        var key = e.Key == Key.System ? e.SystemKey : e.Key;

        // Pure-modifier presses (Ctrl alone, Shift alone, …) don't constitute a complete
        // combo — let the user keep holding modifiers and wait for the trigger key.
        if (key is Key.LeftCtrl or Key.RightCtrl or Key.LeftAlt or Key.RightAlt
                or Key.LeftShift or Key.RightShift or Key.LWin or Key.RWin
                or Key.None or Key.System)
        {
            e.Handled = true;
            return;
        }

        var mods = Keyboard.Modifiers;
        uint modsMask =
            ((mods & ModifierKeys.Control) != 0 ? (uint)HotkeyService.HotkeyMods.Control : 0) |
            ((mods & ModifierKeys.Alt) != 0     ? (uint)HotkeyService.HotkeyMods.Alt     : 0) |
            ((mods & ModifierKeys.Shift) != 0   ? (uint)HotkeyService.HotkeyMods.Shift   : 0) |
            ((mods & ModifierKeys.Windows) != 0 ? (uint)HotkeyService.HotkeyMods.Win     : 0);

        // A summon-hotkey must include at least one modifier — bare letter keys would conflict
        // with normal typing the moment the user focuses any input control.
        if (modsMask == 0)
        {
            HotkeyCaptureButton.Content = "Need a modifier (Ctrl / Alt / Shift)";
            e.Handled = true;
            return;
        }

        _capturedMods = modsMask;
        _capturedKey = KeyInterop.VirtualKeyFromKey(key);
        EndHotkeyCapture(cancelled: false);
        AutoSave();
        e.Handled = true;
    }

    private void EndHotkeyCapture(bool cancelled)
    {
        _isCapturingHotkey = false;
        HotkeyCaptureButton.FontStyle = FontStyles.Normal;

        if (cancelled)
        {
            // Restore the visual to whatever was previously saved.
            RefreshHotkeyButton(_settings.Current.SummonHotkeyMods, _settings.Current.SummonHotkeyKey);
            return;
        }

        RefreshHotkeyButton(_capturedMods, _capturedKey);
    }

    private void RefreshHotkeyButton(uint mods, int vk)
    {
        var label = HotkeyService.FormatCombo(mods, vk);
        if (string.IsNullOrEmpty(label))
        {
            HotkeyCaptureButton.Content = "Not set";
            HotkeyClearButton.Visibility = Visibility.Collapsed;
        }
        else
        {
            HotkeyCaptureButton.Content = label;
            HotkeyClearButton.Visibility = Visibility.Visible;
        }
    }

    private void ClearHotkey()
    {
        _capturedMods = 0;
        _capturedKey = 0;
        RefreshHotkeyButton(0, 0);
        AutoSave();
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint hwnd, int attr, ref int value, int size);

    private const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;
    private const int DWMWA_WINDOW_CORNER_PREFERENCE = 33;
    private const int DWMWCP_ROUND = 2;

    private void ApplyRoundedCorners()
    {
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int pref = DWMWCP_ROUND;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, ref pref, sizeof(int));
    }

    private void ApplyImmersiveDarkMode()
    {
        // Aligns the title bar / Mica tint with the active palette. Without this, a Light
        // theme would still get a dark title bar (carried over from the system default for
        // WPF windows). Safe to call on every theme apply — the attribute is idempotent.
        var hwnd = new WindowInteropHelper(this).Handle;
        if (hwnd == 0) return;
        int dark = _theme.IsEffectiveDark ? 1 : 0;
        _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref dark, sizeof(int));
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
            _capturedMods = m.SummonHotkeyMods;
            _capturedKey = m.SummonHotkeyKey;
            RefreshHotkeyButton(m.SummonHotkeyMods, m.SummonHotkeyKey);
            SourceCombo.SelectedItem = m.AudioSource;
            BusCombo.SelectedIndex = Math.Clamp(m.MonitoredBusIndex, 0, BusCombo.Items.Count - 1);
            SelectEndpointById(m.MonitoredWindowsEndpointId);
            AutoShowMediaToggle.IsChecked = m.AutoShowOnMedia;
            AutoStartToggle.IsChecked = m.AutoStart;
            ThemeCombo.SelectedItem = m.Theme;
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
        // Hotkey button is wired through StartHotkeyCapture/ClearHotkey, not a SelectionChanged.
        SourceCombo.SelectionChanged += (_, _) => AutoSave();
        BusCombo.SelectionChanged += (_, _) => AutoSave();
        EndpointCombo.SelectionChanged += (_, _) => AutoSave();
        AutoShowMediaToggle.Checked += (_, _) => AutoSave();
        AutoShowMediaToggle.Unchecked += (_, _) => AutoSave();
        AutoStartToggle.Checked += (_, _) => AutoSave();
        AutoStartToggle.Unchecked += (_, _) => AutoSave();
        ThemeCombo.SelectionChanged += (_, _) => AutoSave();
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
        m.SummonHotkeyMods = _capturedMods;
        m.SummonHotkeyKey = _capturedKey;
        if (SourceCombo.SelectedItem is AudioSourceMode src) m.AudioSource = src;
        m.MonitoredBusIndex = Math.Max(0, BusCombo.SelectedIndex);
        m.MonitoredWindowsEndpointId = (EndpointCombo.SelectedItem as WindowsAudioEndpointInfo)?.Id ?? string.Empty;
        m.AutoShowOnMedia = AutoShowMediaToggle.IsChecked == true;
        m.AutoStart = AutoStartToggle.IsChecked == true;
        if (ThemeCombo.SelectedItem is Plith.Services.ThemeMode t) m.Theme = t;

        _settings.Save(m);
        AutoStartService.Apply(m.AutoStart);
    }
}
