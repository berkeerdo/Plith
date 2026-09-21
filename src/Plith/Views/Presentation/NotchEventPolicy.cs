using Plith.Cards;

namespace Plith.Views.Presentation;

/// <summary>
/// Whether an event may take away the open notch frame, or has to leave it alone.
///
/// THE DISTINCTION IS WHO CAUSED IT. A frame is a place the person deliberately went: they
/// clicked the notch open and paged to something. Feedback for a key they just pressed belongs
/// on screen, because they are waiting to see the result of their own action. News that arrived
/// on its own does not, because nothing about it is worth taking their place away for.
///
/// The rule this replaces asked a different question, "is the frame already showing the thing
/// the event is about", and that was its own second attempt: the first was "an event never takes
/// the frame", reverted because a volume key with the frame open then showed nothing at all,
/// since no widget page carries the volume. Asking who caused it keeps that fix and adds the one
/// it was missing.
///
/// MEASURED DOING HARM on 2026-09-20. A verification run paged the notch to the shelf widget and
/// clicked, and the click landed on nothing: Spotify had advanced a track in between and the HUD
/// had taken the frame. Plith's own log, at the moment of the click:
///
///     12:57:35.489  Widget page committed: index=3/4   (the shelf page, reached)
///     12:57:36.451  Reposition: content=400x130        (the HUD took the frame)
///     12:57:36.684  Reposition: content=400x68         (back to the parked notch)
///
/// A script is not a person, but what it lost is exactly what a person loses: the thing they had
/// just navigated to, taken away by something they did not do.
///
/// VERIFIED ON HARDWARE, 2026-09-20, with both witnesses outside the behaviour under test. Plith
/// was shown to be subscribed by reading a track name off its own media page
/// ("playing C'EST LA VIE - Demeter"); the track was then changed with the hardware next-track
/// key and Spotify's window title moved to "MRK - Hello", proving an uncaused event really
/// happened; and the open frame was still on the shelf page afterwards.
///
/// The witnesses matter as much as the result. Two earlier probes asked Plith's own LOG whether
/// the event had arrived, and that question is entangled with this rule: when the frame is kept
/// there is no Reposition and no log line, so "nothing in the log" is both the success signature
/// and the nothing-happened signature. Both probes reported INCONCLUSIVE on what was probably a
/// pass, and the first reported PASS while nothing was playing at all.
///
/// Free of every WPF type so it can be tested on the headless suite, the same split as
/// <see cref="NotchGeometry"/> and FullscreenVideoDetector. OsdHost is a BandWindow and cannot
/// be constructed by the test project at all, which is how the previous two versions of this
/// rule reached a running build without a single test between them.
/// </summary>
public static class NotchEventPolicy
{
    /// <summary>
    /// True when the open frame stays where it is.
    /// </summary>
    /// <param name="reason">What asked for the OSD, or null from a caller that carries none.</param>
    /// <param name="frameIsOpen">Whether a widget frame is open and far enough along to be
    /// showing its content. With nothing open there is nothing to keep.</param>
    /// <param name="pageAlreadyShowsIt">Whether the page currently in the frame already displays
    /// what this event is about, which makes replacing it with a HUD a strict downgrade.</param>
    public static bool KeepsOpenFrame(ShowReason? reason, bool frameIsOpen, bool pageAlreadyShowsIt)
    {
        if (!frameIsOpen) return false;

        return !IsUserCaused(reason) || pageAlreadyShowsIt;
    }

    /// <summary>
    /// Whether the person did this themselves.
    ///
    /// AudioChange is deliberately NOT here. It means "the volume changed" and says nothing about
    /// who changed it: another application adjusting its own session raises it just as a key
    /// press does. Declining to interrupt on it costs nothing, because the person's own press
    /// arrives separately as <see cref="ShowReason.VolumeKey"/> from the keyboard hook, and that
    /// one is unambiguous.
    ///
    /// A null reason is not evidence of anything, so it does not interrupt either.
    /// </summary>
    private static bool IsUserCaused(ShowReason? reason) => reason is
        ShowReason.VolumeKey or
        ShowReason.MediaCommand or
        ShowReason.SummonHotkey;
}
