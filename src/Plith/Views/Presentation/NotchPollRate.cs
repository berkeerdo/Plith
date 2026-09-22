using System.Windows;

namespace Plith.Views.Presentation;

/// <summary>
/// Decides how often <see cref="NotchHoverPoller"/> needs to look at the cursor.
///
/// Separated from the poller for the reason everything in this namespace is: the poller needs a
/// Dispatcher and a live cursor, so anything inside it is unreachable from the test suite. The
/// gathering stays there, the rule lives here.
///
/// WHY THIS EXISTS. The poller ran at a flat 60 ms for as long as notch mode was the active
/// presentation, and `docs/PERF-VERIFICATION.md` section 10 measured what that cost: about 340
/// context switches a second on the UI thread, which is 80 to 85 per cent of everything the app
/// does at rest. The CPU cost is nearly nothing and always was, which is why section 2 missed
/// this for two releases: it measured the wrong axis.
///
/// THE RULE. Poll fast when something could plausibly happen, slowly when nothing can. The
/// cursor has to physically travel to a notch that is a few DIP tall at the top of the screen,
/// so a cursor 600 DIP away cannot reach it before the next slow tick sees it coming.
///
/// THE FAST RATE IS UNCHANGED. This changes WHEN 60 ms applies, never what it is. Once the
/// cursor is anywhere near the notch the poller behaves exactly as it did before, so the hover
/// that a person actually performs is sampled identically.
/// </summary>
public static class NotchPollRate
{
    /// <summary>The rate the notch has always been polled at, unchanged, for a cursor that is
    /// near enough to act. See <see cref="NotchHoverPoller"/> for why 60 ms.</summary>
    public static readonly TimeSpan Fast = TimeSpan.FromMilliseconds(60);

    /// <summary>
    /// The rate for a cursor that is too far away to reach the notch before the next tick.
    ///
    /// Paired with <see cref="WatchMarginDip"/> rather than chosen alone: the two are one
    /// decision, and the test suite holds them to it. A cursor would have to cross the whole
    /// margin inside one of these ticks to arrive unseen, which is 2000 DIP a second.
    /// </summary>
    public static readonly TimeSpan Slow = TimeSpan.FromMilliseconds(200);

    /// <summary>
    /// How far from the notch's own window the cursor still counts as near, in DIP.
    ///
    /// Generous on purpose, and the generosity is not about hover latency. The approach detector
    /// samples where a press began on the RISING EDGE of the button, so a press first observed
    /// when the cursor is already over the OSD records the origin as "inside" and refuses the
    /// drag. Keeping the whole neighbourhood of the notch at the fast rate means that edge is
    /// sampled at 60 ms exactly as before, everywhere it could matter.
    /// </summary>
    public const double WatchMarginDip = 400;

    /// <summary>
    /// True when the poller should run at <see cref="Fast"/>.
    /// </summary>
    /// <param name="panelRect">The whole OSD window in DIP, not just the resting strip. Empty
    /// before the first reposition.</param>
    /// <param name="cursorDip">Cursor position in the same space.</param>
    /// <param name="buttonDown">Whether a mouse button is currently held.</param>
    public static bool WantsFastPolling(Rect panelRect, Point cursorDip, bool buttonDown)
    {
        // A held button anywhere on the screen is a drag that might be heading here, and the
        // origin sampling above is why it cannot be allowed to arrive unobserved. This clause
        // costs nothing at rest, which is the only state the slow rate exists for: a resting
        // machine is not holding a mouse button.
        if (buttonDown) return true;

        // No geometry yet, or a degenerate rect. Every cursor would read as "far" and the notch
        // would stop answering hovers altogether, so fast is the only safe answer.
        if (panelRect.IsEmpty || panelRect.Width <= 0 || panelRect.Height <= 0) return true;

        var watch = panelRect;
        watch.Inflate(WatchMarginDip, WatchMarginDip);
        return watch.Contains(cursorDip);
    }

    /// <summary>The interval <see cref="WantsFastPolling"/> implies, for a caller that wants the
    /// answer in the unit the timer takes.</summary>
    public static TimeSpan For(Rect panelRect, Point cursorDip, bool buttonDown) =>
        WantsFastPolling(panelRect, cursorDip, buttonDown) ? Fast : Slow;
}
