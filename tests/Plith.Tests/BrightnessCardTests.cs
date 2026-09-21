using System.IO;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

public class BrightnessCardTests
{
    private static SettingsService NewSettings(int showDurationMs = 2000)
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);
        var m = svc.Current.Clone();
        m.ShowDurationMs = showDurationMs;
        svc.Save(m);
        return svc;
    }

    /// <summary>Captures the hide callback instead of waiting for a timer, so the test decides
    /// when the window closes.</summary>
    private sealed class ManualHide
    {
        public TimeSpan? After { get; private set; }
        public Action? Callback { get; private set; }
        public int Scheduled { get; private set; }

        public void Schedule(TimeSpan after, Action callback)
        {
            After = after;
            Callback = callback;
            Scheduled++;
        }

        public void Fire() => Callback?.Invoke();
    }

    [Fact]
    public void ACardWithNothingReportedIsInvisible()
    {
        var card = new BrightnessCard(NewSettings());
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void ReportingAChangeMakesItVisibleAndAsksForTheOsd()
    {
        var card = new BrightnessCard(NewSettings());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Report(40);

        Assert.True(card.IsVisible);
        Assert.Single(shows);
        Assert.Equal(ShowReason.BrightnessChange, shows[0].Reason);
        Assert.Equal("brightness", shows[0].OriginCardId);
    }

    [Fact]
    public void TheCardGoesAwayOnItsOwnAfterTheShowDuration()
    {
        // The Audio card is always visible, because the OSD has no state in which it says
        // nothing about audio. Brightness is not like that: a permanent row would appear on
        // every volume press for every user, including laptop users who never asked for one.
        var hide = new ManualHide();
        var card = new BrightnessCard(NewSettings(showDurationMs: 2000), hide.Schedule);
        var visibilityChanges = 0;
        card.VisibilityChanged += () => visibilityChanges++;

        card.Report(40);
        Assert.True(card.IsVisible);
        Assert.Equal(TimeSpan.FromMilliseconds(2000), hide.After);

        hide.Fire();

        Assert.False(card.IsVisible);
        Assert.Equal(2, visibilityChanges);
    }

    [Fact]
    public void ASecondChangeRestartsTheWindowRatherThanStackingTimers()
    {
        var hide = new ManualHide();
        var card = new BrightnessCard(NewSettings(), hide.Schedule);

        card.Report(40);
        card.Report(50);

        Assert.Equal(2, hide.Scheduled);
        Assert.True(card.IsVisible);
    }

    [Fact]
    public void TheReportedValueReachesTheViewModel()
    {
        var card = new BrightnessCard(NewSettings());
        card.Report(65);
        Assert.Equal(65, card.Vm.Percent);
    }

    [Fact]
    public void TheHideWindowFollowsTheCurrentSetting()
    {
        var hide = new ManualHide();
        var card = new BrightnessCard(NewSettings(showDurationMs: 3500), hide.Schedule);

        card.Report(40);

        Assert.Equal(TimeSpan.FromMilliseconds(3500), hide.After);
    }

    [Fact]
    public void AccessibleName_IsHumanReadable_AndDrivesToString()
    {
        // WPF's ItemAutomationPeer names the OSD's list container from ToString. See
        // ICard.AccessibleName for the measurement behind this.
        var card = new BrightnessCard(NewSettings());
        Assert.Equal("Brightness", card.AccessibleName);
        Assert.Equal(card.AccessibleName, card.ToString());
        Assert.DoesNotContain("Plith.Cards", card.ToString());
    }

    [Fact]
    public void TheCardSitsBelowAudioInTheStack()
    {
        Assert.Equal(30, new BrightnessCard(NewSettings()).Order);
    }

    [Fact]
    public void TheShowRequestIsRaisedBeforeTheVisibilityChange()
    {
        // Both events make CardHost recompute, and only the request carries the exclusivity.
        // The other order pushes a set containing every card into the bound collection first,
        // which is one frame of the volume bar before the brightness bar replaces it. It was
        // visible in the log as "On screen: media, audio, brightness" followed immediately by
        // "On screen: brightness (exclusive)".
        var card = new BrightnessCard(NewSettings());
        var order = new List<string>();
        card.VisibilityChanged += () => order.Add("visibility");
        card.ShowRequested += _ => order.Add("show");

        card.Report(40);

        Assert.Equal(["show", "visibility"], order);
    }

    [Fact]
    public void TheShowRequestAsksForTheSurfaceToItself()
    {
        var card = new BrightnessCard(NewSettings());
        ShowRequest? seen = null;
        card.ShowRequested += r => seen = r;

        card.Report(40);

        Assert.True(seen!.Exclusive);
    }

}
