using Plith.Services;

namespace Plith.Tests;

public class NotchHomeStateTests
{
    [Fact]
    public void StartsClosed()
    {
        Assert.False(new NotchHomeState().IsOpen);
    }

    [Fact]
    public void OpenRaisesChangedOnce()
    {
        var s = new NotchHomeState();
        int raised = 0;
        s.Changed += () => raised++;

        s.Open();

        Assert.True(s.IsOpen);
        Assert.Equal(1, raised);
    }

    [Fact]
    public void OpenIsIdempotent()
    {
        // OsdHost calls Open() on every hover-in, and a cursor that leaves and re-enters the
        // resting rectangle while the panel is still open produces several in a row. Each one
        // raising Changed would re-run CardHost.RecomputeVisibleCards against an unchanged
        // card set, which reconciles an ObservableCollection bound to a live ItemsControl.
        var s = new NotchHomeState();
        s.Open();
        int raised = 0;
        s.Changed += () => raised++;

        s.Open();

        Assert.True(s.IsOpen);
        Assert.Equal(0, raised);
    }

    [Fact]
    public void CloseRaisesChangedOnceAndIsIdempotent()
    {
        var s = new NotchHomeState();
        s.Open();
        int raised = 0;
        s.Changed += () => raised++;

        s.Close();
        s.Close();

        Assert.False(s.IsOpen);
        Assert.Equal(1, raised);
    }

    [Fact]
    public void CloseOnAFreshStateRaisesNothing()
    {
        // Park() runs during startup, before any hover has happened.
        var s = new NotchHomeState();
        int raised = 0;
        s.Changed += () => raised++;

        s.Close();

        Assert.Equal(0, raised);
    }
}
