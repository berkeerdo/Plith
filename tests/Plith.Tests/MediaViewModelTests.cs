using Plith.Cards;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Tests;

public class MediaViewModelTests
{
    [Fact]
    public void RequestCommand_RaisesCommandRequestedWithTheSameCommand()
    {
        var vm = new MediaViewModel();
        MediaCommand? seen = null;
        vm.CommandRequested += c => seen = c;

        vm.RequestCommand(MediaCommand.SkipNext);

        Assert.Equal(MediaCommand.SkipNext, seen);
    }

    [Fact]
    public void RequestCommand_WithNoSubscriber_DoesNotThrow()
    {
        var vm = new MediaViewModel();
        vm.RequestCommand(MediaCommand.TogglePlayPause);
    }

    [Fact]
    public void PlayPauseLabel_TracksIsPlaying()
    {
        var vm = new MediaViewModel();

        vm.IsPlaying = false;
        Assert.Equal("Play", vm.PlayPauseLabel);

        vm.IsPlaying = true;
        Assert.Equal("Pause", vm.PlayPauseLabel);
    }

    [Fact]
    public void Timeline_RaisesPropertyChangedOnceForItsOwnName()
    {
        var vm = new MediaViewModel();
        var seen = new List<string?>();
        vm.PropertyChanged += (_, e) => seen.Add(e.PropertyName);

        vm.Timeline = new MediaTimeline(TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(200),
                                        DateTimeOffset.UnixEpoch);

        // Exactly one, and named: the media page routes on the name so that a position arriving
        // once a second does not repaint the artwork and re-measure the marquee.
        Assert.Single(seen);
        Assert.Equal("Timeline", seen[0]);
    }

    [Fact]
    public void Timeline_SetToAnEqualValue_RaisesNothing()
    {
        var timeline = new MediaTimeline(TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(200),
                                         DateTimeOffset.UnixEpoch);
        var vm = new MediaViewModel { Timeline = timeline };
        var raised = 0;
        vm.PropertyChanged += (_, _) => raised++;

        vm.Timeline = new MediaTimeline(TimeSpan.FromSeconds(10), TimeSpan.FromSeconds(200),
                                        DateTimeOffset.UnixEpoch);

        Assert.Equal(0, raised);
    }

    [Fact]
    public void Apply_CarriesTheSnapshotsTimeline()
    {
        var vm = new MediaViewModel();
        var timeline = new MediaTimeline(TimeSpan.FromSeconds(30), TimeSpan.FromSeconds(180),
                                         DateTimeOffset.UnixEpoch);

        vm.Apply(new MediaSnapshot("t", "a", null, IsPlaying: true, HasSession: true, timeline));

        Assert.Equal(timeline, vm.Timeline);
    }

    [Fact]
    public void Apply_WithNoSession_ClearsTheTimeline()
    {
        var vm = new MediaViewModel
        {
            Timeline = new MediaTimeline(TimeSpan.FromSeconds(30), TimeSpan.FromSeconds(180),
                                         DateTimeOffset.UnixEpoch),
        };

        vm.Apply(new MediaSnapshot("", "", null, IsPlaying: false, HasSession: false));

        Assert.Null(vm.Timeline);
    }
}
