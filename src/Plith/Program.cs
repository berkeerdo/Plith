using System.Diagnostics;
using System.Threading;

namespace Plith;

internal static class Program
{
    private static Mutex? _singleInstance;

    [STAThread]
    public static int Main()
    {
        // The startup clock, and it is started here rather than in App because here is the
        // earliest point managed code of ours runs. Everything StartupTrace reports is measured
        // from this line, so anything moved above it becomes invisible to the instrument.
        var clock = Stopwatch.StartNew();
        var clrMs = RuntimeStartMs();

        _singleInstance = new Mutex(initiallyOwned: true, name: "Plith.Singleton.{8C0E5C7E-2E4E-4F9F-9A4F-8D2C9B5F2A1B}", out bool created);
        if (!created) return 0;

        var app = new App();
        app.UseStartupClock(() => clock.ElapsedMilliseconds, clrMs);
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

    /// <summary>
    /// What the runtime's own start cost: process creation to the line above. The CLR coming up,
    /// WPF's assemblies loading, and this path being jitted.
    ///
    /// The one span in the whole trace that cannot be read off a Stopwatch, because the interval
    /// is over before a Stopwatch could exist. Two clocks, one of them wall-clock, resolved to
    /// the system timer rather than the performance counter. Good to a few milliseconds, which is
    /// fine for a figure expected to run into the hundreds, and not the same quality of
    /// measurement as the spans beside it. StartupTrace says so where it reports it.
    ///
    /// Zero on failure rather than a guess. A column reading zero is obviously absent; a
    /// plausible invented number is the failure mode this repo has paid for before.
    /// </summary>
    private static long RuntimeStartMs()
    {
        try
        {
            using var self = Process.GetCurrentProcess();
            var elapsed = (long)(DateTime.Now - self.StartTime).TotalMilliseconds;
            return elapsed < 0 ? 0 : elapsed;
        }
        catch (InvalidOperationException) { return 0; }
        catch (System.ComponentModel.Win32Exception) { return 0; }
        catch (NotSupportedException) { return 0; }
    }
}
