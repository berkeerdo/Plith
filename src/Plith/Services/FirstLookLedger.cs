namespace Plith.Services;

/// <summary>
/// Whether this is the first look at the weather page today.
///
/// The first look of the day gets a reveal — the gradient swells in, the sun rises into frame,
/// cloud sweeps across — and every look after it gets the ambient loop. An event the first time,
/// wallpaper every time after: a flourish that fires on every glance stops meaning anything, and
/// on a surface living at the top of the screen it would go from delightful to tiring inside a
/// day.
///
/// The trigger is a <b>stored date</b>, not a session flag. Opening the notch fifty times before
/// noon must produce exactly one reveal, and a session flag would give one per launch — which on
/// a machine that reboots daily looks identical in testing and is wrong in use.
/// </summary>
public static class FirstLookLedger
{
    /// <summary>
    /// Decide, given what was stored and what today is.
    ///
    /// <paramref name="storedDate"/> is null when nothing has been stored — a fresh install, or
    /// a settings file that predates this. That is a first look.
    ///
    /// A stored date in the future is treated as stale rather than trusted. It is reachable
    /// through a clock correction or a timezone move, and trusting it would suppress the reveal
    /// until the calendar caught up — days, in the worst case, with no way for a person to tell
    /// why the animation stopped happening.
    /// </summary>
    public static bool IsFirstLook(DateOnly? storedDate, DateOnly today) =>
        storedDate is not DateOnly stored || stored != today;
}
