using System.Threading;
using System.Windows;

namespace Plith.Installer;

// CA1001: App owns _singleInstanceMutex but WPF Application cannot implement IDisposable.
// The mutex is released and disposed in OnExit, which is the correct WPF lifetime hook.
#pragma warning disable CA1001
public partial class App : Application
{
#pragma warning restore CA1001
    // Single-instance mutex — prevents two installer windows competing for the cert store
    // and the install dir. Mutex named with a unique GUID so it doesn't collide with any
    // other software using a "Plith" mutex name.
    private const string SingleInstanceMutexName = "Global\\Plith.Installer.SingleInstance.7F9C8E1A";
    private Mutex? _singleInstanceMutex;

    public bool IsUninstallMode { get; private set; }

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // initiallyOwned: false because we only need the mutex as a system-wide existence flag —
        // checking createdNew is what tells us we're the first instance. Owning the mutex would
        // require ReleaseMutex() from the same thread on shutdown, which throws if this is the
        // second-instance path that never acquired ownership.
        _singleInstanceMutex = new Mutex(initiallyOwned: false, SingleInstanceMutexName, out bool createdNew);
        if (!createdNew)
        {
            MessageBox.Show("Plith Setup is already running.",
                "Plith Setup", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown();
            return;
        }

        IsUninstallMode = e.Args.Length > 0 && e.Args[0] == "--uninstall";

        var window = new MainWindow();
        window.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
