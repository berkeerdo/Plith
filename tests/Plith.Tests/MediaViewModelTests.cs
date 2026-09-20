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

    [Fact]
    public void Apply_CarriesCanSeek()
    {
        var vm = new MediaViewModel();

        vm.Apply(new MediaSnapshot("t", "a", null, IsPlaying: true, HasSession: true,
                                   Timeline: null, CanSeek: true));

        Assert.True(vm.CanSeek);
    }

    [Fact]
    public void Apply_WithASourceThatRefusesSeek_LeavesCanSeekFalse()
    {
        // The default, and it matters which way round the default falls: a bar that offers a
        // drag the source ignores is worse than one that offers none.
        var vm = new MediaViewModel();

        vm.Apply(new MediaSnapshot("t", "a", null, IsPlaying: true, HasSession: true));

        Assert.False(vm.CanSeek);
    }

    [Fact]
    public void RequestSeek_RaisesSeekRequestedWithThePosition()
    {
        var vm = new MediaViewModel();
        TimeSpan? seen = null;
        vm.SeekRequested += p => seen = p;

        vm.RequestSeek(TimeSpan.FromSeconds(97));

        Assert.Equal(TimeSpan.FromSeconds(97), seen);
    }

    [Fact]
    public void RequestSeek_WithNoSubscriber_DoesNotThrow()
    {
        new MediaViewModel().RequestSeek(TimeSpan.FromSeconds(1));
    }
}
