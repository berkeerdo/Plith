using System.IO;
using System.IO.Pipes;
using System.Security.AccessControl;
using System.Security.Principal;

namespace Plith.Services.Shelf;

/// <summary>
/// Plith's end of the wire.
///
/// The ACL is the whole point of this class. Plith runs at High integrity because it has
/// UIAccess, and a pipe it creates with the default security carries a High mandatory label —
/// a Medium process cannot write to it. Measured: default ACL gives "Access to the path is
/// denied"; an explicit rule for Everyone connects and delivers.
///
/// That is a deliberate widening, so it is stated rather than buried: any process on this
/// machine can connect to this pipe and send a Dropped message. Everything arriving is treated
/// as a claim about paths, never as a command — see ShelfStore, which stats what it is told
/// about and stores nothing it cannot see.
/// </summary>
public sealed class DropChannelServer : IDisposable
{
    private readonly string _pipeName;
    private readonly DiagnosticLog? _log;
    private NamedPipeServerStream? _pipe;
    private CancellationTokenSource? _cts;
    private bool _disposed;

    public DropChannelServer(string userSid, DiagnosticLog? log = null)
    {
        _pipeName = DropChannel.PipeName(userSid);
        _log = log;
    }

    /// <summary>Raised off the UI thread, once per decoded line.</summary>
    public event Action<DropMessage>? Received;

    /// <summary>True while a catcher is on the other end. Plith must not hide the notch for a
    /// catcher that is not there — that would leave the drag over nothing at all.</summary>
    public bool IsConnected => _pipe is { IsConnected: true };

    public void Start()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
        if (_cts is not null) return;

        _cts = new CancellationTokenSource();
        _pipe = CreatePipe();
        _ = Task.Run(() => AcceptLoop(_cts.Token), CancellationToken.None);
    }

    private NamedPipeServerStream CreatePipe()
    {
        var security = new PipeSecurity();
        security.AddAccessRule(new PipeAccessRule(
            new SecurityIdentifier(WellKnownSidType.WorldSid, null),
            PipeAccessRights.ReadWrite,
            AccessControlType.Allow));

        return NamedPipeServerStreamAcl.Create(
            _pipeName, PipeDirection.InOut, 1, PipeTransmissionMode.Byte,
            PipeOptions.Asynchronous, 0, 0, security);
    }

    internal PipeSecurity DebugSecurity() => _pipe!.GetAccessControl();

    private async Task AcceptLoop(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            try
            {
                await _pipe!.WaitForConnectionAsync(ct).ConfigureAwait(false);
                using var reader = new StreamReader(_pipe, leaveOpen: true);

                while (!ct.IsCancellationRequested && _pipe.IsConnected)
                {
                    var line = await reader.ReadLineAsync(ct).ConfigureAwait(false);
                    if (line is null) break;
                    if (DropChannel.TryDecode(line, out var message)) Received?.Invoke(message);
                    else _log?.Warn("DropChannel", "Discarded a line that did not decode.");
                }
            }
            catch (OperationCanceledException) { return; }
            catch (ObjectDisposedException) { return; }
            catch (IOException ex)
            {
                // The catcher died or was restarted. Ordinary, not a fault: disconnect and wait
                // again rather than leaving the shelf permanently deaf.
                _log?.Info("DropChannel", $"Client gone: {ExceptionText.Describe(ex)}");
            }

            try { _pipe!.Disconnect(); }
            catch (ObjectDisposedException) { return; }
            catch (InvalidOperationException) { /* was never connected */ }
        }
    }

    /// <summary>
    /// Queue a message. Returns the task that completes when THIS message has been written, and
    /// every message queued before it has been written first.
    ///
    /// The serialization is here rather than at the call sites, and it is not a nicety. Every
    /// send builds a fresh <see cref="StreamWriter"/> over the one shared pipe stream (see
    /// <see cref="WriteAsync"/> for why it must), and every call site is fire-and-forget. Two
    /// sends in flight at once are therefore two writers appending to the same stream with an
    /// await in the middle of each: a named pipe preserves the order bytes are handed to it, so
    /// no reordering is possible, but nothing stops one writer's bytes landing in the MIDDLE of
    /// the other's line. What arrives is one line ending in another line's tail, which decodes
    /// to nonsense, or worse, to a real message with the wrong numbers in it.
    ///
    /// That was reachable the moment the shelf started sending one Items message PER STACK.
    /// ShelfModel on the far side rejects a stack whose declared total disagrees with the set it
    /// is assembling, but that is a detection of last resort, not a substitute for sending
    /// correctly: it cannot notice a corrupted line that still parses.
    ///
    /// A chain rather than a semaphore, because the order has to be the CALL order and nothing
    /// else. SemaphoreSlim documents no FIFO guarantee for its async waiters, so two callers
    /// could be released in the other order; here the link is made under the lock, in call
    /// order, so the sequence is fixed before any await happens.
    /// </summary>
    public Task SendAsync(DropMessage message)
    {
        lock (_sendGate)
        {
            var next = AfterAsync(_sendChain, message);
            _sendChain = next;
            return next;
        }
    }

    private readonly object _sendGate = new();
    private Task _sendChain = Task.CompletedTask;

    /// <summary>Wait for the send before this one, then write. <see cref="WriteAsync"/> swallows
    /// its own failures, so the chain cannot be left faulted and a single failed send cannot
    /// poison every send after it.</summary>
    private async Task AfterAsync(Task previous, DropMessage message)
    {
        await previous.ConfigureAwait(false);
        await WriteAsync(message).ConfigureAwait(false);
    }

    private async Task WriteAsync(DropMessage message)
    {
        if (_pipe is not { IsConnected: true } pipe) return;

        try
        {
            // Not cached and not disposed: disposing the writer would close the pipe under the
            // read loop, and a cached one would outlive the connection it was built on.
            var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
            await writer.WriteLineAsync(DropChannel.Encode(message)).ConfigureAwait(false);
        }
        catch (IOException ex)
        {
            _log?.Info("DropChannel", $"Send failed: {ExceptionText.Describe(ex)}");
        }
        catch (ObjectDisposedException)
        {
            // Shutting down mid-send.
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        _cts?.Cancel();
        _pipe?.Dispose();
        _cts?.Dispose();
    }
}
