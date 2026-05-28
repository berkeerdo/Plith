using System.Threading;

namespace Plith;

internal static class Program
{
    private static Mutex? _singleInstance;

    [STAThread]
    public static int Main()
    {
        _singleInstance = new Mutex(initiallyOwned: true, name: "Plith.Singleton.{8C0E5C7E-2E4E-4F9F-9A4F-8D2C9B5F2A1B}", out bool created);
        if (!created) return 0;

        var app = new App();
        app.InitializeComponent();
        try
        {
            return app.Run();
        }
        finally
        {
            // ReleaseMutex throws ApplicationException if the current thread no longer owns the
            // mutex (e.g. Environment.Exit shortcut, host abandons the STA thread). Swallow it so
            // it doesn't mask the real shutdown path; the OS releases the mutex on process exit.
            try { _singleInstance.ReleaseMutex(); }
            catch (ApplicationException) { }
            _singleInstance.Dispose();
        }
    }
}
