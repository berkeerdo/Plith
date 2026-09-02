using System.IO;
using Plith.Cards;
using Plith.Services;

namespace Plith.Tests;

public class MediaCardTests
{
    private static SettingsService NewSettings(bool autoShowOnMedia = true, bool compactMode = false)
    {
        var path = Path.Combine(Path.GetTempPath(), "PlithTests", Guid.NewGuid().ToString("N"), "config.ini");
        var svc = new SettingsService(path);
        var m = svc.Current.Clone();
        m.AutoShowOnMedia = autoShowOnMedia;
        m.CompactMode = compactMode;
        svc.Save(m);
        return svc;
    }

    private static MediaSnapshot Playing(string title = "Sample track")
        => new(title, "Sample artist", null, IsPlaying: true, HasSession: true);

    private static MediaSnapshot NoSession()
        => new("", "", null, IsPlaying: false, HasSession: false);

    [Fact]
    public void Apply_WithAutoShowOn_RaisesShowRequested()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing());

        Assert.Single(shows);
        Assert.Equal(ShowReason.MediaChange, shows[0].Reason);
        Assert.Equal("media", shows[0].OriginCardId);
    }

    [Fact]
    public void Apply_WithAutoShowOff_RaisesNothing()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: false));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing());

        Assert.Empty(shows);
    }

    [Fact]
    public void Apply_WithNoSession_RaisesNothingEvenWithAutoShowOn()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(NoSession());

        Assert.Empty(shows);
    }

    [Fact]
    public void IsVisible_IsFalseWithoutASession()
    {
        var card = new MediaCard(NewSettings());
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void IsVisible_IsTrueWithASession()
    {
        var card = new MediaCard(NewSettings());
        card.Apply(Playing());
        Assert.True(card.IsVisible);
    }

    [Fact]
    public void IsVisible_IsFalseInCompactModeDespiteASession()
    {
        var card = new MediaCard(NewSettings(compactMode: true));
        card.Apply(Playing());
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void SessionAppearing_RaisesVisibilityChanged()
    {
        var card = new MediaCard(NewSettings());
        card.Activate();
        int changes = 0;
        card.VisibilityChanged += () => changes++;

        card.Apply(Playing());

        Assert.True(changes > 0);
    }

    [Fact]
    public void CompactModeToggle_RaisesVisibilityChanged()
    {
        var settings = NewSettings(compactMode: false);
        var card = new MediaCard(settings);
        card.Activate();
        card.Apply(Playing());

        int changes = 0;
        card.VisibilityChanged += () => changes++;

        var m = settings.Current.Clone();
        m.CompactMode = true;
        settings.Save(m);

        Assert.True(changes > 0);
        Assert.False(card.IsVisible);
    }

    [Fact]
    public void ViewModelCommandRequest_SurfacesAsCommandInvoked()
    {
        var card = new MediaCard(NewSettings());
        MediaCommand? seen = null;
        card.CommandInvoked += (_, c) => seen = c;

        card.Vm.RequestCommand(MediaCommand.SkipNext);

        Assert.Equal(MediaCommand.SkipNext, seen);
    }
}
