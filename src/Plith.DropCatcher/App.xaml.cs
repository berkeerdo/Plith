using System.Globalization;
using System.Security.Principal;
using System.Windows;
using System.Windows.Threading;
using Plith.Services.Shelf;

namespace Plith.DropCatcher;

public partial class App : Application, IDisposable
{
    private CatcherLog _log = null!;
    private CatcherWindow _window = null!;
    private CatcherClient? _client;
    private Mutex? _single;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _log = new CatcherLog();
        var sid = WindowsIdentity.GetCurrent().User?.Value ?? "unknown";

        // Per-user, matching the pipe name: one catcher per signed-in session, and a second
        // instance quits rather than fighting the first for the same rectangle.
        _single = new Mutex(true, @"Local\Plith.DropCatcher." + sid, out var isFirst);
        if (!isFirst)
        {
            _log.Info("Another catcher is already running for this user. Exiting.");
            Shutdown();
            return;
        }

        _log.Info($"Started. Integrity: {IntegrityLevel.Describe()}");

        _window = new CatcherWindow(_log);
        _window.FilesDropped += OnFilesDropped;

        if (TryReadProbeRect(e.Args, out var probe))
        {
            // Stand-alone mode, for measuring whether a Medium window can receive a drop at all
            // without Plith having to be involved. Nothing else in the design is worth building
            // if this fails, so it is reachable on its own.
            _log.Info($"PROBE MODE. Log: {_log.LogPath}");
            _window.ShowAt(probe.x, probe.y, probe.w, probe.h);
            return;
        }

        _client = new CatcherClient(sid, _log);
        _client.Received += OnReceived;
        _client.Start();
    }

    private void OnReceived(DropMessage message) => Dispatcher.Invoke(() =>
    {
        switch (message.Verb)
        {
            case DropVerb.Show:
                _window.ShowAt((int)message.X, (int)message.Y, (int)message.W, (int)message.H);
                break;
            case DropVerb.Hide:
                _window.HideNow();
                break;
            default:
                break;
        }
    }, DispatcherPriority.Send);

    private void OnFilesDropped(IReadOnlyList<string> paths)
        => _ = _client?.SendAsync(new DropMessage(DropVerb.Dropped, 0, 0, 0, 0, paths));

    private static bool TryReadProbeRect(string[] args, out (int x, int y, int w, int h) rect)
    {
        rect = default;
        if (args.Length != 5 || !args[0].Equals("--probe", StringComparison.OrdinalIgnoreCase)) return false;

        var numbers = new int[4];
        for (var i = 0; i < 4; i++)
        {
            if (!int.TryParse(args[i + 1], NumberStyles.Integer, CultureInfo.InvariantCulture, out numbers[i]))
                return false;
        }

        rect = (numbers[0], numbers[1], numbers[2], numbers[3]);
        return true;
    }

    protected override void OnExit(ExitEventArgs e)
    {
        Dispose();
        base.OnExit(e);
    }

    public void Dispose()
    {
        _client?.Dispose();
        _client = null;
        _single?.Dispose();
        _single = null;
        GC.SuppressFinalize(this);
    }
}
