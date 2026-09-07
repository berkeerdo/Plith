namespace Plith.Services;

/// <summary>
/// Whether the notch's ambient home view is open.
///
/// Lives here rather than on the presentation because two layers that must not know about
/// each other both need it: OsdHost sets it (it owns the hover signal and the window), and
/// AmbientCard reads it (it owns nothing but its own visibility). CardHost stays out of it
/// entirely — its contract is that each card decides its own IsVisible, and MediaCard already
/// reads SettingsService the same way.
///
/// Both mutators are idempotent and raise Changed only on a real transition. OsdHost calls
/// Open() on every hover-in, including ones where the panel is already open, and Changed is
/// wired to CardHost.RecomputeVisibleCards — which reconciles an ObservableCollection bound
/// to a live ItemsControl. Re-raising for no change would churn that on every cursor wobble.
///
/// No thread affinity of its own, but callers must respect CardHost's: every ICard event
/// must be raised on the UI dispatcher, and Changed becomes one.
/// </summary>
public sealed class NotchHomeState
{
    public bool IsOpen { get; private set; }

    public event Action? Changed;

    public void Open()
    {
        if (IsOpen) return;
        IsOpen = true;
        Changed?.Invoke();
    }

    public void Close()
    {
        if (!IsOpen) return;
        IsOpen = false;
        Changed?.Invoke();
    }
}
