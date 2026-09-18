using System.Globalization;
using System.Management;
using System.Windows.Threading;

namespace Plith.Services.Brightness;

/// <summary>
/// Notices that the screen's brightness changed, whoever changed it.
///
/// Windows raises WmiMonitorBrightnessEvent on every brightness change of an internal panel
/// and carries the new percentage in the event, so nothing has to be queried afterwards. That
/// is why this feature hooks no keys: laptop brightness keys are consumed in the driver stack
/// and there is no VK_BRIGHTNESS in the Windows SDK to hook even if they were not. Listening
/// to the change also covers the Settings slider, the Quick Settings panel, and any other
/// application.
///
/// It covers INTERNAL PANELS ONLY. An external monitor never raises it, because DDC/CI is
/// request and response and the monitor never announces anything. On a desktop this class
/// starts, never fires, and costs nothing, which is the correct behaviour rather than a
/// degraded one.
/// </summary>
public sealed class BrightnessMonitor : IDisposable
{
    private readonly Dispatcher _dispatcher;
    private readonly DiagnosticLog? _log;
    private ManagementEventWatcher? _watcher;
    private bool _disposed;

    public BrightnessMonitor(Dispatcher dispatcher, DiagnosticLog? log = null)
    {
        _dispatcher = dispatcher;
        _log = log;
    }

    /// <summary>Raised on the UI dispatcher with the new percentage.</summary>
    public event Action<int>? Changed;

    public void Start()
    {
        if (_watcher is not null || _disposed) return;

        try
        {
            var scope = new ManagementScope(@"\.\root\wmi");
            var query = new WqlEventQuery("SELECT * FROM WmiMonitorBrightnessEvent");
            _watcher = new ManagementEventWatcher(scope, query);
            _watcher.EventArrived += OnEventArrived;
            _watcher.Start();
            _log?.Info("Brightness", "Watching WmiMonitorBrightnessEvent.");
        }
        catch (Exception ex)
        {
            // No internal panel, or WMI is unavailable. Degrade silently, the way
            // MediaSessionClient does when SMTC is missing. The catch is deliberately wide:
            // this runs during startup, and the exception types this stack produces on a
            // machine with no panel are not documented anywhere the product can rely on.
            _log?.Info("Brightness", $"No brightness event source: {ex.GetType().Name}: {ex.Message}");
            _watcher = null;
        }
    }

    private void OnEventArrived(object sender, EventArrivedEventArgs e)
    {
        // WMI raises on a worker thread. CardHost reconciles straight into an
        // ObservableCollection bound to a live ItemsControl and its own documentation names
        // this exact case as the expected cause of a crash deep inside the WPF binding
        // engine. Nothing may reach a card from here without this hop.
        int percent;
        try
        {
            percent = Convert.ToInt32(e.NewEvent["Brightness"], CultureInfo.InvariantCulture);
        }
        catch (Exception)
        {
            return;
        }

        if (_disposed) return;
        _dispatcher.BeginInvoke(new Action(() => { if (!_disposed) Changed?.Invoke(percent); }));
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        if (_watcher is null) return;
        try
        {
            _watcher.EventArrived -= OnEventArrived;
            _watcher.Stop();
            _watcher.Dispose();
        }
        catch (ManagementException) { }
        _watcher = null;
    }
}
