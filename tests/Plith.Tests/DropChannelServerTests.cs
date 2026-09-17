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

    private const string SomePath = @"C:\temp\x.txt";
}
