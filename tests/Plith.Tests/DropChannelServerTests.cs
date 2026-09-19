using System.IO;
using System.IO.Pipes;
using System.Security.AccessControl;
using System.Security.Principal;
using Plith.Services.Shelf;

namespace Plith.Tests;

public class DropChannelServerTests
{
    private static string TestSid() => "S-1-5-21-test-" + Guid.NewGuid().ToString("N");

    /// <summary>
    /// The assumption the whole design rests on: a lower-integrity process must be able to reach
    /// this pipe. Measured before the design was written — a default ACL answers "Access to the
    /// path is denied", and an explicit one connects. This test cannot reproduce the integrity
    /// difference in-process, so it asserts the thing that CAUSES the difference: that the pipe
    /// carries a rule for Everyone rather than the default owner-only security.
    /// </summary>
    [Fact]
    public void Server_OpensThePipeToEveryone()
    {
        using var server = new DropChannelServer(TestSid());
        server.Start();

        var rules = server.DebugSecurity().GetAccessRules(true, false, typeof(SecurityIdentifier));

        Assert.Contains(rules.Cast<PipeAccessRule>(),
            r => ((SecurityIdentifier)r.IdentityReference).IsWellKnown(WellKnownSidType.WorldSid)
                 && r.AccessControlType == AccessControlType.Allow
                 && r.PipeAccessRights.HasFlag(PipeAccessRights.ReadWrite));
    }

    /// <summary>
    /// A client connects, sends, and the server raises what it sent. Same integrity level here —
    /// the cross-integrity half is what the ACL above buys, and only a real Medium process can
    /// prove that. This proves the other half: that the framing survives a real pipe, where a
    /// line arrives in whatever chunks the OS chooses rather than whole.
    /// </summary>
    [Fact]
    public async Task Server_RaisesWhatAClientSends()
    {
        var sid = TestSid();
        using var server = new DropChannelServer(sid);
        var received = new TaskCompletionSource<DropMessage>();
        server.Received += m => received.TrySetResult(m);
        server.Start();

        using var client = new NamedPipeClientStream(".", DropChannel.PipeName(sid), PipeDirection.InOut);
        await client.ConnectAsync(5000);
        var writer = new StreamWriter(client) { AutoFlush = true };
        await writer.WriteLineAsync(DropChannel.Encode(
            new DropMessage(DropVerb.Dropped, 0, 0, 0, 0, [SomePath])));

        Assert.Same(received.Task, await Task.WhenAny(received.Task, Task.Delay(5000)));
        var message = await received.Task;
        Assert.Equal(DropVerb.Dropped, message.Verb);
        Assert.Equal(SomePath, Assert.Single(message.Paths));
    }

    /// <summary>
    /// The hazard Task 6 made reachable: several sends in flight on one pipe at once.
    ///
    /// The shelf sends one Items message PER STACK and every call site is fire-and-forget, so
    /// this is the ordinary case rather than a corner. Each send builds a fresh StreamWriter over
    /// the shared stream and awaits inside it; two overlapping writers can therefore put one
    /// line's bytes in the middle of another's, and what arrives decodes to nonsense or, worse,
    /// to a valid message with the wrong numbers in it.
    ///
    /// Asserted two ways, because one is not enough. That every line DECODES catches an
    /// interleave that produced garbage; that the indices come back 0..n-1 IN ORDER catches an
    /// interleave that happened to leave both lines parseable, and also catches a serialization
    /// built on something with no ordering guarantee.
    ///
    /// Verified to catch a real break rather than a hypothetical one: with SendAsync reverted to
    /// a direct call to the write, the reader got back a line carrying 96 fields instead of 5 -
    /// many messages spliced into one - and it DECODED, as an Items message with the right verb
    /// and the right index and ninety-one paths that are not paths. That is the failure worth
    /// naming: not a line that arrives broken, but one that arrives plausible.
    ///
    /// The payload size is load-bearing and was measured too. At 400 characters a message the
    /// same break does NOT reproduce here: StreamWriter buffers 1024 characters by default, so a
    /// short line becomes one write with no await inside it and lands whole by construction. It
    /// takes a line longer than that buffer to split into several writes with suspension points
    /// between them. A shelf of twenty items with ordinary Windows paths clears 1024 characters
    /// easily, so this is the product's own size rather than a size invented to fail.
    /// </summary>
    [Fact]
    public async Task Server_DoesNotInterleaveOverlappingSends()
    {
        const int Messages = 20;

        var sid = TestSid();
        using var server = new DropChannelServer(sid);
        server.Start();

        using var client = new NamedPipeClientStream(".", DropChannel.PipeName(sid), PipeDirection.InOut);
        await client.ConnectAsync(5000);

        // The client's connect returning and the server's accept completing are two events, and
        // a send before the second one is dropped on the floor by design (there is nobody to send
        // to). Waited for rather than slept past, so the test measures ordering and not timing.
        for (var i = 0; i < 100 && !server.IsConnected; i++) await Task.Delay(20);
        Assert.True(server.IsConnected);

        // A long payload per message, so a write is big enough for the OS to split rather than
        // landing atomically by luck.
        var padding = new string('p', 8000);

        // The reader runs BESIDE the sends, not after them. The pipe's buffer is smaller than
        // this many messages, so a test that queued everything first and only then started
        // reading would block in the write and never reach its own assertions.
        var read = Task.Run(async () =>
        {
            using var reader = new StreamReader(client);
            var lines = new List<string>();
            for (var i = 0; i < Messages; i++)
            {
                var line = await reader.ReadLineAsync();
                if (line is null) break;
                lines.Add(line);
            }

            return lines;
        });

        // Queued without awaiting any of them, which is exactly how ShelfSession sends a shelf.
        var sends = new List<Task>();
        for (var i = 0; i < Messages; i++)
            sends.Add(server.SendAsync(new DropMessage(DropVerb.Items, i, Messages, 0, 0, [padding])));

        await Task.WhenAll(sends);
        Assert.Same(read, await Task.WhenAny(read, Task.Delay(10000)));

        var received = await read;
        Assert.Equal(Messages, received.Count);
        for (var i = 0; i < Messages; i++)
        {
            Assert.True(DropChannel.TryDecode(received[i], out var message), $"Line {i} did not decode.");
            Assert.Equal(DropVerb.Items, message.Verb);
            Assert.Equal(i, (int)message.X);
            Assert.Equal(padding, Assert.Single(message.Paths));
        }
    }

