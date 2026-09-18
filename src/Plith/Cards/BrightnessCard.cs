using System.Windows.Threading;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Cards;

/// <summary>
/// Brightness card. Visible from a brightness change until the OSD's show duration elapses,
/// and invisible the rest of the time.
///
/// That is unlike the Audio card, which is always visible because the OSD has no state in
/// which it says nothing about audio. Applying the same rule here would put a brightness row
/// on every volume press, for every user, including the laptop users whose panel this card
/// may never be able to reach.
///
/// The cost is that the show duration has two readers: this card and OsdHost's hide timer.
/// The alternative is plumbing an "OSD hidden" signal back from OsdHost through CardHost,
/// which today is deliberately fire and forget and holds no window reference. Reading one
/// setting from two places is the smaller price.
/// </summary>
public sealed class BrightnessCard : ICard
{
    private readonly SettingsService _settings;
    private readonly Action<TimeSpan, Action> _scheduleHide;
    private readonly DispatcherTimer? _timer;
    private bool _visible;

    /// <param name="scheduleHide">Restarts the visibility window. Defaults to a
    /// DispatcherTimer. A test supplies its own so the window can be closed on demand rather
    /// than waited out.</param>
    public BrightnessCard(SettingsService settings, Action<TimeSpan, Action>? scheduleHide = null)
    {
        _settings = settings;
        Vm = new BrightnessCardViewModel();

        if (scheduleHide is not null)
        {
            _scheduleHide = scheduleHide;
        }
        else
        {
            _timer = new DispatcherTimer();
            _timer.Tick += (_, _) => { _timer.Stop(); Hide(); };
            _scheduleHide = (after, _) =>
            {
                _timer.Stop();
                _timer.Interval = after;
                _timer.Start();
            };
        }
    }

    public string Id => "brightness";
    public string AccessibleName => "Brightness";
    public int Order => 30;
    public bool IsVisible => _visible;
    public object ViewModel => Vm;
    public BrightnessCardViewModel Vm { get; }

    public event Action? VisibilityChanged;
    public event Action<ShowRequest>? ShowRequested;

    // Load-bearing for accessibility, not a debugging aid: WPF's ItemAutomationPeer names the
    // OSD's list container from this. See ICard.AccessibleName.
    public override string ToString() => AccessibleName;

    public void Activate() { }

    public void Deactivate() => _timer?.Stop();

    /// <summary>
    /// The screen's brightness is now <paramref name="percent"/>.
    ///
    /// Must be called on the UI dispatcher. Both callers sit on a worker thread of their own
    /// (a WMI callback, and the writer's pump) and both marshal before reaching here, because
    /// CardHost reconciles straight into a bound ObservableCollection.
    /// </summary>
    public void Report(int percent)
    {
        Vm.Percent = percent;

        var wasVisible = _visible;
        _visible = true;
        if (!wasVisible) VisibilityChanged?.Invoke();

        _scheduleHide(TimeSpan.FromMilliseconds(_settings.Current.ShowDurationMs), Hide);

        ShowRequested?.Invoke(new ShowRequest(ShowReason.BrightnessChange, Id));
    }

    private void Hide()
    {
        if (!_visible) return;
        _visible = false;
        VisibilityChanged?.Invoke();
    }
}
