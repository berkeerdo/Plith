namespace Plith.Services;

/// <summary>
/// Remembers windows this process hid that belong to somebody else, so they can be put back.
///
/// WHY THIS EXISTS. <see cref="NativeFlyoutSuppressor"/> hides the shell's own volume flyout by
/// calling ShowWindow(SW_HIDE) on it. Nothing about that is persisted anywhere, so uninstalling
/// Plith leaves no setting behind and a crashed Plith takes its WinEvent hooks with it. The gap
/// was narrower and worse than a leftover setting: on the Windows builds where the shell creates
/// the flyout window ONCE and merely moves it on screen for each key press, the shell never
/// hides it and therefore believes it is still visible. Plith hiding it behind the shell's back
/// leaves a window the shell will position but never show again, so the volume OSD is gone for
/// the rest of that shell's life, and Plith exiting does not bring it back.
///
/// THE RETURN VALUE IS THE POINT. ShowWindow reports whether the window was PREVIOUSLY visible,
/// so a window that was already hidden when the suppressor reached it was hidden by somebody
/// else. Restoring that one would be this process showing a window it never took, which is the
/// same class of rudeness as the bug being fixed. Only a true transition from visible to hidden
/// is recorded.
///
/// Free of every Win32 type on purpose: the hide and show calls arrive as delegates, so the
/// bookkeeping can be tested on the headless suite. The same split as NotchGeometry and
/// FullscreenVideoDetector, for the same reason.
/// </summary>
public sealed class HiddenWindowRegistry
{
    private readonly HashSet<nint> _hidden = [];

    /// <summary>How many windows are currently owed a restore.</summary>
    public int Count => _hidden.Count;

    /// <summary>
    /// Hide <paramref name="hwnd"/> through <paramref name="hide"/>, and remember it only if it
    /// was visible until this call. <paramref name="hide"/> returns the window's previous
    /// visibility, which is what ShowWindow itself returns.
    /// </summary>
    public void Hide(nint hwnd, Func<nint, bool> hide)
    {
        ArgumentNullException.ThrowIfNull(hide);

        if (hide(hwnd)) _hidden.Add(hwnd);
    }

    /// <summary>
    /// Show everything this registry hid, and forget it.
    ///
    /// A window may have been destroyed since, and showing a dead handle is a no-op that reports
    /// failure rather than throwing, so no result is inspected here. Cleared either way: a handle
    /// that could not be restored will not be restorable later, and holding it would mean trying
    /// again forever.
    /// </summary>
    public void RestoreAll(Func<nint, bool> show)
    {
        ArgumentNullException.ThrowIfNull(show);

        foreach (var hwnd in _hidden) show(hwnd);
        _hidden.Clear();
    }
}
