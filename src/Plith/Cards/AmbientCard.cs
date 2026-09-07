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
    private const int WeatherMaxAgeMinutes = 90;

    private readonly NotchHomeState _home;
    private readonly SettingsService _settings;
    private readonly WeatherService? _weather;
    private readonly DispatcherTimer _timer;
    private bool _lastVisible;

    public AmbientCard(NotchHomeState home, SettingsService settings, WeatherService? weather = null)
    {
        _home = home;
        _settings = settings;
        _weather = weather;
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
        _timer.Tick += (_, _) => Tick(DateTime.Now, CultureInfo.CurrentCulture, BatteryReader.Read(), _weather?.Current);

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
        // NotchHomeState is externally owned — App creates it and shares it with OsdHost — so
        // it is subscribed here rather than in the constructor, the same convention _settings
        // follows. MediaCard's constructor-time subscription to Vm.HasSessionChanged is not a
        // precedent for this: Vm is MediaCard's own view model, not a shared external object.
        _home.Changed += OnStateChanged;
        _settings.Changed += OnSettingsChanged;
        if (_weather is not null) _weather.Updated += OnWeatherUpdated;
        Tick(DateTime.Now, CultureInfo.CurrentCulture, BatteryReader.Read(), _weather?.Current);   // seed, so the first open is not blank
        _timer.Start();
    }

    public void Deactivate()
    {
        _timer.Stop();
        _home.Changed -= OnStateChanged;
        _settings.Changed -= OnSettingsChanged;
        if (_weather is not null) _weather.Updated -= OnWeatherUpdated;
    }

    /// <summary>Apply a point in time, a power reading and a weather reading to the row. All
    /// are parameters so the headless suite can drive a desktop and a laptop, with or without
    /// weather, from the same machine.</summary>
    public void Tick(DateTime now, CultureInfo culture, BatteryStatusRaw? battery, WeatherSnapshot? weather)
    {
        Vm.ApplyClock(now, culture);
        Vm.ApplyBattery(battery);
        Vm.ApplyWeather(weather, DateTimeOffset.UtcNow, WeatherMaxAgeMinutes);
    }

    // Reaches the row without waiting for the next 1 Hz timer tick, so a successful refresh
    // that lands while the notch is open is visible immediately rather than up to a second
    // late. WeatherService.Updated already fires on the UI dispatcher (see its doc comment),
    // so this handler is safe to call Tick from directly.
    private void OnWeatherUpdated() => Tick(DateTime.Now, CultureInfo.CurrentCulture, BatteryReader.Read(), _weather?.Current);

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
