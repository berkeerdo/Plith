using Plith.Views.Presentation;

namespace Plith.Tests;

public class NotchOpeningPolicyTests
{
    [Fact]
    public void PlayingOpensOnTheMediaPage()
    {
        Assert.Equal(2, NotchOpeningPolicy.OpeningPage(isPlaying: true, mediaPageIndex: 2));
    }

    [Fact]
    public void NotPlayingOpensOnTheFirstPage()
    {
        // A paused session is not what is happening now. This is the decision the spec argues:
        // a Spotify left open and paused for days would otherwise lock the notch onto the media
        // page and the clock would never come first again.
        Assert.Equal(0, NotchOpeningPolicy.OpeningPage(isPlaying: false, mediaPageIndex: 2));
    }

    [Fact]
    public void PlayingWithNoMediaPageInstalledOpensOnTheFirstPage()
    {
        // The page list is rebuilt as pages come and go, and the index is -1 while there is no
        // media page in it. Opening on -1 would throw or clamp to a page nobody asked for.
        Assert.Equal(0, NotchOpeningPolicy.OpeningPage(isPlaying: true, mediaPageIndex: -1));
    }

    [Fact]
    public void TheMediaPageBeingFirstIsNotASpecialCase()
    {
        Assert.Equal(0, NotchOpeningPolicy.OpeningPage(isPlaying: true, mediaPageIndex: 0));
    }
}