    /// <summary>
    /// A send that fails must not take the channel with it.
    ///
    /// The chain that fixed the interleave introduced this: a faulted task assigned to the chain
    /// is rethrown by the next link's `await previous`, which faults that one, and so on for the
    /// life of the process. Every call site is fire-and-forget, so the symptom would have been a
    /// channel that went permanently and silently mute after one broken write, and the way to
    /// reach it was ordinary - the catcher dying mid-send, which is a thing that happens to a
    /// helper process a person can close from Task Manager.
    ///
    /// Driven the way it really happens rather than by injecting an exception: a client connects
    /// and goes away, a send is attempted into the corpse, and then a new client connects and is
    /// sent to. The assertion is on the SECOND client, because the first send's failure is
    /// expected and uninteresting; what matters is that anything works afterwards.
    /// </summary>
    [Fact]
    public async Task Server_KeepsSendingAfterASendFailed()
    {
        var sid = TestSid();
        using var server = new DropChannelServer(sid);
        server.Start();

        using (var doomed = new NamedPipeClientStream(".", DropChannel.PipeName(sid), PipeDirection.InOut))
        {
            await doomed.ConnectAsync(5000);
            for (var i = 0; i < 100 && !server.IsConnected; i++) await Task.Delay(20);
            Assert.True(server.IsConnected);
        }

        // Into a pipe whose other end has just gone. Awaited rather than discarded, because the
        // point of the test is that this task COMPLETES rather than faulting.
        await server.SendAsync(new DropMessage(DropVerb.Hide, 0, 0, 0, 0, []));

        using var client = new NamedPipeClientStream(".", DropChannel.PipeName(sid), PipeDirection.InOut);
        await client.ConnectAsync(5000);
        for (var i = 0; i < 100 && !server.IsConnected; i++) await Task.Delay(20);
        Assert.True(server.IsConnected);

        // The read is STARTED BEFORE the send is awaited, and getting that backwards deadlocks
        // rather than fails. The pipe is created with both buffer sizes set to zero, so a write
        // does not return until the other end has taken the bytes; awaiting a send that nobody is
        // reading hangs the test host hard enough that the runner reports it as a crash. Measured
        // here the expensive way.
        using var reader = new StreamReader(client);
        var read = reader.ReadLineAsync();

        await server.SendAsync(new DropMessage(DropVerb.OpenShelf, 1, 2, 3, 4, [SomePath]));
        Assert.Same(read, await Task.WhenAny(read, Task.Delay(5000)));

        Assert.True(DropChannel.TryDecode((await read)!, out var message));
        Assert.Equal(DropVerb.OpenShelf, message.Verb);
        Assert.Equal(SomePath, Assert.Single(message.Paths));
    }

    /// <summary>
    /// A catcher that goes away must say so, because nothing can ask.
    ///
    /// PipeStream caches its connection state rather than probing, so IsConnected only turns
    /// false once an operation has failed or the read loop has disconnected. Anything waiting on
    /// the catcher therefore cannot poll for bad news, and the shelf is exactly that: Plith's own
    /// window is hidden for as long as a shelf is up.
    /// </summary>
    [Fact]
    public async Task Server_ReportsAClientThatGoesAway()
    {
        var sid = TestSid();
        using var server = new DropChannelServer(sid);
        var lost = new TaskCompletionSource();
        server.Disconnected += () => lost.TrySetResult();
        server.Start();

        using (var client = new NamedPipeClientStream(".", DropChannel.PipeName(sid), PipeDirection.InOut))
        {
            await client.ConnectAsync(5000);
            for (var i = 0; i < 100 && !server.IsConnected; i++) await Task.Delay(20);
            Assert.True(server.IsConnected);
        }

        Assert.Same(lost.Task, await Task.WhenAny(lost.Task, Task.Delay(5000)));
    }

    private const string SomePath = @"C:\temp\x.txt";
}
