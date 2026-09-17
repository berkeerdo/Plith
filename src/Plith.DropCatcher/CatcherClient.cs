using System.IO;
using System.IO.Pipes;
using Plith.Services.Shelf;

namespace Plith.DropCatcher;

/// <summary>
/// The catcher's end of the wire.
///
/// Reconnecting rather than one-shot: Plith is the process people restart, and a catcher that
/// gave up on the first failed connect would be a silently dead shelf until the next sign-in.
/// The retry is slow on purpose — nothing is waiting on it, and a tight loop against a pipe that
/// will not exist until Plith next launches is a busy wait that lasts for days.
/// </summary>
internal sealed class CatcherClient : IDisposable
{
    private static readonly TimeSpan RetryDelay = TimeSpan.FromSeconds(3);

    private readonly string _pipeName;
    private readonly CatcherLog _log;
    private readonly CancellationTokenSource _cts = new();
    private NamedPipeClientStream? _pipe;
    private StreamWriter? _writer;

    public CatcherClient(string userSid, CatcherLog log)
    {
        _pipeName = DropChannel.PipeName(userSid);
        _log = log;
    }

    /// <summary>Raised off the UI thread. Marshal before touching a window.</summary>
    public event Action<DropMessage>? Received;

    public void Start() => _ = Task.Run(() => RunAsync(_cts.Token), CancellationToken.None);

    private async Task RunAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                var pipe = new NamedPipeClientStream(".", _pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
                await pipe.ConnectAsync(ct).ConfigureAwait(false);

                _pipe = pipe;
                _writer = new StreamWriter(pipe) { AutoFlush = true };
                _log.Info("Connected to Plith.");

                await SendAsync(new DropMessage(DropVerb.Hello, 0, 0, 0, 0, [])).ConfigureAwait(false);

                using var reader = new StreamReader(pipe, leaveOpen: true);
                while (!ct.IsCancellationRequested && pipe.IsConnected)
                {
                    var line = await reader.ReadLineAsync(ct).ConfigureAwait(false);
                    if (line is null) break;
                    if (DropChannel.TryDecode(line, out var message)) Received?.Invoke(message);
                }

                _log.Info("Disconnected.");
            }
            catch (OperationCanceledException) { return; }
            catch (IOException ex) { _log.Info($"Pipe error: {ex.GetType().Name}: {ex.Message}"); }
            catch (UnauthorizedAccessException ex)
            {
                // The one failure that means the design is broken rather than the app is closed:
                // Plith's pipe exists but refuses this process. That is what a default ACL on a
                // High-integrity pipe produces, and it is why DropChannelServer builds its own.
                _log.Info($"ACCESS DENIED on the pipe — Plith's ACL is not open to this process. {ex.Message}");
            }

            _writer = null;
            _pipe?.Dispose();
            _pipe = null;

            try { await Task.Delay(RetryDelay, ct).ConfigureAwait(false); }
            catch (OperationCanceledException) { return; }
        }
    }

    public async Task SendAsync(DropMessage message)
    {
        if (_writer is not { } writer || _pipe is not { IsConnected: true }) return;

        try { await writer.WriteLineAsync(DropChannel.Encode(message)).ConfigureAwait(false); }
        catch (IOException ex) { _log.Info($"Send failed: {ex.Message}"); }
        catch (ObjectDisposedException) { }
    }

    public void Dispose()
    {
        _cts.Cancel();
        _pipe?.Dispose();
        _cts.Dispose();
    }
}
