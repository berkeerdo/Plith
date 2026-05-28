using System.Windows;
using Plith.Services;
using Plith.Views;

namespace Plith;

public partial class App : Application
{
    private SettingsService? _settings;
    private TrayIconHost? _trayHost;
    private OsdOrchestrator? _orchestrator;
    private OsdWindow? _osd;
    private NativeFlyoutSuppressor? _flyoutSuppressor;
    private HotkeyService? _hotkey;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _settings = new SettingsService();
        _settings.Load();

        // Reconcile the registry Run entry with the saved preference on every launch — the
        // user could have manually edited the INI between sessions, or moved the binary.
        AutoStartService.Apply(_settings.Current.AutoStart);

        _osd = new OsdWindow(_settings);
        _osd.Show();   // create the native handle now so first ShowOsd is instant; Opacity=0 keeps it invisible
        _orchestrator = new OsdOrchestrator(_osd, _settings);
        _orchestrator.Start();

        // Suppress the native Windows volume flyout system-wide so Plith's OSD is the only one
        // the user sees. Runs whether Voicemeeter is active or not — Windows still pops its
        // flyout on raw volume keys regardless of who's listening to the endpoint.
        _flyoutSuppressor = new NativeFlyoutSuppressor();
        _flyoutSuppressor.Start();

        _trayHost = new TrayIconHost(this, _settings);
        _trayHost.Initialize();

        // The summon hotkey pops the OSD with whatever values the view-model currently holds —
        // useful for one-handed media skips without touching the volume wheel. Default is None
        // (off); the user picks a combo in the settings window and we re-apply on every change.
        _hotkey = new HotkeyService();
        _hotkey.Pressed += () => _osd?.ShowOsd(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs));
        ApplyHotkeyFromSettings(_settings.Current);
        _settings.Changed += ApplyHotkeyFromSettings;
    }

    private void ApplyHotkeyFromSettings(SettingsModel m)
    {
        if (_hotkey is null) return;
        if (!_hotkey.Apply(m.SummonHotkey))
        {
            System.Diagnostics.Trace.WriteLine(
                $"Plith: hotkey {m.SummonHotkey} unavailable — already owned by another process.");
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        if (_settings is not null) _settings.Changed -= ApplyHotkeyFromSettings;
        _hotkey?.Dispose();
        _flyoutSuppressor?.Dispose();
        _orchestrator?.Dispose();
        _trayHost?.Dispose();
        base.OnExit(e);
    }
}
