using Plith.Cards;
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
}
