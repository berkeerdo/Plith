using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Threading;
using Plith.Services;

namespace Plith.Views;

public partial class SettingsWindow : Window
{
    private readonly SettingsService _settings;
    private readonly HotkeyService _hotkey;
    private readonly ThemeService _theme;
    private readonly OsdHost _osd;
    private readonly UpdateCheckService _updates = new();
    private readonly List<AccentSwatch> _accentSwatches = new();
    private bool _loadingFromModel;
    private bool _loadingCustomPickerFromModel;
    private DispatcherTimer? _savedPulseTimer;

    // Row entry for each preset + the single custom swatch in the accent picker.
    // Kept as a mutable record so RefreshAccentSelection can flip visuals in-place
    // without rebuilding the WrapPanel on every settings change.
    // Root is a FrameworkElement (in practice always the Button built by CreateSwatch /
    // CreateCustomSwatch) rather than Button so callers that only need to add it to the
    // WrapPanel don't have to know its concrete type.
    private sealed record AccentSwatch(string Id, FrameworkElement Root, Border Fill, TextBlock Tick, TextBlock? PlusIcon, bool IsCustom);

    // Captured combo for the in-progress hotkey recording. Apply on first valid KeyDown.
    private uint _capturedMods;
    private int _capturedKey;
    private bool _isCapturingHotkey;

    // Latest CheckAsync result held between the check button and the download button so we
    // don't have to hit GitHub twice per user flow.
    private UpdateInfo? _lastUpdateInfo;

    public SettingsWindow(SettingsService settings, HotkeyService hotkey, ThemeService theme, OsdHost osd)
    {
        _settings = settings;
        _hotkey = hotkey;
        _theme = theme;
        _osd = osd;
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
            string maximizeLabel = WindowState == WindowState.Maximized ? "Restore" : "Maximize";
            MaximizeButton.ToolTip = maximizeLabel;
            AutomationProperties.SetName(MaximizeButton, maximizeLabel);
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
        WireUpdateCheck();
        WirePositionEditor();
        BuildAccentSwatches();
        WireAccentPicker();
    }

    private void WirePositionEditor()
    {
        OpenPositionOverlayButton.Click += (_, _) => _osd.EnterPositionEditMode();
        _osd.EditModeChanged += OnOsdEditModeChanged;
        Closed += (_, _) => _osd.EditModeChanged -= OnOsdEditModeChanged;
        RefreshPositionSummary();
    }

