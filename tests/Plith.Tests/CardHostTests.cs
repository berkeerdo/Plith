using System.Collections.Generic;
using System.IO;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

/// <summary>Minimal ICard stand-in so CardHost can be tested without any real card.</summary>
internal sealed class FakeCard : ICard
{
    public FakeCard(string id, int order, bool visible = true)
    {
        Id = id;
        Order = order;
        _isVisible = visible;
    }

    public string Id { get; }
    public int Order { get; }
    public object ViewModel { get; } = new object();

    private bool _isVisible;
    public bool IsVisible
    {
        get => _isVisible;
        set { _isVisible = value; VisibilityChanged?.Invoke(); }
    }

    public int ActivateCount { get; private set; }
    public int DeactivateCount { get; private set; }
    public int ThemeChangedCount { get; private set; }

    public event Action? VisibilityChanged;
    public event Action<ShowRequest>? ShowRequested;

    public void Activate() => ActivateCount++;
    public void Deactivate() => DeactivateCount++;
    public void OnThemeChanged() => ThemeChangedCount++;

    public void RaiseShow(ShowRequest request) => ShowRequested?.Invoke(request);
}

internal sealed class FakeSuppressor : IShowSuppressor
{
    private bool _suppressed;
    public bool IsSuppressed
    {
        get => _suppressed;
        set { _suppressed = value; SuppressionChanged?.Invoke(value); }
    }

    public event Action<bool>? SuppressionChanged;
}

public class CardHostTests
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

    [Fact]
    public void Cards_AreExposedSortedByOrder()
    {
        var host = new CardHost(NewSettings());
        host.Register(new FakeCard("audio", 20));
        host.Register(new FakeCard("media", 10));

        Assert.Equal(new[] { "media", "audio" }, host.Cards.Select(c => c.Id));
    }

    [Fact]
    public void Register_SameCardTwice_Throws()
    {
        var host = new CardHost(NewSettings());
        var media = new FakeCard("media", 10);
        host.Register(media);

        Assert.Throws<InvalidOperationException>(() => host.Register(media));
    }

    [Fact]
    public void VisibleCards_ExcludesInvisibleCard()
    {
        var host = new CardHost(NewSettings());
        host.Register(new FakeCard("media", 10, visible: false));
        host.Register(new FakeCard("audio", 20));

        Assert.Equal(new[] { "audio" }, host.VisibleCards.Select(c => c.Id));
    }

    [Fact]
    public void VisibilityChanged_UpdatesVisibleCardsWithoutRaisingShow()
    {
        var host = new CardHost(NewSettings());
        var media = new FakeCard("media", 10, visible: false);
        host.Register(media);
        host.Register(new FakeCard("audio", 20));

        int shows = 0;
        host.ShowRequested += _ => shows++;

        media.IsVisible = true;

        Assert.Equal(new[] { "media", "audio" }, host.VisibleCards.Select(c => c.Id));
        Assert.Equal(0, shows);
    }

    [Fact]
    public void CardShowRequest_IsReEmittedWithSettingsDuration()
    {
        var host = new CardHost(NewSettings(showDurationMs: 3500));
        var audio = new FakeCard("audio", 20);
        host.Register(audio);

        TimeSpan? seen = null;
        host.ShowRequested += d => seen = d;

        audio.RaiseShow(new ShowRequest(ShowReason.AudioChange, "audio"));

        Assert.Equal(TimeSpan.FromMilliseconds(3500), seen);
    }

    [Fact]
    public void DurationOverride_WinsOverSettingsValue()
    {
        var host = new CardHost(NewSettings(showDurationMs: 2000));

        TimeSpan? seen = null;
        host.ShowRequested += d => seen = d;

        host.RequestShow(new ShowRequest(ShowReason.EditModeExit, null, TimeSpan.FromMilliseconds(1500)));

        Assert.Equal(TimeSpan.FromMilliseconds(1500), seen);
    }

    [Fact]
    public void SuppressedHost_SwallowsShowRequest()
    {
        var suppressor = new FakeSuppressor { IsSuppressed = true };
        var host = new CardHost(NewSettings(), suppressor);

        int shows = 0;
        host.ShowRequested += _ => shows++;

        host.RequestShow(new ShowRequest(ShowReason.SummonHotkey));

        Assert.Equal(0, shows);
    }

    [Fact]
    public void SuppressionTurningOn_RaisesHideRequested()
    {
        var suppressor = new FakeSuppressor();
        var host = new CardHost(NewSettings(), suppressor);

        int hides = 0;
        host.HideRequested += () => hides++;

        suppressor.IsSuppressed = true;

        Assert.Equal(1, hides);
    }

    [Fact]
    public void SuppressionTurningOff_DoesNotRaiseHideRequested()
    {
        var suppressor = new FakeSuppressor { IsSuppressed = true };
        var host = new CardHost(NewSettings(), suppressor);

        int hides = 0;
        host.HideRequested += () => hides++;

        suppressor.IsSuppressed = false;

        Assert.Equal(0, hides);
    }

    [Fact]
    public void Start_ActivatesEveryCard_DisposeDeactivatesEveryCard()
    {
        var host = new CardHost(NewSettings());
        var a = new FakeCard("audio", 20);
        var b = new FakeCard("media", 10);
        host.Register(a);
        host.Register(b);

        host.Start();
        Assert.Equal(1, a.ActivateCount);
        Assert.Equal(1, b.ActivateCount);

        host.Dispose();
        Assert.Equal(1, a.DeactivateCount);
        Assert.Equal(1, b.DeactivateCount);
    }

    [Fact]
    public void NotifyThemeChanged_ReachesInvisibleCardsToo()
    {
        var host = new CardHost(NewSettings());
        var hidden = new FakeCard("media", 10, visible: false);
        host.Register(hidden);

        host.NotifyThemeChanged();

        Assert.Equal(1, hidden.ThemeChangedCount);
    }

    [Fact]
    public void MiddleCardGoingInvisible_CollapsesVisibleCardsInOrder()
    {
        var host = new CardHost(NewSettings());
        var first = new FakeCard("first", 10);
        var middle = new FakeCard("middle", 20);
        var last = new FakeCard("last", 30);
        host.Register(first);
        host.Register(middle);
        host.Register(last);

        middle.IsVisible = false;

        Assert.Equal(new[] { "first", "last" }, host.VisibleCards.Select(c => c.Id));
    }

    [Fact]
    public void MiddleCardComingBackVisible_ReinsertsAtItsOrderPosition()
    {
        var host = new CardHost(NewSettings());
        var first = new FakeCard("first", 10);
        var middle = new FakeCard("middle", 20);
        var last = new FakeCard("last", 30);
        host.Register(first);
        host.Register(middle);
        host.Register(last);

        middle.IsVisible = false;
        middle.IsVisible = true;

        Assert.Equal(new[] { "first", "middle", "last" }, host.VisibleCards.Select(c => c.Id));
    }

    [Fact]
    public void LastCardGoingInvisible_ShrinksVisibleCards()
    {
        var host = new CardHost(NewSettings());
        var first = new FakeCard("first", 10);
        var middle = new FakeCard("middle", 20);
        var last = new FakeCard("last", 30);
        host.Register(first);
        host.Register(middle);
        host.Register(last);

        last.IsVisible = false;

        Assert.Equal(new[] { "first", "middle" }, host.VisibleCards.Select(c => c.Id));
        Assert.Equal(2, host.VisibleCards.Count);
    }
}
