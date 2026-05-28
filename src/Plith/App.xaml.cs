using System.Windows;
using Plith.Services;
using Plith.Views;

namespace Plith;

public partial class App : Application
{
    private TrayIconHost? _trayHost;
    private OsdOrchestrator? _orchestrator;
    private OsdWindow? _osd;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _osd = new OsdWindow();
        _osd.Show();   // create the native handle now so first ShowOsd is instant; Opacity=0 keeps it invisible
        _orchestrator = new OsdOrchestrator(_osd);
        _orchestrator.Start();

        _trayHost = new TrayIconHost(this);
        _trayHost.Initialize();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _orchestrator?.Dispose();
        _trayHost?.Dispose();
        base.OnExit(e);
    }
}