    private void OnOsdEditModeChanged(bool isEditing)
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.BeginInvoke(new Action<bool>(OnOsdEditModeChanged), isEditing);
            return;
        }
        // Grey the button out while the overlay is open, and hide the Settings window
        // itself so its own chrome doesn't sit on top of the overlay hotspots.
        OpenPositionOverlayButton.IsEnabled = !isEditing;
        OpenPositionOverlayButton.Content = isEditing ? "Overlay open..." : "Set position";
        if (isEditing) Hide();
        else { Show(); Activate(); RefreshPositionSummary(); }
    }

    private void RefreshPositionSummary()
    {
        var m = _settings.Current;
        PositionSummary.Text = m.Position switch
        {
            OsdPosition.Custom => "Position: Custom (drag-placed). Click 'Set position' to move it again.",
            _                  => $"Position: {m.Position}. Click 'Set position' to open the overlay picker.",
        };
    }

    private void WireUpdateCheck()
    {
        var currentVersion = typeof(SettingsWindow).Assembly.GetName().Version?.ToString(3) ?? "0.0.0";
        UpdateStatusLabel.Text = $"Version {currentVersion}";
        UpdateCheckButton.Click += async (_, _) => await RunUpdateCheckAsync();
        UpdateOpenPageButton.Click += (_, _) => OpenReleasePage();
        UpdateDownloadButton.Click += async (_, _) => await RunUpdateDownloadAsync();
    }

    private async Task RunUpdateCheckAsync()
    {
        UpdateCheckButton.IsEnabled = false;
        UpdateStatusLabel.Text = "Checking GitHub…";
        UpdateStatusHint.Text = "Downloads from GitHub releases and runs the installer.";
        UpdateActionRow.Visibility = Visibility.Collapsed;
        try
        {
            var info = await _updates.CheckAsync();
            _lastUpdateInfo = info;

            if (info is null)
            {
                UpdateStatusLabel.Text = "Update check failed";
                UpdateStatusHint.Text = "Network error or GitHub API unreachable. Try again later.";
                return;
            }

            if (!info.IsAvailable)
            {
                UpdateStatusLabel.Text = $"You're up to date — v{info.CurrentVersion}";
                UpdateStatusHint.Text = $"Latest release on GitHub is v{info.LatestVersion}.";
                return;
            }

            UpdateStatusLabel.Text = $"Update available: v{info.LatestVersion}";
            UpdateStatusHint.Text = info.InstallerAssetUrl is null
                ? "Release published but no installer asset attached — use Release notes."
                : $"Currently on v{info.CurrentVersion}. Click Download and install to apply.";
            UpdateActionRow.Visibility = Visibility.Visible;
            UpdateDownloadButton.IsEnabled = info.InstallerAssetUrl is not null;
        }
        finally
        {
            UpdateCheckButton.IsEnabled = true;
        }
    }

    private void OpenReleasePage()
    {
        if (_lastUpdateInfo?.ReleasePageUrl is not { Length: > 0 } url) return;
        try
        {
            Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
        }
        catch { /* the user can copy the URL from the status text if the shell fails */ }
    }

    private async Task RunUpdateDownloadAsync()
    {
        if (_lastUpdateInfo?.InstallerAssetUrl is not { Length: > 0 } assetUrl) return;

        UpdateDownloadButton.IsEnabled = false;
        UpdateCheckButton.IsEnabled = false;
        UpdateProgressBar.Visibility = Visibility.Visible;
        UpdateProgressBar.Value = 0;
        UpdateStatusHint.Text = "Downloading installer…";

        var progress = new Progress<double>(p =>
        {
            UpdateProgressBar.Value = p;
            UpdateStatusHint.Text = $"Downloading installer… {p * 100:F0}%";
        });

        var path = await _updates.DownloadInstallerAsync(assetUrl, _lastUpdateInfo.InstallerAssetSize, progress);
        if (path is null)
        {
            UpdateStatusHint.Text = "Download failed. Check plith.log or try Release notes.";
            UpdateProgressBar.Visibility = Visibility.Collapsed;
            UpdateDownloadButton.IsEnabled = true;
            UpdateCheckButton.IsEnabled = true;
            return;
        }

        UpdateStatusHint.Text = "Launching installer — Plith will exit.";
        try
        {
            // The installer needs UAC because it writes to Program Files. UseShellExecute + RunAs
            // gets the elevation prompt. Plith exits immediately after so the installer's file
            // replace step doesn't collide with our own binary being held open.
            Process.Start(new ProcessStartInfo(path)
            {
                UseShellExecute = true,
                Verb = "runas",
            });
            Application.Current.Shutdown();
        }
        catch (Exception ex)
        {
            UpdateStatusHint.Text = $"Failed to start installer: {ex.Message}";
            UpdateDownloadButton.IsEnabled = true;
            UpdateCheckButton.IsEnabled = true;
        }
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
        // The mini OSD card in the preview pane caches threshold brushes per the audio card
        // view-model contract; refresh it in tandem with the main OSD so the bar colour
        // matches the swap.
        Preview?.PreviewAudio.RefreshThresholdBrushes();
        // A palette polarity flip also changes the derived Accent brush, so the selection
        // ring on the current swatch needs to redraw with the new colour.
        RefreshAccentSelection();
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
            // Esc dismisses the custom-colour popup before it closes the whole window,
            // matching the "innermost overlay wins" convention users already have from
            // the position overlay picker.
            if (CustomAccentPopup?.IsOpen == true)
            {
                CustomAccentPopup.IsOpen = false;
                e.Handled = true;
                return;
            }
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
            HoverToggle.IsChecked = m.HoverKeepAlive;
            OpacitySlider.Value = m.OsdOpacityPercent;
            ColorThresholdsToggle.IsChecked = m.UseColorThresholds;
            CompactToggle.IsChecked = m.CompactMode;
            FullscreenVideoToggle.IsChecked = m.HideDuringFullscreenVideo;
            FullscreenHideListBox.Text = m.FullscreenVideoHideList;
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
        // Position no longer lives in a combo - it changes via the overlay picker,
        // which persists through SettingsService. Hook the model event so the
        // preview thumbnail refreshes when the overlay commits a new position.
        _settings.Changed += _ => Dispatcher.BeginInvoke(new Action(SyncPreview));
    }

    private void SyncPreview()
    {
        if (Preview is null) return;
        Preview.UpdateOpacity(OpacitySlider.Value / 100.0);
        Preview.UpdateColorThresholds(ColorThresholdsToggle.IsChecked == true);
        Preview.UpdateCompact(CompactToggle.IsChecked == true);
        Preview.UpdatePosition(_settings.Current.Position);
    }

    private void WireAutoSave()
    {
        DurationSlider.ValueChanged += (_, _) => AutoSave();
        HoverToggle.Checked += (_, _) => AutoSave();
        HoverToggle.Unchecked += (_, _) => AutoSave();
        OpacitySlider.ValueChanged += (_, _) => AutoSave();
        ColorThresholdsToggle.Checked += (_, _) => AutoSave();
        ColorThresholdsToggle.Unchecked += (_, _) => AutoSave();
        CompactToggle.Checked += (_, _) => AutoSave();
        CompactToggle.Unchecked += (_, _) => AutoSave();
        FullscreenVideoToggle.Checked += (_, _) => AutoSave();
        FullscreenVideoToggle.Unchecked += (_, _) => AutoSave();
        FullscreenHideListBox.LostFocus += (_, _) => AutoSave();
        // Matches CustomHexBox's pattern: LostFocus alone means an edit followed by
        // Alt+F4 (rather than tabbing/clicking away first) is lost. Enter commits without
        // requiring focus to leave the box.
        FullscreenHideListBox.KeyDown += (_, e) =>
        {
            if (e.Key == Key.Enter)
            {
                AutoSave();
                e.Handled = true;
            }
        };
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
        // Position is owned by the overlay picker (persisted directly through
        // SettingsService.Save from OsdHost); we leave m.Position untouched here.
        m.HoverKeepAlive = HoverToggle.IsChecked == true;
        m.OsdOpacityPercent = (int)Math.Round(OpacitySlider.Value);
        m.UseColorThresholds = ColorThresholdsToggle.IsChecked == true;
        m.CompactMode = CompactToggle.IsChecked == true;
        m.HideDuringFullscreenVideo = FullscreenVideoToggle.IsChecked == true;
        m.FullscreenVideoHideList = FullscreenHideListBox.Text ?? string.Empty;
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

    // =============================================================================
    //   Accent Theme Studio
    // =============================================================================
    //
    // The Appearance card carries a WrapPanel of preset swatches plus one Custom
    // swatch that opens the HSL popup declared next to it in XAML. Selection and
    // hover state live entirely on the swatch objects — no VMs, no bindings — so
    // theme swaps and settings-file loads can update the picker with a single
    // RefreshAccentSelection call.
    //
    // Persistence + live-apply path:
    //   click preset  → _settings.Save(m.AccentThemeId = id) → ThemeService.Apply
    //                   rebuilds the accent override → every DynamicResource'd
    //                   brush in both the Settings window and the OSD updates.
    //   drag slider   → same, with m.CustomAccentColor = hex.
    // -----------------------------------------------------------------------------

    private void BuildAccentSwatches()
    {
        AccentSwatchPanel.Children.Clear();
        _accentSwatches.Clear();

        foreach (var preset in AccentTheme.Presets)
        {
            var sw = CreateSwatch(preset.Id, preset.DisplayName, preset.BaseColor, isCustom: false);
            _accentSwatches.Add(sw);
            AccentSwatchPanel.Children.Add(sw.Root);
        }

        var custom = CreateCustomSwatch();
        _accentSwatches.Add(custom);
        AccentSwatchPanel.Children.Add(custom.Root);

        RefreshAccentSelection();
    }

    private AccentSwatch CreateSwatch(string id, string tooltip, Color baseColor, bool isCustom)
    {
        // Outer button reserves 2px for the halo-style selection ring, rendered by the
        // shared SwatchTemplate as a plain Border. The inner Fill is smaller so the ring
        // reads as a highlight around a solid dot instead of a coloured border on the
        // swatch itself. This used to be a bare Border with a MouseLeftButtonUp handler,
        // which meant it was unreachable by keyboard and invisible to a screen reader —
        // a Border exposes no focus and no invoke pattern. Button gets both for free.
        var root = new Button
        {
            Width = 40,
            Height = 40,
            Background = Brushes.Transparent,
            BorderBrush = Brushes.Transparent,
            BorderThickness = new Thickness(2),
            Padding = new Thickness(0),
            Cursor = Cursors.Hand,
            Margin = new Thickness(0, 0, 8, 8),
            ToolTip = tooltip,
            Template = SwatchTemplate,
        };
        AutomationProperties.SetName(root, tooltip);
        var grid = new Grid();
        var fill = new Border
        {
            Width = 28,
            Height = 28,
            CornerRadius = new CornerRadius(14),
            Background = new SolidColorBrush(baseColor),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var tick = new TextBlock
        {
            Text = "", // Segoe MDL2 Assets "CheckMark"
            FontFamily = new FontFamily("Segoe MDL2 Assets"),
            FontSize = 14,
            Foreground = ContrastText(baseColor),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed,
        };
        grid.Children.Add(fill);
        grid.Children.Add(tick);
        root.Content = grid;

        // Click (not a mouse event) so Space/Enter activate the swatch exactly like a
        // mouse click does, and there's no separate mouse handler left behind that could
        // fire alongside Click and double-invoke the same swatch.
        root.Click += (_, _) => OnSwatchClicked(id, isCustom);

        return new AccentSwatch(id, root, fill, tick, PlusIcon: null, IsCustom: isCustom);
    }

    private AccentSwatch CreateCustomSwatch()
    {
        var settings = _settings.Current;
        var currentCustom = AccentTheme.ParseHexColor(
            settings.CustomAccentColor,
            AccentTheme.ResolveBase(AccentTheme.DefaultId, null));

        var root = new Button
        {
            Width = 40,
            Height = 40,
            Background = Brushes.Transparent,
            BorderBrush = Brushes.Transparent,
            BorderThickness = new Thickness(2),
            Padding = new Thickness(0),
            Cursor = Cursors.Hand,
            Margin = new Thickness(0, 0, 8, 8),
            ToolTip = "Custom color",
            Template = SwatchTemplate,
        };
        AutomationProperties.SetName(root, "Custom color");
        var grid = new Grid();
        var fill = new Border
        {
            Width = 28,
            Height = 28,
            CornerRadius = new CornerRadius(14),
            Background = new SolidColorBrush(currentCustom),
            BorderBrush = (Brush)FindResource("CardBorderStrong"),
            BorderThickness = new Thickness(1),
        };
        // "+" glyph shows when the user hasn't picked a custom colour yet, cueing the
        // affordance. It's hidden as soon as they commit one so the swatch just reads
        // as a colour.
        var plus = new TextBlock
        {
            Text = "", // MDL2 "Add"
            FontFamily = new FontFamily("Segoe MDL2 Assets"),
            FontSize = 12,
            Foreground = ContrastText(currentCustom),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
        };
        var tick = new TextBlock
        {
            Text = "",
            FontFamily = new FontFamily("Segoe MDL2 Assets"),
            FontSize = 14,
            Foreground = ContrastText(currentCustom),
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            IsHitTestVisible = false,
            Visibility = Visibility.Collapsed,
        };
        grid.Children.Add(fill);
        grid.Children.Add(plus);
        grid.Children.Add(tick);
        root.Content = grid;

        root.Click += (_, _) => OnSwatchClicked(AccentTheme.CustomId, isCustom: true);

        return new AccentSwatch(AccentTheme.CustomId, root, fill, tick, plus, IsCustom: true);
    }

    // Shared ControlTemplate for every accent swatch Button: a single rounded Border
    // whose Background/BorderBrush/BorderThickness are template-bound back to the
    // Button's own properties, and whose content (the fill dot + tick/plus glyphs built
    // by CreateSwatch/CreateCustomSwatch) is rendered through a plain ContentPresenter.
    // This neutralises every piece of default Button chrome — no ButtonChrome, no
    // padding contribution, no theme-driven background — so the swatch renders exactly
    // like the old bare Border did, just with real focus/keyboard/UIA-invoke support.
    private static ControlTemplate? _swatchTemplate;
    private static ControlTemplate SwatchTemplate => _swatchTemplate ??= BuildSwatchTemplate();

    private static ControlTemplate BuildSwatchTemplate()
    {
        var border = new FrameworkElementFactory(typeof(Border));
        border.SetValue(Border.CornerRadiusProperty, new CornerRadius(20));
        border.SetValue(Border.BackgroundProperty, new TemplateBindingExtension(Button.BackgroundProperty));
        border.SetValue(Border.BorderBrushProperty, new TemplateBindingExtension(Button.BorderBrushProperty));
        border.SetValue(Border.BorderThicknessProperty, new TemplateBindingExtension(Button.BorderThicknessProperty));
        border.SetValue(Border.SnapsToDevicePixelsProperty, true);

        var content = new FrameworkElementFactory(typeof(ContentPresenter));
        content.SetValue(FrameworkElement.HorizontalAlignmentProperty, HorizontalAlignment.Stretch);
        content.SetValue(FrameworkElement.VerticalAlignmentProperty, VerticalAlignment.Stretch);
        border.AppendChild(content);

        return new ControlTemplate(typeof(Button)) { VisualTree = border };
    }

    private void OnSwatchClicked(string id, bool isCustom)
    {
        // Preserve the user's custom colour when switching to a preset: keep it on the
        // in-memory model so re-picking Custom later opens the popup on the last value.
        var m = _settings.Current.Clone();
        m.AccentThemeId = id;
        _settings.Save(m);
        // ThemeService listens on SettingsService.Changed and calls Apply -> rebuild
        // accent override -> refresh every DynamicResource'd Accent* brush in both
        // this window and the OSD. Nothing else to do for the theme itself.
        RefreshAccentSelection();
        PulseSavedIndicator();

        if (isCustom)
        {
            SyncCustomPickerFromCurrent();
            CustomAccentPopup.IsOpen = true;
            CustomHexBox.Focus();
        }
        else
        {
            CustomAccentPopup.IsOpen = false;
        }
    }

    private void WireAccentPicker()
    {
        CustomHueSlider.ValueChanged += (_, _) => OnCustomSliderChanged();
        CustomSatSlider.ValueChanged += (_, _) => OnCustomSliderChanged();
        CustomLumSlider.ValueChanged += (_, _) => OnCustomSliderChanged();
        CustomHexBox.LostFocus += (_, _) => OnCustomHexChanged();
        CustomHexBox.KeyDown += (_, e) =>
        {
            if (e.Key == Key.Enter)
            {
                OnCustomHexChanged();
                e.Handled = true;
            }
        };
    }

    private void SyncCustomPickerFromCurrent()
    {
        // Start point: the user's saved custom colour if any, otherwise the current
        // preset's base — so the popup doesn't jump to a colour that has nothing to
        // do with what they're looking at.
        var m = _settings.Current;
        var start = AccentTheme.TryParseHexColor(m.CustomAccentColor, out var saved)
            ? saved
            : AccentTheme.ResolveBase(
                string.Equals(m.AccentThemeId, AccentTheme.CustomId, StringComparison.OrdinalIgnoreCase)
                    ? AccentTheme.DefaultId
                    : m.AccentThemeId,
                null);

        _loadingCustomPickerFromModel = true;
        try
        {
            var (h, s, l) = AccentTheme.RgbToHsl(start);
            CustomHueSlider.Value = h;
            CustomSatSlider.Value = s * 100.0;
            CustomLumSlider.Value = l * 100.0;
            CustomHexBox.Text = AccentTheme.ToHex(start);
        }
        finally
        {
            _loadingCustomPickerFromModel = false;
        }
        UpdateCustomLabels(start);
    }

    private void OnCustomSliderChanged()
    {
        if (_loadingCustomPickerFromModel) return;
        double h = CustomHueSlider.Value;
        double s = CustomSatSlider.Value / 100.0;
        double l = CustomLumSlider.Value / 100.0;
        var color = AccentTheme.HslToRgb(h, s, l);
        ApplyCustomColor(color, updateHex: true, updateSliders: false);
    }

    private void OnCustomHexChanged()
    {
        if (_loadingCustomPickerFromModel) return;
        if (!AccentTheme.TryParseHexColor(CustomHexBox.Text, out var color))
        {
            // Restore last-known-good hex on invalid input so the box never sits in a
            // "you typed nonsense" state.
            CustomHexBox.Text = AccentTheme.ToHex(
                AccentTheme.ParseHexColor(_settings.Current.CustomAccentColor,
                    AccentTheme.ResolveBase(AccentTheme.DefaultId, null)));
            return;
        }
        ApplyCustomColor(color, updateHex: false, updateSliders: true);
    }

    private void ApplyCustomColor(Color color, bool updateHex, bool updateSliders)
    {
        _loadingCustomPickerFromModel = true;
        try
        {
            if (updateSliders)
            {
                var (h, s, l) = AccentTheme.RgbToHsl(color);
                CustomHueSlider.Value = h;
                CustomSatSlider.Value = s * 100.0;
                CustomLumSlider.Value = l * 100.0;
            }
            if (updateHex)
            {
                CustomHexBox.Text = AccentTheme.ToHex(color);
            }
            UpdateCustomLabels(color);
        }
        finally
        {
            _loadingCustomPickerFromModel = false;
        }

        var m = _settings.Current.Clone();
        m.AccentThemeId = AccentTheme.CustomId;
        m.CustomAccentColor = AccentTheme.ToHex(color);
        _settings.Save(m);
        RefreshAccentSelection();
        PulseSavedIndicator();
    }

    private void UpdateCustomLabels(Color color)
    {
        var (h, s, l) = AccentTheme.RgbToHsl(color);
        CustomHueValue.Text = $"{h:0}°";
        CustomSatValue.Text = $"{s * 100:0}";
        CustomLumValue.Text = $"{l * 100:0}";
        CustomPreviewSwatch.Background = new SolidColorBrush(color);
    }

    private void RefreshAccentSelection()
    {
        var id = _settings.Current.AccentThemeId ?? AccentTheme.DefaultId;
        // Selection ring uses the currently applied Accent brush so it matches the
        // rest of the UI (and shifts on dark/light swap alongside everything else).
        var accentBrush = (Brush)FindResource("Accent");

        foreach (var sw in _accentSwatches)
        {
            bool selected = string.Equals(sw.Id, id, StringComparison.OrdinalIgnoreCase);
            // Root is typed FrameworkElement on the record, but is always the Button
            // built by CreateSwatch/CreateCustomSwatch, which templates its BorderBrush
            // into the selection ring — so this cast is always safe.
            if (sw.Root is Control control)
            {
                control.BorderBrush = selected ? accentBrush : Brushes.Transparent;
            }
            sw.Tick.Visibility = selected ? Visibility.Visible : Visibility.Collapsed;
        }

        // Refresh custom swatch appearance from settings — the user may have picked a
        // new hex, or cleared it by switching to a preset (in which case the swatch
        // still remembers the last hex, only the picked-mode flag flips).
        var custom = _accentSwatches.LastOrDefault(s => s.IsCustom);
        if (custom is not null)
        {
            var customColor = AccentTheme.ParseHexColor(
                _settings.Current.CustomAccentColor,
                AccentTheme.ResolveBase(AccentTheme.DefaultId, null));
            custom.Fill.Background = new SolidColorBrush(customColor);
            var contrast = ContrastText(customColor);
            custom.Tick.Foreground = contrast;
            if (custom.PlusIcon is not null)
            {
                custom.PlusIcon.Foreground = contrast;
                // Hide the "+" hint whenever a custom colour is stored — the fill itself
                // is the affordance at that point.
                custom.PlusIcon.Visibility = string.IsNullOrWhiteSpace(_settings.Current.CustomAccentColor)
                    ? Visibility.Visible
                    : Visibility.Collapsed;
                // Don't stack the "+" on top of the tick when this swatch is selected.
                if (string.Equals(id, AccentTheme.CustomId, StringComparison.OrdinalIgnoreCase))
                    custom.PlusIcon.Visibility = Visibility.Collapsed;
            }
        }
    }

    // Rec.601 luma is close enough for a pass/fail readability decision against pure
    // black vs pure white overlay glyphs on a coloured swatch. Threshold picked so
    // Praxvon Lime (#CAFF33, luma 218) shows black text, Emerald (#4AD695, luma 175)
    // shows black text, and Violet (#BD93F9, luma 158) also picks black — while any
    // saturated dark tone gets white.
    private static SolidColorBrush ContrastText(Color bg)
    {
        double luma = 0.299 * bg.R + 0.587 * bg.G + 0.114 * bg.B;
        return luma > 150 ? Brushes.Black : Brushes.White;
    }
}
