using System.IO;
using System.IO.Pipes;
using System.Windows;
using System.Windows.Media;
using Plith.Services;
using Plith.Services.Shelf;
using Plith.Views.Presentation;

namespace Plith.Tests;

/// <summary>
/// What the hand-over can be measured on without a screen.
///
/// The shelf itself cannot be: it is a layered window in a second process, and over Remote
/// Desktop nothing can capture one. So these cover the half that is arithmetic and routing, and
/// docs/SHELF-VERIFICATION.md carries the half that needs a person at a console.
/// </summary>
public sealed class ShelfSessionTests : IDisposable
{
    private readonly string _directory;
    private readonly string _storePath;
    private readonly DropChannelServer _channel;

    public ShelfSessionTests()
    {
        _directory = Path.Combine(Path.GetTempPath(), "plith-session-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_directory);
        _storePath = Path.Combine(_directory, "shelf.txt");

        // Never started, so nothing is ever connected to it. That is the state every test here
        // wants: HandleMessage must work with no catcher on the far end (it only touches the
        // store), and Open must refuse with a sentence.
        _channel = new DropChannelServer("S-1-5-21-test-" + Guid.NewGuid().ToString("N"));
    }

    public void Dispose()
    {
        _channel.Dispose();
        try { Directory.Delete(_directory, recursive: true); }
        catch (IOException) { }
    }

    private string MakeFile(string name)
    {
        var path = Path.Combine(_directory, name);
        File.WriteAllText(path, "x");
        return path;
    }

    private (ShelfSession Session, ShelfStore Store) Build()
    {
        var store = new ShelfStore(_storePath);
        var session = new ShelfSession(_channel, store, () => AnyPalette);
        return (session, store);
    }

    private static readonly ShelfPalette AnyPalette = new(
        Colors.Black, Colors.Black, Colors.White, Colors.Gray, Colors.DimGray,
        Colors.Lime, Colors.Lime, IsDark: true);

    /// <summary>
    /// A click that does nothing is indistinguishable from the product being broken, so the one
    /// thing Open must never do with no catcher on the wire is fail quietly.
    ///
    /// The sentence itself is not asserted, deliberately: which of the four CatcherStart values
    /// happens depends on whether a catcher happens to be running on the machine the suite runs
    /// on, and a test that pinned the wording would pass or fail on that rather than on this.
    /// </summary>
    [Fact]
    public void Open_WithNoCatcher_SaysSoRatherThanNothing()
    {
        var (session, _) = Build();

        string? said = null;
        var opened = false;
        session.Unavailable += why => said = why;
        session.Opened += () => opened = true;

        session.Open(new Rect(0, 0, 190, 6), dpiScale: 1.0);

        Assert.False(opened);
        Assert.False(string.IsNullOrWhiteSpace(said));
    }

    /// <summary>
    /// The catcher dying while the shelf is up must put the notch back.
    ///
    /// This is not a missing shelf, it is a missing OSD: Plith's whole window is hidden for the
    /// duration of a shelf, no ShelfClosed can arrive from a process that is gone, and the hide
    /// timer cannot re-show a window that was hidden with SWP_HIDEWINDOW. Without a second cause
    /// for Closed the volume keys show nothing until Plith restarts.
    ///
    /// A real connected pipe, because Open refuses to report itself open without one, and that
    /// refusal is half of the answer to the same hazard.
    /// </summary>
    [Fact]
    public async Task ChannelLost_WithAShelfOpen_PutsTheNotchBack()
    {
        var sid = "S-1-5-21-test-" + Guid.NewGuid().ToString("N");
        using var server = new DropChannelServer(sid);
        server.Start();

        using var client = new NamedPipeClientStream(".", DropChannel.PipeName(sid), PipeDirection.InOut);
        await client.ConnectAsync(5000);
        for (var i = 0; i < 100 && !server.IsConnected; i++) await Task.Delay(20);
        Assert.True(server.IsConnected);

        var session = new ShelfSession(server, new ShelfStore(_storePath), () => AnyPalette);
        var opened = false;
        var closed = false;
        session.Opened += () => opened = true;
        session.Closed += () => closed = true;

        session.Open(new Rect(0, 0, 190, 6), dpiScale: 1.0);
        Assert.True(opened);
        Assert.False(closed);

        session.OnChannelLost();
        Assert.True(closed);
    }

    /// <summary>A channel loss with no shelf on screen reports nothing. The notch is already up,
    /// and a Closed for a shelf nobody opened would be a second restore of a window that was
    /// never hidden.</summary>
    [Fact]
    public void ChannelLost_WithNoShelfOpen_ReportsNothing()
    {
        var (session, _) = Build();

        var closed = false;
        session.Closed += () => closed = true;

        session.OnChannelLost();

        Assert.False(closed);
    }

    /// <summary>
    /// Every verb the catcher can send, routed to the store method that answers it. The store is
    /// the authority on all four; this only checks that the routing reaches it.
    /// </summary>
    [Fact]
    public void HandleMessage_RoutesTheShelfChangingVerbs()
    {
        var (session, store) = Build();
        var a = MakeFile("a.txt");
        var b = MakeFile("b.txt");
        store.Add([a, b]);

        session.HandleMessage(new DropMessage(DropVerb.RemoveItems, 0, 0, 0, 0, [a]));
        Assert.Equal([b], store.Items.Select(i => i.Path));

        session.HandleMessage(new DropMessage(DropVerb.NewStack, 0, 0, 0, 0, []));
        Assert.Equal(2, store.Stacks.Count);

        // Index 0 is the empty stack NewStack just put at the front, so this moves b into it.
        session.HandleMessage(new DropMessage(DropVerb.Restack, 0, 0, 0, 0, [b]));
        Assert.Equal([b], store.Stacks[0].Select(i => i.Path));

        session.HandleMessage(new DropMessage(DropVerb.ClearShelf, 0, 0, 0, 0, []));
        Assert.Empty(store.Items);
    }

    /// <summary>
    /// ShelfClosed prunes and then reports. The order matters: the notch comes back on the
    /// Closed event, and an empty stack surviving into the next open would be a column nobody
    /// asked for in a surface that was just rebuilt from scratch.
    /// </summary>
    [Fact]
    public void HandleMessage_ShelfClosed_PrunesThenReports()
    {
        var (session, store) = Build();
        store.Add([MakeFile("a.txt")]);
        store.NewStack();
        Assert.Equal(2, store.Stacks.Count);

        var stacksWhenClosed = -1;
        session.Closed += () => stacksWhenClosed = store.Stacks.Count;

        session.HandleMessage(new DropMessage(DropVerb.ShelfClosed, 0, 0, 0, 0, []));

        Assert.Equal(1, stacksWhenClosed);
    }

    /// <summary>
    /// A verb Plith sends rather than receives must do nothing when it arrives the other way.
    /// The pipe's ACL is open to every process on the machine, so this is reachable by something
    /// other than a bug.
    /// </summary>
    [Fact]
    public void HandleMessage_IgnoresTheVerbsPlithItselfSends()
    {
        var (session, store) = Build();
        store.Add([MakeFile("a.txt")]);

        foreach (var verb in new[] { DropVerb.Hello, DropVerb.Show, DropVerb.Hide, DropVerb.Dropped,
                                     DropVerb.OpenShelf, DropVerb.Items, DropVerb.Palette })
        {
            session.HandleMessage(new DropMessage(verb, 0, 0, 0, 0, [MakeFile("b.txt")]));
        }

        Assert.Single(store.Items);
    }

    /// <summary>
    /// The selection ring is DERIVED, never the raw accent.
    ///
    /// A near-white accent on the dark theme is the case that proves it: sent raw it measured
    /// 1.25:1 against the surface, which is a ring nobody can see. RingOn walks its lightness
    /// keeping hue and saturation, so what crosses the wire clears the 3:1 a non-text stroke
    /// needs.
    /// </summary>
    [Fact]
    public void DerivePalette_SendsARingThatCanBeSeen()
    {
        var palette = ShelfSession.DerivePalette(Color.FromRgb(0xFA, 0xFA, 0xFA), isDark: true);

        Assert.True(ContrastInk.ContrastRatio(palette.SelectionRing, palette.SurfaceEnd) >= 3.0);
        Assert.NotEqual(palette.Accent, palette.SelectionRing);
    }

    /// <summary>An accent that already clears the threshold is passed through untouched, so the
    /// ring still reads as the colour the person picked.</summary>
    [Fact]
    public void DerivePalette_LeavesAnAccentThatAlreadyClearsTheThreshold()
    {
        var palette = ShelfSession.DerivePalette(Color.FromRgb(0x4A, 0xD6, 0x95), isDark: true);

        Assert.Equal(palette.Accent, palette.SelectionRing);
    }

    /// <summary>
    /// The shelf grows out of the open frame, so the two rectangles must share a centre. A shift
    /// of even a few DIP reads as the shape sliding sideways while it opens rather than as the
    /// notch continuing into something larger.
    /// </summary>
    [Fact]
    public void ShelfRect_SharesTheOpenFramesCentreAndTop()
    {
        var hover = new Rect(865, 0, 190, 6);

        var frame = NotchGeometry.DropTargetRect(hover);
        var shelf = NotchGeometry.ShelfRect(hover);

        Assert.Equal(frame.Left + (frame.Width / 2), shelf.Left + (shelf.Width / 2), 6);
        Assert.Equal(frame.Top, shelf.Top);
        Assert.Equal(NotchGeometry.ShelfFrameDip.Width, shelf.Width);
        Assert.Equal(NotchGeometry.ShelfFrameDip.Height, shelf.Height);
    }
}
