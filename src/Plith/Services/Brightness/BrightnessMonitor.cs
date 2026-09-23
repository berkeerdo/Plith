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
    private bool _sawEvent;

    public BrightnessMonitor(Dispatcher dispatcher, DiagnosticLog? log = null)
    {
        _dispatcher = dispatcher;
        _log = log;
    }

    /// <summary>Raised on the UI dispatcher with the new percentage.</summary>
    public event Action<int>? Changed;

    /// <summary>
    /// The WMI namespace the event lives in.
    ///
    /// Named rather than inline so the test suite can assert it parses. It shipped as
    /// <c>\.\root\wmi</c>, one backslash short of a path, and <see cref="ManagementScope"/>
    /// throws "Invalid parameter" on it from the constructor, before WMI is contacted at all.
    /// </summary>
    internal const string ScopePath = @"\\.\root\wmi";

    public void Start()
    {
        if (_watcher is not null || _disposed) return;

        try
        {
            var scope = new ManagementScope(ScopePath);
            var query = new WqlEventQuery("SELECT * FROM WmiMonitorBrightnessEvent");
            _watcher = new ManagementEventWatcher(scope, query);
            _watcher.EventArrived += OnEventArrived;
            _watcher.Start();
            _log?.Info("Brightness", "Watching WmiMonitorBrightnessEvent.");
        }
        catch (Exception ex)
        {
            // WARN, and the level is the lesson rather than a preference. This read "No
            // brightness event source" at INFO for the life of the feature, which is what a
            // desktop with no internal panel was expected to say, so a malformed namespace
            // path produced a line that looked like the machine answering normally.
            //
            // It cannot mean "no panel". MEASURED on a desktop with no internal panel and no
            // WmiMonitorBrightness instance: the subscription starts without throwing and
            // simply never fires. Subscribing succeeds wherever WMI exists, so reaching here
            // means the query or the namespace is wrong, and that is a defect every time.
            //
            // Still caught, and still wide: this runs during startup and no brightness event
            // is worth failing a launch over.
            _log?.Warn("Brightness", $"Could not subscribe to brightness events: {ex.GetType().Name}: {ex.Message}");
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

        // The FIRST event only, then silence. Holding a brightness key raises these as fast as
        // the driver moves the panel, so a line each would be the log's largest source by far.
        // One line answers the only question worth asking from the outside: did this feature
        // ever hear anything on this machine. It shipped broken precisely because nothing here
        // could say so, and "Watching WmiMonitorBrightnessEvent." only reports the subscription.
        if (!_sawEvent)
        {
            _sawEvent = true;
            _log?.Info("Brightness", $"First brightness event received: {percent}%.");
        }

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
