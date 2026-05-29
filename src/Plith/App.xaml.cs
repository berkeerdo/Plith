using System.Windows;
using Plith.Services;
using Plith.Views;

namespace Plith;

// CA1001: App holds disposable fields but isn't IDisposable. WPF owns the Application
// lifecycle and calls OnExit on shutdown, where we explicitly Dispose them; making App
// IDisposable doesn't fit the WPF pattern. Justified suppression.
[System.Diagnostics.CodeAnalysis.SuppressMessage("Design", "CA1001:Types that own disposable fields should be disposable",
    Justification = "Disposable fields are released in OnExit, matching the WPF Application lifecycle.")]
public partial class App : Application
{
    private SettingsService? _settings;
    private TrayIconHost? _trayHost;
    private OsdOrchestrator? _orchestrator;
    private OsdWindow? _osd;
    private HotkeyService? _hotkey;
    private ThemeService? _theme;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _settings = new SettingsService();
        _settings.Load();

        // Reconcile the registry Run entry with the saved preference on every launch — the
        // user could have manually edited the INI between sessions, or moved the binary.
        AutoStartService.Apply(_settings.Current.AutoStart);

        // ThemeService must start before any Window is shown so the first paint already
        // uses the active palette; otherwise a Light user would see a one-frame dark flash.
        _theme = new ThemeService(this, _settings);
        _theme.Start();
        // The OSD viewmodel caches threshold brush references for the hot-path GainColor
        // getter; tell it to re-resolve from the active palette every time the theme swaps.
        _theme.ThemeApplied += () => _osd?.ViewModel.RefreshThresholdBrushes();

        _osd = new OsdWindow(_settings);
        _osd.Show();   // create the native handle now so first ShowOsd is instant; Opacity=0 keeps it invisible
        _orchestrator = new OsdOrchestrator(_osd, _settings);
        _orchestrator.Start();

        // NativeFlyoutSuppressor is intentionally NOT started. The class-and-process
        // matching net was wide enough on Win11 26200 to hide non-flyout shell windows
        // (Start menu, taskbar popups) owned by Explorer, breaking the desktop until
        // Plith was killed. The service stays in the codebase for a future, properly
        // narrow opt-in implementation; for now both OSDs co-exist on volume change,
        // which is strictly better UX than a wedged Windows shell.

        // The summon hotkey pops the OSD with whatever values the view-model currently holds —
        // useful for one-handed media skips without touching the volume wheel. Default is None
        // (off); the user picks a combo in the settings window and we re-apply on every change.
        // _hotkey is created BEFORE _trayHost so the tray can hand the service to SettingsWindow
        // for the binding-conflict warning.
        _hotkey = new HotkeyService();
        _hotkey.Pressed += () => _osd?.ShowOsd(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs));
        ApplyHotkeyFromSettings(_settings.Current);
        _settings.Changed += ApplyHotkeyFromSettings;

        _trayHost = new TrayIconHost(this, _settings, _hotkey, _theme);
        _trayHost.Initialize();
    }

    private void ApplyHotkeyFromSettings(SettingsModel m)
    {
        if (_hotkey is null) return;
        if (!_hotkey.Apply(m.SummonHotkeyMods, m.SummonHotkeyKey))
        {
            System.Diagnostics.Trace.WriteLine(
                "Plith: hotkey " + HotkeyService.FormatCombo(m.SummonHotkeyMods, m.SummonHotkeyKey)
                + " unavailable — already owned by another process.");
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        if (_settings is not null) _settings.Changed -= ApplyHotkeyFromSettings;
        _hotkey?.Dispose();
        _theme?.Dispose();
        _orchestrator?.Dispose();
        _osd?.AllowShutdown();    // unblock OnClosing so real shutdown can destroy the window
        _trayHost?.Dispose();
        base.OnExit(e);
    }
}
