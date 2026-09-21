namespace Plith.Views.Presentation;

/// <summary>
/// Which widget page the notch opens on.
///
/// A class of its own, free of WPF, for the reason NotchEventPolicy is: OsdHost is a BandWindow
/// the test project cannot construct, so a rule that lives inside it has no test, and two
/// previous versions of the event rule reached a running build with nothing between them.
///
/// It also keeps the carousel spec's property that the opening page is COMPUTED at open time and
/// never stored. Remembering the last page is deliberately deferred, and a value written in a
/// completion handler is the exact hazard that produced five defects on this branch.
/// </summary>
public static class NotchOpeningPolicy
{
    /// <summary>
    /// The page to open on.
    ///
    /// Playing rather than merely having a session: the opening page should be what is happening
    /// now, and a paused session is not that. A Spotify left open and paused for days would
    /// otherwise lock the notch onto the media page and the clock would never come first again.
    /// </summary>
    /// <param name="isPlaying">Whether the current media session reports Playing.</param>
    /// <param name="mediaPageIndex">Where the media page sits in the current page list, or -1
    /// when it is not installed.</param>
    public static int OpeningPage(bool isPlaying, int mediaPageIndex)
        => isPlaying && mediaPageIndex >= 0 ? mediaPageIndex : 0;
}
