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

    /// <summary>Guards the link, not the write. See <see cref="SendAsync"/>.</summary>
    private readonly object _sendGate = new();
    private Task _sendChain = Task.CompletedTask;

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

    /// <summary>
    /// Queue a message. Returns the task that completes when THIS message has been written, and
    /// every message queued before it has been written first.
    ///
    /// The serialization matches DropChannelServer.SendAsync, and it is owed for the same reason
    /// rather than for symmetry: this end writes every message through ONE shared StreamWriter
    /// over one pipe stream, and every call site is fire-and-forget. Two overlapping writes on
    /// one StreamWriter is not a corrupted line, it is an InvalidOperationException ("The stream
    /// is currently in use by a previous operation"), thrown into a task nobody awaits, so the
    /// message is silently lost and nothing anywhere reports it.
    ///
    /// Slice 1 had two verbs on this side and two sends in flight was hard to reach. This slice
    /// added five more and made it ordinary: a Restack raised from inside DoDragDrop, followed
    /// milliseconds later by the dismissal's ShelfClosed. A LOST ShelfClosed is the expensive
    /// one - Plith's window stays hidden for the life of a shelf, so the person is left with no
    /// notch and no OSD on any volume key until the catcher dies.
    ///
    /// A chain rather than a semaphore, for the reason the server end records: SemaphoreSlim
    /// documents no FIFO guarantee for its async waiters, so two callers could be released in
    /// the other order. Here the link is made under the lock, in CALL order, so the sequence is
    /// fixed before any await happens.
    ///
    /// What is copied is the reasoning, not the lines. The server's WriteAsync swallows the
    /// three exception types a dead pipe produces and can therefore never fault its chain; this
    /// one has to swallow InvalidOperationException as well, because that is what a StreamWriter
    /// raises for the overlap above and for a write into a pipe that has gone - and the
    /// belt-and-braces catch in <see cref="WriteAfterAsync"/> is what makes "can never fault"
    /// true of this chain rather than merely intended.
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

    /// <summary>
    /// Wait for the send before this one, then write.
    ///
    /// The Task.Run comes FIRST, as on the server end, and for the same reason: an async method
    /// runs synchronously until its first await, and this one is called from inside the lock.
    /// Without the hop, a send whose predecessor had already finished would do the whole pipe
    /// write on the caller's thread and inside the lock. That caller is the catcher's UI thread,
    /// which is the thread the shelf itself runs on, and this pipe is created with both buffer
    /// sizes at zero so a write does not return until Plith reads it.
    /// </summary>
    private async Task AfterAsync(Task previous, DropMessage message)
        => await Task.Run(() => WriteAfterAsync(previous, message)).ConfigureAwait(false);

    /// <summary>
    /// The chain's link, and it CANNOT FAULT, which is the whole design of this method.
    ///
    /// A faulted task assigned to _sendChain is rethrown by the next link's `await previous`,
    /// which faults that one too, and so on for the life of the process. Every call site here is
    /// fire-and-forget, so nothing would ever notice: after one broken write the catcher would be
    /// silently and permanently mute, which is the worse version of the very defect the chain
    /// exists to fix.
    ///
    /// Two catches rather than one. The first keeps a PREVIOUS send's failure from becoming this
    /// send's failure, which is what makes the chain self-healing. The second means this link
    /// never hands a faulted task to the next one for an exception nobody predicted, and stops
    /// the last send of a run from ending as an unobserved fault.
    /// </summary>
    private async Task WriteAfterAsync(Task previous, DropMessage message)
    {
        try
        {
            await previous.ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            _log.Info($"The send before this one failed; continuing: {ex.GetType().Name}: {ex.Message}");
        }

        try
        {
            await WriteAsync(message).ConfigureAwait(false);
        }
        catch (Exception ex)
        {
            _log.Info($"Send failed unexpectedly: {ex.GetType().Name}: {ex.Message}");
        }
    }

    private async Task WriteAsync(DropMessage message)
    {
        // Re-read inside the write rather than captured by the caller: a link can sit in the
        // chain across a disconnect and a reconnect, and the writer it should use is whichever
        // one is live when its turn comes.
        if (_writer is not { } writer || _pipe is not { IsConnected: true }) return;

        try { await writer.WriteLineAsync(DropChannel.Encode(message)).ConfigureAwait(false); }
        catch (IOException ex) { _log.Info($"Send failed: {ex.Message}"); }
        catch (ObjectDisposedException) { }
        catch (InvalidOperationException ex)
        {
            // What a StreamWriter throws for a write that overlaps another on the same writer,
            // and what PipeStream throws for a write on a pipe that is no longer connected. The
            // chain above makes the first unreachable; the second is RACED rather than avoidable,
            // since the IsConnected check and RunAsync's teardown are on different threads.
            _log.Info($"Send into a closed pipe: {ex.Message}");
        }
    }

    public void Dispose()
    {
        _cts.Cancel();
        _pipe?.Dispose();
        _cts.Dispose();
    }
}
