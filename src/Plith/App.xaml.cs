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

        _trayHost = new TrayIconHost(this, _settings);
        _trayHost.Initialize();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _orchestrator?.Dispose();
        _trayHost?.Dispose();
        base.OnExit(e);
    }
}
