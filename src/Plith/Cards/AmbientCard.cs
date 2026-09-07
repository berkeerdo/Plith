using System.Globalization;
using System.Windows.Threading;
using Plith.Services;
using Plith.ViewModels;

namespace Plith.Cards;

/// <summary>
/// The notch's ambient row: clock, weather and battery in a single horizontal card.
///
/// One card rather than three on purpose. OsdContent's stack is a vertical StackPanel that
/// draws a divider above every non-first card, so three cards would be three stacked rows and
/// two dividers before the media card is even reached. The reference apps render this content
/// as one compact row, and so does this — see spec section 1.
///
/// Visible only while NotchHomeState is open, which OsdHost opens on a deliberate hover and
/// never on an audio or media event. That is the whole point of the feature: an event shows
/// what changed, a hover shows what is going on.
/// </summary>
public sealed class AmbientCard : ICard
{
    private readonly NotchHomeState _home;
    private readonly SettingsService _settings;
    private readonly DispatcherTimer _timer;
    private bool _lastVisible;

    public AmbientCard(NotchHomeState home, SettingsService settings)
    {
        _home = home;
        _settings = settings;
        Vm = new AmbientCardViewModel();

        // 1 Hz so the minute rolls over promptly. The timer runs from Activate() to
        // Deactivate() regardless of whether the notch is open, which is 60 no-op property
        // writes a minute — the view model suppresses PropertyChanged when the string is
        // unchanged, so 59 of every 60 reach nothing. Gating it on the home state would couple
        // this card to presentation state it otherwise never reads, for no measurable gain.
        _timer = new DispatcherTimer(DispatcherPriority.Background, Dispatcher.CurrentDispatcher)
        {
            Interval = TimeSpan.FromSeconds(1),
        };
        _timer.Tick += (_, _) => Tick(DateTime.Now, CultureInfo.CurrentCulture);

        // Subscribed here rather than in Activate(), matching MediaCard's Vm.HasSessionChanged:
        // NotchHomeState is a shared object this card doesn't own the lifetime of, but the card
        // itself is registered once and lives for the app's lifetime, so there is no repeated
        // Activate()/Deactivate() cycle to guard against. Activate()/Deactivate() below manage
        // only what genuinely needs to start and stop with them — the settings subscription and
        // the timer.
        _home.Changed += OnStateChanged;

        _lastVisible = IsVisible;
    }

    public string Id => "ambient";
    public string AccessibleName => "Ambient status";
    public int Order => 5;
    public object ViewModel => Vm;
    public AmbientCardViewModel Vm { get; }

    public bool IsVisible => _home.IsOpen && _settings.Current.ShowAmbientOnHover;

    // Load-bearing for accessibility, not a debugging aid: WPF's ItemAutomationPeer names the
    // OSD's list container from this. See ICard.AccessibleName.
    public override string ToString() => AccessibleName;

    public event Action? VisibilityChanged;

    // Never raised: the ambient row is something the user opened by hovering, so it has no
    // business asking for the OSD on its own. Empty accessors rather than a field-like event,
    // because a field-like event that is never invoked trips CS0067 — same as AudioCard's
    // VisibilityChanged.
    public event Action<ShowRequest>? ShowRequested { add { } remove { } }

    public void Activate()
    {
        _settings.Changed += OnSettingsChanged;
        Tick(DateTime.Now, CultureInfo.CurrentCulture);   // seed, so the first open is not blank
        _timer.Start();
    }

    public void Deactivate()
    {
        _timer.Stop();
        _settings.Changed -= OnSettingsChanged;
    }

    /// <summary>Apply a point in time to the row. Public and parameterised so the headless
    /// suite can drive it without a Dispatcher loop or a real clock.</summary>
    public void Tick(DateTime now, CultureInfo culture) => Vm.ApplyClock(now, culture);

    private void OnSettingsChanged(SettingsModel _) => OnStateChanged();

    private void OnStateChanged()
    {
        // Raise only on a real transition. CardHost reconciles an ObservableCollection bound
        // to a live ItemsControl on every raise.
        bool now = IsVisible;
        if (now == _lastVisible) return;
        _lastVisible = now;
        VisibilityChanged?.Invoke();
    }
}
