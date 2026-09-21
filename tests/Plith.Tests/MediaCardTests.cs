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

    private static MediaSnapshot Paused(string title = "Sample track")
        => new(title, "Sample artist", null, IsPlaying: false, HasSession: true);

    private static MediaSnapshot NoSession()
        => new("", "", null, IsPlaying: false, HasSession: false);

    [Fact]
    public void Apply_WithAutoShowOn_RaisesShowRequestedOnTheTrackChange()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        // Two snapshots, because one cannot be a change. The first is what the session already
        // held when Plith looked at it.
        card.Apply(Playing("First track"));
        card.Apply(Playing("Second track"));

        Assert.Single(shows);
        Assert.Equal(ShowReason.MediaChange, shows[0].Reason);
        Assert.Equal("media", shows[0].OriginCardId);
    }

    // The setting is called "Show on track change", and until these tests it meant "show on any
    // SMTC event at all". The card compared nothing and every snapshot became a show. Each test
    // below is a gesture that produced an OSD it had no business producing.

    [Fact]
    public void TheFirstSnapshotIsABaseline_AndShowsNothing()
    {
        // Plith reads the session once at startup. The person did nothing, so nothing pops.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("Already playing when Plith started"));

        Assert.Empty(shows);
    }

    [Fact]
    public void SeekingWithinTheSameTrack_ShowsNothing()
    {
        // Seeking in a YouTube or Netflix tab raises a session event with the title unchanged.
        // Content is what is compared, so it does not matter which event it was.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("The same video"));
        card.Apply(Playing("The same video"));

        Assert.Empty(shows);
    }

    [Fact]
    public void PausingTheCurrentTrack_ShowsNothing()
    {
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("The same video"));
        card.Apply(Paused("The same video"));

        Assert.Empty(shows);
    }

    [Fact]
    public void ASessionThatIsNotPlaying_ShowsNothingEvenWithADifferentTitle()
    {
        // Pausing a video hands the current session to whatever else holds one, a Spotify that
        // is sitting paused. Its track is not the one that was showing, but nobody played it.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("A video"));
        card.Apply(Paused("A paused song in another player"));

        Assert.Empty(shows);
    }

    [Fact]
    public void ResumingATrackThatArrivedPaused_ShowsNothing()
    {
        // The baseline follows every snapshot, playing or not. Without that, pressing play on
        // the session that just arrived paused would read as a change from the OLD title and
        // pop the OSD for a play/pause.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("A video"));
        card.Apply(Paused("A paused song in another player"));
        card.Apply(Playing("A paused song in another player"));

        Assert.Empty(shows);
    }

    [Fact]
    public void ReturningToASessionAfterASwap_ShowsNothing()
    {
        // Pausing a video hands the session to a paused Spotify; playing it again hands it back.
        // Comparing titles alone, that return looks exactly like a new track. It is not one, and
        // the session swap is the only thing that says so.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("A video"));
        card.NoteSessionReplaced();
        card.Apply(Paused("A paused song in another player"));
        card.NoteSessionReplaced();
        card.Apply(Playing("A video"));

        Assert.Empty(shows);
    }

    [Fact]
    public void ASwapSuppressesOnlyItsOwnSnapshot_NotTheSessionAfterIt()
    {
        // The suppression covers the arrival of the new session, which nobody asked for. A track
        // change inside that session afterwards is a real one.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("A video"));
        card.NoteSessionReplaced();
        card.Apply(Playing("The song that was already queued"));
        card.Apply(Playing("The next song"));

        Assert.Single(shows);
    }

    [Fact]
    public void AnEmptyTitle_IsNotATrack_AndNeitherShowsNorDisturbsTheBaseline()
    {
        // Sources fill their properties progressively, so a transition can pass through a
        // momentarily empty title. Treating that as a track would show twice for one change.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("First track"));
        card.Apply(Playing(""));
        card.Apply(Playing("First track"));

        Assert.Empty(shows);
    }

    [Fact]
    public void TheBaselineIsKeptWhileTheSettingIsOff_SoTurningItOnShowsNothingByItself()
    {
        var settings = NewSettings(autoShowOnMedia: false);
        var card = new MediaCard(settings);
        card.Activate();
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Apply(Playing("First track"));

        var m = settings.Current.Clone();
        m.AutoShowOnMedia = true;
        settings.Save(m);

        card.Apply(Playing("First track"));

        Assert.Empty(shows);
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

    // See the matching test in AudioCardTests: ToString feeds WPF's ItemAutomationPeer, so it is
    // load-bearing for screen readers rather than a debugging convenience.
    [Fact]
    public void AccessibleName_IsHumanReadable_AndDrivesToString()
    {
        var card = new MediaCard(NewSettings());
        Assert.Equal("Now playing", card.AccessibleName);
        Assert.Equal(card.AccessibleName, card.ToString());
        Assert.DoesNotContain("Plith.Cards", card.ToString());
    }

    [Fact]
    public void ApplyTimeline_ReachesTheViewModel()
    {
        var card = new MediaCard(NewSettings());
        var timeline = new MediaTimeline(TimeSpan.FromSeconds(12), TimeSpan.FromSeconds(200),
                                         DateTimeOffset.UnixEpoch);

        card.ApplyTimeline(timeline);

        Assert.Equal(timeline, card.Vm.Timeline);
    }

    [Fact]
    public void ApplyTimeline_RaisesNoShowRequest()
    {
        // This test is the reason ApplyTimeline exists at all. Apply() raises ShowRequested when
        // AutoShowOnMedia is on, and TimelinePropertiesChanged fires about once a second on some
        // sources: routing the position through Apply would summon the OSD every second.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        card.Apply(Playing());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.ApplyTimeline(new MediaTimeline(TimeSpan.FromSeconds(12), TimeSpan.FromSeconds(200),
                                             DateTimeOffset.UnixEpoch));

        Assert.Empty(shows);
    }

    [Fact]
    public void Seek_ReachesTheCardAsAPosition()
    {
        var card = new MediaCard(NewSettings());
        TimeSpan? seen = null;
        card.SeekInvoked += (_, p) => seen = p;

        card.Vm.RequestSeek(TimeSpan.FromSeconds(42));

        Assert.Equal(TimeSpan.FromSeconds(42), seen);
    }

    [Fact]
    public void Seek_RaisesNoShowRequest()
    {
        // A transport command asks for a show; a seek must not. It happens on a surface the
        // person is already looking at and already holding open, so a show would replace the
        // page under their own hand with a HUD about it.
        var card = new MediaCard(NewSettings(autoShowOnMedia: true));
        card.Apply(Playing());
        var shows = new List<ShowRequest>();
        card.ShowRequested += r => shows.Add(r);

        card.Vm.RequestSeek(TimeSpan.FromSeconds(42));

        Assert.Empty(shows);
    }
}
