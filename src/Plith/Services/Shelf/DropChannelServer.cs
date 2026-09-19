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

    /// <summary>
    /// A catcher that WAS connected is gone. Raised off the UI thread, once per connection lost,
    /// and never for a connection that was never made.
    ///
    /// It exists because IsConnected cannot be polled for this. PipeStream caches its state and
    /// does not probe, so the flag only turns false once an operation on the pipe has failed or
    /// this loop has called Disconnect. Anything waiting on an answer from the catcher therefore
    /// has no way to learn that no answer is coming, and the shelf is exactly that: Plith's own
    /// window is down for the duration, and without this signal a catcher dying while the shelf
    /// is up leaves the OSD hidden until Plith restarts.
    /// </summary>
    public event Action? Disconnected;

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

            // Announced BEFORE Disconnect, while the flag still says what happened, and guarded
            // on it so a cancelled wait for a first connection does not report a loss. Whoever
            // handles it is on this thread, so it must not block: App marshals it.
            if (_pipe is { IsConnected: true }) Disconnected?.Invoke();

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

    /// <summary>
    /// Wait for the send before this one, then write.
    ///
    /// The Task.Run comes FIRST and is not decoration. An async method runs synchronously until
    /// its first await, and this one is called from inside the lock; without the hop, a send
    /// whose predecessor had already finished would run the whole write on the caller's thread
    /// and inside the lock. The caller is normally the UI thread and the write is pipe I/O, so
    /// the cost of getting this wrong is a frozen OSD rather than a wrong one.
    /// </summary>
    private async Task AfterAsync(Task previous, DropMessage message)
        => await Task.Run(() => WriteAfterAsync(previous, message)).ConfigureAwait(false);

    /// <summary>
    /// The chain's link, and it CANNOT FAULT. That is the whole design of this method.
    ///
    /// The first version let exceptions through, and that was a real defect rather than an
    /// untidiness: a faulted task assigned to _sendChain is rethrown by the next link's
    /// `await previous`, which faults that one too, and so on for the life of the process. Every
    /// call site is fire-and-forget, so nothing would ever have noticed. After one broken write
    /// the channel would be silently and permanently mute, and the way to reach it was ordinary:
    /// the catcher dying mid-send.
    ///
    /// Two catches rather than one. The first keeps a PREVIOUS send's failure from becoming this
    /// send's failure, which is what makes the chain self-healing. The second means this link
    /// never hands a faulted task to the next one at all, for an exception nobody predicted, and
    /// stops the last send of a run from ending as an unobserved fault, since every call site
    /// discards the task it gets back.
    ///
    /// The alternative was resetting _sendChain to Task.CompletedTask on a fault. Not taken: it
    /// needs a second place that mutates the chain, and it has to do so from a continuation
    /// running outside the lock that fixes the order, which is the one property the lock exists
    /// to guarantee. A link that cannot fault needs neither.
    /// </summary>
    private async Task WriteAfterAsync(Task previous, DropMessage message)
    {
        try
        {
            await previous.ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            _log?.Info("DropChannel", $"The send before this one failed; continuing: {ExceptionText.Describe(ex)}");
        }

        try
        {
            await WriteAsync(message).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            _log?.Warn("DropChannel", $"Send failed unexpectedly: {ExceptionText.Describe(ex)}");
        }
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
            // Shutting down mid-send. FIRST, because ObjectDisposedException derives from
            // InvalidOperationException and the compiler rejects the other order outright: the
            // narrower clause has to precede the wider one.
        }
        catch (InvalidOperationException ex)
        {
            // What PipeStream throws for a write on a pipe that is no longer connected, and it is
            // RACED rather than avoidable: the IsConnected check above and AcceptLoop's
            // Disconnect() run on different threads, and the window between them is exactly when
            // the catcher dies. Caught beside IOException because it means the same thing.
            _log?.Info("DropChannel", $"Send into a closed pipe: {ExceptionText.Describe(ex)}");
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
